pub mod db;
pub mod mail;

mod cache;
mod compiler;
pub mod db_objects;
pub mod s3;

use crate::{
    auth::{
        add_password::get_email_to_be_sent_for_reset_password, backend::VentAuthBackend,
        PermissionsTarget,
    },
    cfg::Settings,
    error::{ChannelReason, SendSnafu, SqlxAction, SqlxSnafu, VentError},
    routes::{
        calendar::{get_events, update_calendar_thread},
        public::serve_bytes_with_mime,
    },
    state::{
        cache::VentCache,
        compiler::VentCompiler,
        db::VentDatabase,
        mail::{email_sender_thread, EmailToSend},
    },
};
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use axum_login::permission_required;
use icalendar::Calendar;
use liquid::Object;
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::{fmt::Debug, path::Path, sync::Arc};
use tokio::sync::{
    broadcast::{channel as broadcast_channel, Sender as BroadcastSender},
    mpsc::UnboundedSender,
    RwLock,
};
use crate::state::s3::S3Bucket;

#[derive(Clone, Debug)]
pub struct VentState {
    mail_sender: UnboundedSender<EmailToSend>,
    update_calendar_sender: UnboundedSender<()>,
    calendar: Arc<RwLock<Calendar>>,
    stop_senders: BroadcastSender<()>,
    pub settings: Settings,
    database: VentDatabase,
    compiler: VentCompiler,
    cache: VentCache,
    pub bucket: S3Bucket,
}

impl VentState {
    pub async fn new(postgres: Pool<Postgres>) -> Self {
        let bucket = S3Bucket::new();
        let settings = Settings::new().await.expect("unable to get settings");
        let (stop_senders_tx, stop_senders_rx1) = broadcast_channel(2);

        let calendar = Arc::new(RwLock::new({
            let conn = postgres
                .acquire()
                .await
                .expect("unable to get postgres connection");
            get_events(
                conn,
                settings.timezone_id.clone(),
                &settings.brand.instance_name,
            )
            .await
            .expect("unable to create calendar")
        }));

        let mail_sender = email_sender_thread(settings.clone(), stop_senders_rx1);
        let update_calendar_sender = update_calendar_thread(
            postgres.clone(),
            stop_senders_tx.subscribe(),
            settings.timezone_id.clone(),
            &settings.brand.instance_name,
            calendar.clone(),
        );

        let database = VentDatabase::new(postgres);
        let compiler = VentCompiler;
        let cache = VentCache::new();
        cache.pre_populate().await;

        Self {
            database,
            mail_sender,
            update_calendar_sender,
            stop_senders: stop_senders_tx,
            calendar,
            settings,
            compiler,
            cache,
            bucket
        }
    }

    pub async fn get_connection(&self) -> Result<PoolConnection<Postgres>, VentError> {
        self.database.pool.acquire().await.context(SqlxSnafu {
            action: SqlxAction::AcquiringConnection,
        })
    }

    pub async fn reset_password(&self, user_id: i32) -> Result<(), VentError> {
        let email =
            get_email_to_be_sent_for_reset_password(self.get_connection().await?, user_id).await?;

        self.mail_sender.send(email).expect("error sending email");

        Ok(())
    }

    pub fn update_events(&self) -> Result<(), VentError> {
        self.update_calendar_sender.send(()).context(SendSnafu {
            reason: ChannelReason::SendUpdateCalMessage,
        })
    }

    pub fn send_stop_notices(&self) {
        self.stop_senders
            .send(())
            .expect("unable to send stop messages");
    }

    pub async fn compile(
        &self,
        path: impl AsRef<Path> + Debug,
        globals: Object,
        title_additional_info: Option<String>,
    ) -> Result<Html<String>, VentError> {
        self.compiler
            .compile_with_newtitle(
                path,
                globals,
                title_additional_info,
                &self.settings,
                self.cache.clone(),
            )
            .await
    }

    pub async fn get_calendar(&self) -> Result<Response, VentError> {
        let bytes = self.calendar.read().await.to_string().into_bytes();
        serve_bytes_with_mime(bytes, "text/calendar").await
    }
}

pub async fn reload_cache(State(state): State<VentState>) -> impl IntoResponse {
    state.cache.clear();
    Redirect::to("/")
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/reload_pages", get(reload_cache))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::DevAccess
        ))
}
