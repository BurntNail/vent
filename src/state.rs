use std::env::var;
use lettre::{Message, transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor, AsyncTransport};
use once_cell::sync::Lazy;
use sqlx::{Pool, Postgres, pool::PoolConnection};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::{error::KnotError, PROJECT_NAME, liquid_utils::DOMAIN};

#[derive(Debug)]
pub struct EmailToSend {
    pub to_username: String,
    pub to_fullname: String,
    pub unique_id: i128,
}

#[derive(Clone)]
pub struct KnotState {
    postgres: Pool<Postgres>,
    mail_sender: UnboundedSender<EmailToSend>,
    stop_sender: UnboundedSender<()>,
}

impl KnotState {
    pub fn new (postgres: Pool<Postgres>) -> Self {
        let (mail_sender, stop_sender) = email_sender_thread();

        Self {
            postgres,
            mail_sender,
            stop_sender,
        }
    }

    pub async fn get_connection (&self) -> Result<PoolConnection<Postgres>, sqlx::Error> {
        self.postgres.acquire().await
    }

    pub fn send_email (&self, email: EmailToSend) {
        self.mail_sender.send(email).expect("error sending email");
    }

    pub fn stop_emails (&self) {
        self.stop_sender.send(()).expect("unable to send stop message");
    }
}


pub fn email_sender_thread () -> (UnboundedSender<EmailToSend>,UnboundedSender<()>) {
    static GMAIL_USERNAME: Lazy<String> = Lazy::new(|| var("GMAIL_USERNAME").expect("unable to get GMAIL_USERNAME"));
    static GMAIL_PASSWORD: Lazy<String> = Lazy::new(|| var("GMAIL_APP_PASSWORD").expect("unable to get GMAIL_APP_PASSWORD"));
    static USERNAME_DOMAIN: Lazy<String> = Lazy::new(|| var("USERNAME_DOMAIN").expect("unable to get USERNAME_DOMAIN"));


    let (msg_tx, mut msg_rx) = unbounded_channel();
    let (stop_tx, mut stop_rx) = unbounded_channel();

    async fn send_email (EmailToSend {to_username, to_fullname, unique_id}: EmailToSend, mailer: &AsyncSmtpTransport<Tokio1Executor>) -> Result<(), KnotError> {
        let m = Message::builder()
            .from(format!("Knot NoReply <{}>", GMAIL_USERNAME.as_str()).parse()?)
            .to(format!("{to_fullname} <{to_username}@{}>", USERNAME_DOMAIN.as_str()).parse()?)
            .subject("Knot - Add Password")
            .body(format!(r#"Dear {},

You've just tried to login to {}, but you don't have a password set yet.

To set one, go to {}/{}

Have a nice day!"#, to_fullname, PROJECT_NAME.as_str(), DOMAIN.1, unique_id))?;

        info!(?m, "Sending email");

        mailer.send(m).await?;

        Ok(())
    }

    tokio::spawn(async move {
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com").expect("unable to get relay").credentials(Credentials::new(GMAIL_USERNAME.clone(), GMAIL_PASSWORD.clone())).build();

        loop {
            if let Some(ret) = tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Mail thread stopping");
                    Some(Ok::<_, KnotError>(()))
                },
                msg = msg_rx.recv() => match msg {
                    None => Some(Ok(())),
                    Some(msg) => {
                        if let Err(e) = send_email(msg, &mailer).await {
                            error!(?e, "Error sending email");
                        }

                        None
                    }
                }
            } {
                return ret;
            }
        }
    });

    (msg_tx, stop_tx)
}