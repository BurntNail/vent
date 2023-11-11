pub mod db;
pub mod mail;

pub mod db_objects;

use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use tokio::{
    fs::File,
    sync::{
        broadcast::{channel as broadcast_channel, Sender as BroadcastSender},
        mpsc::UnboundedSender,
    },
};

use crate::{
    auth::{
        add_password::get_email_to_be_sent_for_reset_password,
        pg_session::clear_out_old_sessions_thread,
    },
    cfg::Settings,
    error::{ChannelReason, VentError, SendSnafu, SqlxAction, SqlxSnafu},
    routes::calendar::update_calendar_thread,
    state::{
        db::VentDB,
        mail::{email_sender_thread, EmailToSend},
    },
};

#[derive(Clone, Debug)]
pub struct VentState {
    mail_sender: UnboundedSender<EmailToSend>,
    update_calendar_sender: UnboundedSender<()>,
    stop_senders: BroadcastSender<()>,
    pub settings: Settings,
    database: VentDB,
}

impl VentState {
    pub async fn new(postgres: Pool<Postgres>) -> Self {
        let settings = Settings::new().await.expect("unable to get settings");
        let (stop_senders_tx, stop_senders_rx1) = broadcast_channel(3);

        let mail_sender = email_sender_thread(settings.clone(), stop_senders_rx1);
        let update_calendar_sender =
            update_calendar_thread(postgres.clone(), stop_senders_tx.subscribe());
        clear_out_old_sessions_thread(postgres.clone(), stop_senders_tx.subscribe());

        let database = VentDB::new(postgres);

        Self {
            database,
            mail_sender,
            update_calendar_sender,
            stop_senders: stop_senders_tx,
            settings,
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
}
