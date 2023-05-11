use axum::{extract::State, response::IntoResponse};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, sync::Arc};

use crate::{error::KnotError, liquid_utils::compile};

pub const LOCATION: &str = "/";

#[derive(Serialize, Deserialize)]
struct Event {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Person {
    pub person_name: String,
    pub person_email: String,
}

pub async fn get_index(
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    #[derive(Serialize)]
    struct WholeEvent {
        event: Event,
        participants: Vec<Person>,
        prefects: Vec<Person>,
        n_participants: usize,
        n_prefects: usize,
    }

    let mut events = vec![];
    for event in sqlx::query_as!(
        Event,
        r#"
SELECT *
FROM events
        "#
    ).fetch_all(&mut conn).await? {
        let event_id = event.id;
        let prefects = sqlx::query_as!(
            Person,
            r#"
SELECT p.person_name, p.person_email
FROM people p
INNER JOIN events e ON e.id = $1
INNER JOIN prefect_events pe ON p.id = pe.prefect_id
            "#,
            event_id
        ).fetch_all(&mut conn).await?;
        let participants = sqlx::query_as!(
            Person,
            r#"
SELECT p.person_name, p.person_email
FROM people p
INNER JOIN events e ON e.id = $1
INNER JOIN participant_events pe ON p.id = pe.participant_id
            "#,
            event_id
        ).fetch_all(&mut conn).await?;

        events.push(
            WholeEvent {
                event,
                n_participants: participants.len(),
                n_prefects: prefects.len(),
                participants,
                prefects
            }
        );
    }

    let globals = liquid::object!({ "events": events });

    compile("www/index.liquid", globals).await
}
