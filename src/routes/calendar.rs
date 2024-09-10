//! Module that publishes an iCalendar file in a GET method

use crate::{
    error::{SqlxAction, SqlxSnafu, VentError},
    state::{db_objects::DbEvent, VentState},
};
use axum::{extract::State, response::IntoResponse};

use icalendar::{Calendar, CalendarDateTime, Component, Event, EventLike};
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Pool, Postgres};
use std::{collections::HashMap, time::Duration};
use std::sync::Arc;
use tokio::{
    sync::{
        broadcast::{error::TryRecvError, Receiver as BroadcastReceiver},
        mpsc::{unbounded_channel, UnboundedSender},
    },
};
use tokio::sync::RwLock;

#[axum::debug_handler]
pub async fn get_calendar_feed(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    Ok(state.get_calendar().await)
}

pub async fn get_events(
    mut conn: PoolConnection<Postgres>,
    tzid: String,
    calendar_title: &str,
) -> Result<Calendar, VentError> {
    let mut prefect_events: HashMap<i32, Vec<String>> = HashMap::new();

    let prefects = sqlx::query!(
            r#"
    SELECT id, first_name, surname FROM people p WHERE p.permissions != 'participant'"#
        )
        .fetch_all(&mut *conn)
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
        .fetch_all(&mut *conn)
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
        id: _,
        event_name,
        date,
        location,
        teacher: _,
        other_info: _,
        zip_file: _,
        is_locked: _,
        extra_points: _,
    } in sqlx::query_as!(DbEvent, r#"SELECT * FROM events"#)
        .fetch_all(&mut *conn)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingAllEvents,
        })?
    {
        /* let other_info = other_info.unwrap_or_default();
        let prefects = prefect_events
            .get(&id)
            .map(|x| x.join(", "))
            .unwrap_or_default(); */

        debug!(?event_name, ?date, "Adding event to calendar");

        calendar.push(
            Event::new()
                .summary(&event_name)
                .starts(CalendarDateTime::WithTimezone {
                    date_time: date,
                    tzid: tzid.clone(),
                })
                .ends(date)
                .location(&location)
                /* .description(&format!(
                    r#"
Teacher: {teacher}
Other Information: {other_info}
Prefects Attending: {prefects}"#
                )) */
                .done(),
        );
    }
    calendar.name(calendar_title);

    Ok(calendar)
}

pub fn update_calendar_thread(
    pool: Pool<Postgres>,
    mut stop_rx: BroadcastReceiver<()>,
    tzid: String,
    instance_name: &impl AsRef<str>,
    calendar: Arc<RwLock<Calendar>>
) -> UnboundedSender<()> {
    let instance_name = instance_name.as_ref();
    let (update_tx, mut update_rx) = unbounded_channel();
    let calendar_title = format!("{instance_name} Events");

    tokio::spawn(async move {
        loop {
            if !matches!(stop_rx.try_recv(), Err(TryRecvError::Empty)) {
                info!("Old sessions thread stopping");
                return;
            }

            if let Ok(()) = update_rx.try_recv() {
                match pool.acquire().await {
                    Ok(conn) => {
                        match get_events(conn, tzid.clone(), &calendar_title).await {
                            Ok(x) => {
                                *calendar.write().await = x;
                            },
                            Err(e) => {
                                error!(?e, "Error updating calendar!!!");
                            }
                        }
                    }
                    Err(e) => error!(?e, "Error getting connection to update calendar"),
                }
            }

            tokio::time::sleep(Duration::from_secs(10 * 60)).await; //check every 10m
        }
    });

    update_tx
}
