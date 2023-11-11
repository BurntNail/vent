//! Module that publishes an iCalendar file in a GET method

use crate::{
    error::{IOAction, IOSnafu, VentError, SqlxAction, SqlxSnafu},
    routes::public::serve_static_file,
    state::{db_objects::DbEvent, VentState},
};
use axum::{extract::State, response::IntoResponse};
use icalendar::{Calendar, Component, Event, EventLike};
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::{time::Duration};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{
        broadcast::Receiver as BroadcastReceiver,
        mpsc::{unbounded_channel, UnboundedSender},
    },
};

#[instrument(level = "debug", skip(state))]
#[axum::debug_handler]
pub async fn get_calendar_feed(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    if !(state.ensure_calendar_exists().await?) {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    serve_static_file("calendar.ics").await
}

pub fn update_calendar_thread(
    pool: Pool<Postgres>,
    mut stop_rx: BroadcastReceiver<()>,
) -> UnboundedSender<()> {
    let (update_tx, mut update_rx) = unbounded_channel();

    async fn update_events(mut conn: PoolConnection<Postgres>) -> Result<(), VentError> {
        let mut calendar = Calendar::new();

        for DbEvent {
            id: _id,
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
Other Information: {other_info}"#
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
            if tokio::select! {
                _stop = stop_rx.recv() => {
                    info!("Mail thread stopping");
                    true
                },
                _msg = update_rx.recv() => {
                    match pool.acquire().await {
                        Ok(conn) => {
                            if let Err(e) = update_events(conn).await {
                                error!(?e, "Error updating calendar!!!");
                            }
                        }
                        Err(e) => error!(?e, "Error getting connection to update calendar"),
                    }
                    false
                }
            } {
                return;
            }
        }
    });

    update_tx
}
