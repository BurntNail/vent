use axum::{extract::State, response::{Html, IntoResponse}, http::StatusCode};
use chrono::NaiveDateTime;
use liquid::ParserBuilder;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, sync::Arc};
use tokio::fs::read_to_string;

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq)]
struct Event {
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: String,
}

#[derive(Serialize, Deserialize)]
struct Person {
    pub person_name: String,
    pub person_email: String,
}

#[derive(thiserror::Error, Debug)]
pub enum RootError {
    #[error("Database Error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Liquid Error")]
    Liquid(#[from] liquid::Error),
    #[error("IO Error")]
    IO(#[from] std::io::Error),
}

impl IntoResponse for RootError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("Internal Server Error: {self:?}"))).into_response()
    }
}

pub async fn root(State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, RootError> {
    let mut conn = pool.acquire().await?;

    #[derive(Serialize, Deserialize)]
    struct Anon {
        pub event_name: String,
        pub date: NaiveDateTime,
        pub location: String,
        pub teacher: String,
        pub other_info: Option<String>,
        pub person_name: String,
        pub person_email: String,
    }
    #[derive(Serialize, Deserialize)]
    struct WholeEvent {
        pub event: Event,
        pub people: Vec<Person>,
        pub n_events: usize,
    }

    let mut events = HashMap::new();
    for anon in sqlx::query_as!(
        Anon,
        r#"
        SELECT e.event_name, e.date, e.location, e.teacher, e.other_info, p.person_name, p.person_email
        FROM events e
        INNER JOIN participant_events pe ON pe.event_id = e.id
        INNER JOIN people p ON p.id = pe.participant_id
        "#
    ).fetch_all(&mut conn).await? {
        let Anon { event_name, date, location, teacher, other_info, person_name, person_email } = anon;
        let other_info = other_info.unwrap_or_default();

        let event = Event {event_name, date, location, teacher, other_info};
        let person = Person {person_name, person_email};

        events.entry(event).or_insert(vec![]).push(person);
    }
    let mut events: Vec<WholeEvent> = events
        .into_iter()
        .map(|(event, people)| WholeEvent {
            event,
            n_events: people.len(), //n_events before people to borrow then own
            people,
        })
        .collect();
    events.sort_by_key(
        |WholeEvent {
             event:
                 Event {
                     event_name: _,
                     date,
                     location: _,
                     teacher: _,
                     other_info: _,
                 },
             people: _,
             n_events: _,
         }| { *date },
    );

    let globals = liquid::object!({ "events": events });
    info!(?globals);

    let liquid = read_to_string("www/templates/index.liquid").await?;
    let template = ParserBuilder::with_stdlib()
        .build()?
        .parse(&liquid)?;

    let output = template.render(&globals)?;

    Ok(Html(output))
}
