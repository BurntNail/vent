//! Module that publishes an iCalendar file in a GET/HEAD method

use crate::{error::KnotError, routes::DbEvent, state::KnotState};
use axum::{extract::State, response::IntoResponse};
use chrono::Duration;
use icalendar::{Calendar, Component, Event, EventLike};
use std::collections::HashMap;
use tokio::{fs::File, io::AsyncWriteExt};
use tracing::Instrument;

use super::public::serve_static_file;

#[instrument(level = "debug", skip(state))]
pub async fn get_calendar_feed(
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let prefect_events: Result<_, KnotError> = async {
        let mut map: HashMap<i32, Vec<String>> = HashMap::new();

        let prefects = sqlx::query!(
            r#"
    SELECT id, first_name, surname FROM people p WHERE p.permissions != 'participant'"#
        )
        .fetch_all(&mut state.get_connection().await?)
        .await?
        .into_iter()
        .map(|x| (x.id, format!("{} {}", x.first_name, x.surname)))
        .collect::<HashMap<_, _>>();
        let relations = sqlx::query!(
            r#"
    SELECT event_id, prefect_id FROM prefect_events"#
        )
        .fetch_all(&mut state.get_connection().await?)
        .await?;

        for rec in relations {
            if let Some(name) = prefects.get(&rec.event_id).cloned() {
                map.entry(rec.event_id).or_default().push(name);
            }
        }

        Ok(map)
    }
    .instrument(debug_span!("getting_prefect_events"))
    .await;
    let prefect_events = prefect_events?;

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
        .fetch_all(&mut state.get_connection().await?)
        .await?
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
                .starts(date)
                .ends(date + Duration::minutes(45))
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

    let calr: Result<_, KnotError> = async {
        let mut local_file = File::create("calendar.ics").await?;
        local_file
            .write_all(calendar.done().to_string().as_bytes())
            .await?;
        Ok(())
    }
    .instrument(debug_span!("writing_calendar_to_file"))
    .await;
    calr?;

    serve_static_file("calendar.ics").await
}
