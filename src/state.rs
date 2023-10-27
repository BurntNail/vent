use itertools::Itertools;
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use rand::{thread_rng, Rng};
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    cfg::Settings,
    error::{KnotError, SqlxAction, SqlxSnafu},
};

#[derive(Debug)]
pub struct EmailToSend {
    pub to_username: String,
    pub to_id: i32,
    pub to_fullname: String,
    pub unique_id: i32,
}

#[derive(Clone, Debug)]
pub struct KnotState {
    postgres: Pool<Postgres>,
    mail_sender: UnboundedSender<EmailToSend>,
    stop_sender: UnboundedSender<()>,
    pub settings: Settings,
}

impl KnotState {
    pub async fn new(postgres: Pool<Postgres>) -> Self {
        let settings = Settings::new().await.expect("unable to get settings");
        let (mail_sender, stop_sender) = email_sender_thread(settings.clone());

        Self {
            postgres,
            mail_sender,
            stop_sender,
            settings,
        }
    }

    pub async fn get_connection(&self) -> Result<PoolConnection<Postgres>, KnotError> {
        self.postgres.acquire().await.context(SqlxSnafu {
            action: SqlxAction::AcquiringConnection,
        })
    }

    pub async fn reset_password(&self, user_id: i32) -> Result<(), KnotError> {
        let current_ids =
            sqlx::query!(r#"SELECT password_link_id FROM people WHERE password_link_id <> NULL"#)
                .fetch_all(&mut self.get_connection().await?)
                .await
                .context(SqlxSnafu {
                    action: SqlxAction::FindingPerson(user_id.into()),
                })?
                .into_iter()
                .map(|x| x.password_link_id.unwrap()) //we check for null above so fine
                .collect_vec();

        let id: i32 = {
            let mut rng = thread_rng();
            let mut tester = rng.gen::<u16>();
            while current_ids.contains(&(tester.into())) {
                tester = rng.gen::<u16>();
            }
            tester
        }
        .into(); //ensure always positive

        sqlx::query!(
            "UPDATE people SET password_link_id = $1, hashed_password = NULL WHERE id = $2",
            id,
            user_id
        )
        .execute(&mut self.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingPerson(id.into()),
        })?;

        let person = sqlx::query!(
            "SELECT username, first_name, surname FROM people WHERE id = $1",
            user_id
        )
        .fetch_one(&mut self.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?;

        self.mail_sender
            .send(EmailToSend {
                to_username: person.username,
                to_id: user_id,
                to_fullname: format!("{} {}", person.first_name, person.surname),
                unique_id: id,
            })
            .expect("error sending email");

        Ok(())
    }

    pub fn stop_emails(&self) {
        self.stop_sender
            .send(())
            .expect("unable to send stop message");
    }
}

pub fn email_sender_thread(
    settings: Settings,
) -> (UnboundedSender<EmailToSend>, UnboundedSender<()>) {
    let mail_settings = settings.mail.clone();
    let (msg_tx, mut msg_rx) = unbounded_channel();
    let (stop_tx, mut stop_rx) = unbounded_channel();

    async fn send_email(
        EmailToSend {
            to_username,
            to_id,
            to_fullname,
            unique_id,
        }: EmailToSend,
        mailer: &AsyncSmtpTransport<Tokio1Executor>,
        from_username: &str,
        username_domain: &str,
        project_name: &str,
        project_domain: &str,
    ) -> Result<(), KnotError> {
        let m = Message::builder()
            .from(format!("Knot NoReply <{}>", from_username).parse()?)
            .to(format!("{to_fullname} <{to_username}@{}>", username_domain).parse()?)
            .subject("Knot - Add Password".to_string())
            .body(format!(
                r#"Dear {},

You've just tried to login to {}, but you don't have a password set yet.

To set one, go to {}/add_password/{}?code={}.

Have a nice day!"#,
                to_fullname, project_name, project_domain, to_id, unique_id
            ))?;

        info!(%to_fullname, %to_id, numbers=%unique_id, "Sending email.");

        mailer.send(m).await?;

        Ok(())
    }

    tokio::spawn(async move {
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&mail_settings.smtp)
            .expect("unable to get relay")
            .credentials(Credentials::new(
                mail_settings.username.clone(),
                mail_settings.password.clone(),
            ))
            .build();

        loop {
            if let Some(ret) = tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Mail thread stopping");
                    Some(Ok::<_, KnotError>(()))
                },
                msg = msg_rx.recv() => match msg {
                    None => Some(Ok(())),
                    Some(msg) => {
                        if let Err(e) = send_email(msg, &mailer, &mail_settings.username, &mail_settings.password, &mail_settings.smtp, &settings.brand.domain).await {
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
