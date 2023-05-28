//! Module that publishes an iCalendar file in a GET/HEAD method

use crate::{error::KnotError, routes::DbEvent};
use axum::{extract::State, response::IntoResponse};
use chrono::Duration;
use icalendar::{Calendar, Component, Event, EventLike};
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, sync::Arc};
use tokio::{fs::File, io::AsyncWriteExt};

use super::public::serve_static_file;

pub async fn get_calendar_feed(
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let prefect_events = {
        let mut map: HashMap<i32, Vec<String>> = HashMap::new();

        let prefects = sqlx::query!(
            r#"
    SELECT id, first_name, surname FROM people p WHERE p.is_prefect = true"#
        )
        .fetch_all(pool.as_ref())
        .await?
        .into_iter()
        .map(|x| (x.id, format!("{} {}", x.first_name, x.surname)))
        .collect::<HashMap<_, _>>();
        let rels = sqlx::query!(
            r#"
    SELECT event_id, prefect_id FROM prefect_events"#
        )
        .fetch_all(pool.as_ref())
        .await?;

        for rec in rels {
            if let Some(name) = prefects.get(&rec.event_id).cloned() {
                map.entry(rec.event_id).or_default().push(name);
            }
        }

        map
    };

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
        .fetch_all(pool.as_ref())
        .await?
    {
        let other_info = other_info.unwrap_or_default();
        let prefects = prefect_events
            .get(&id)
            .map(|x| x.join(", "))
            .unwrap_or_default();

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

    {
        let mut local_file = File::create("calendar.ics").await?;
        local_file
            .write_all(calendar.done().to_string().as_bytes())
            .await?;
    }

    serve_static_file("calendar.ics").await
}
