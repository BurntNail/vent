pub mod db;
pub mod mail;

mod cache;
mod compiler;
pub mod db_objects;

use crate::{
    auth::add_password::get_email_to_be_sent_for_reset_password,
    cfg::Settings,
    error::{ChannelReason, SendSnafu, SqlxAction, SqlxSnafu, VentError},
    routes::calendar::update_calendar_thread,
    state::{
        cache::VentCache,
        compiler::VentCompiler,
        db::VentDatabase,
        mail::{email_sender_thread, EmailToSend},
    },
};
use axum::response::Html;
use liquid::Object;
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::{fmt::Debug, path::Path, sync::Arc};
use tokio::{
    fs::File,
    sync::{
        broadcast::{channel as broadcast_channel, Sender as BroadcastSender},
        mpsc::UnboundedSender,
        Mutex,
    },
};

#[derive(Clone, Debug)]
pub struct VentState {
    mail_sender: UnboundedSender<EmailToSend>,
    update_calendar_sender: UnboundedSender<()>,
    stop_senders: BroadcastSender<()>,
    pub settings: Settings,
    database: VentDatabase,
    compiler: VentCompiler,
    cache: Arc<Mutex<VentCache>>,
}

impl VentState {
    pub async fn new(postgres: Pool<Postgres>) -> Self {
        let settings = Settings::new().await.expect("unable to get settings");
        let (stop_senders_tx, stop_senders_rx1) = broadcast_channel(2);

        let mail_sender = email_sender_thread(settings.clone(), stop_senders_rx1);
        let update_calendar_sender = update_calendar_thread(
            postgres.clone(),
            stop_senders_tx.subscribe(),
            settings.timezone_id.clone(),
            &settings.brand.instance_name,
        );

        let database = VentDatabase::new(postgres);
        let compiler = VentCompiler;
        let mut cache = VentCache::new();
        cache.pre_populate().await;

        Self {
            database,
            mail_sender,
            update_calendar_sender,
            stop_senders: stop_senders_tx,
            settings,
            compiler,
            cache: Arc::new(Mutex::new(cache)),
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

    pub async fn ensure_calendar_exists(&self) -> Result<bool, VentError> {
        if let Err(e) = File::open("./calendar.ics").await {
            warn!(?e, "Tried to open calendar, failed, rebuilding");
            self.update_events()?;

            Ok(false)
        } else {
            debug!("Successfully found calendar");

            Ok(true)
        }
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
}
