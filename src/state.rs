use icalendar::{Calendar, Component, Event, EventLike};
use itertools::Itertools;
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use rand::{thread_rng, Rng};
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::{collections::HashMap, time::Duration};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{
        broadcast::{
            channel as broadcast_channel, Receiver as BroadcastReceiver, Sender as BroadcastSender,
        },
        mpsc::{unbounded_channel, UnboundedSender},
    },
};

use crate::{
    cfg::Settings,
    error::{
        ChannelReason, IOAction, IOSnafu, KnotError, LettreAction, LettreEmailSnafu, SendSnafu,
        SqlxAction, SqlxSnafu,
    },
    routes::DbEvent,
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
    update_calendar_sender: UnboundedSender<()>,
    stop_senders: BroadcastSender<()>,
    pub settings: Settings,
}

impl KnotState {
    pub async fn new(postgres: Pool<Postgres>) -> Self {
        let settings = Settings::new().await.expect("unable to get settings");
        let (stop_senders_tx, stop_senders_rx1) = broadcast_channel(2);

        let mail_sender = email_sender_thread(settings.clone(), stop_senders_rx1);
        let update_calendar_sender =
            update_calendar_thread(postgres.clone(), stop_senders_tx.subscribe());

        Self {
            postgres,
            mail_sender,
            update_calendar_sender,
            stop_senders: stop_senders_tx,
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

    pub fn update_events(&self) -> Result<(), KnotError> {
        self.update_calendar_sender.send(()).context(SendSnafu {
            reason: ChannelReason::SendUpdateCalMessage,
        })
    }

    pub async fn ensure_calendar_exists(&self) -> Result<(), KnotError> {
        if let Err(e) = File::open("./calendar.ics").await {
            warn!(?e, "Tried to open calendar, failed, rebuilding");
            self.update_events()?;
        }

        Ok(())
    }

    pub fn send_stop_notices(&self) {
        self.stop_senders
            .send(())
            .expect("unable to send stop messages");
    }
}

pub fn update_calendar_thread(
    pool: Pool<Postgres>,
    mut stop_rx: BroadcastReceiver<()>,
) -> UnboundedSender<()> {
    let (update_tx, mut update_rx) = unbounded_channel();

    async fn update_events(mut conn: PoolConnection<Postgres>) -> Result<(), KnotError> {
        let mut prefect_events: HashMap<i32, Vec<String>> = HashMap::new();

        let prefects = sqlx::query!(
            r#"
    SELECT id, first_name, surname FROM people p WHERE p.permissions != 'participant'"#
        )
        .fetch_all(&mut conn)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .into_iter()
        .map(|x| (x.id, format!("{} {}", x.first_name, x.surname)))
        .collect::<HashMap<_, _>>();
        let relations = sqlx::query!(
            r#"
    SELECT event_id, prefect_id FROM prefect_events"#
        )
        .fetch_all(&mut conn)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingParticipantsOrPrefectsAtEvents { event_id: None },
        })?;

        for rec in relations {
            if let Some(name) = prefects.get(&rec.event_id).cloned() {
                prefect_events.entry(rec.event_id).or_default().push(name);
            }
        }

        debug!(?prefect_events, "Worked out PEs");

        let mut calendar = Calendar::new();
        for DbEvent {
            id,
            event_name,
            date,
            location,
            teacher,
            other_info,
            zip_file: _,
        } in sqlx::query_as!(DbEvent, r#"SELECT * FROM events"#)
            .fetch_all(&mut conn)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingAllEvents,
            })?
        {
            let other_info = other_info.unwrap_or_default();
            let prefects = prefect_events
                .get(&id)
                .map(|x| x.join(", "))
                .unwrap_or_default();

            debug!(?event_name, ?date, "Adding event to calendar");

            calendar.push(
                Event::new()
                    .summary(&event_name)
                    .starts((date, chrono_tz::Europe::London))
                    .ends(date + chrono::Duration::minutes(45))
                    .location(&location)
                    .description(&format!(
                        r#"
Teacher: {teacher}
Other Information: {other_info}
Prefects Attending: {prefects}"#
                    ))
                    .done(),
            );
        }
        calendar.name("Kingsley House Events");

        let mut local_file = File::create("calendar.ics").await.context(IOSnafu {
            action: IOAction::CreatingFile("calendar.ics".into()),
        })?;
        local_file
            .write_all(calendar.done().to_string().as_bytes())
            .await
            .context(IOSnafu {
                action: IOAction::WritingToFile,
            })?;

        Ok(())
    }

    tokio::spawn(async move {
        loop {
            if let Some(_) = tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Calendar updater thread stopping");
                    Some(())
                },
                _update = update_rx.recv() => {
                    match pool.acquire().await {
                        Ok(conn) => if let Err(e) = update_events(conn).await {
                            error!(?e, "Error updating calendar!!!");
                        },
                        Err(e) => error!(?e, "Error getting connection to update calendar")
                    }

                    None
                }
            } {
                return;
            }

            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
        }
    });

    update_tx
}

pub fn email_sender_thread(
    settings: Settings,
    mut stop_rx: BroadcastReceiver<()>,
) -> UnboundedSender<EmailToSend> {
    let mail_settings = settings.mail.clone();
    let (msg_tx, mut msg_rx) = unbounded_channel();

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
            ))
            .context(LettreEmailSnafu {
                trying_to: LettreAction::BuildMessage,
            })?;

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

    msg_tx
}
