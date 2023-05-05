use std::sync::Arc;
use sqlx::types::chrono::{NaiveDateTime};
use axum::{extract::State, response::Html, Form};
use liquid::ParserBuilder;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::fs::read_to_string;

pub async fn root(State(pool): State<Arc<Pool<Postgres>>>) -> Html<String> {
    let mut conn = pool.acquire().await.expect("error getting db conn");

    #[derive(Serialize)]
    struct Anon {
        pub name: String
    }

    let events = sqlx::query_as!(
        Anon,
        "SELECT name FROM events"
    ).fetch_all(&mut conn).await.expect("error getting stuff");

    //MVP to see event names as can't serialise the date time.


    let liquid = read_to_string("www/templates/index.liquid").await.unwrap();
    let template = ParserBuilder::with_stdlib()
        .build()
        .unwrap()
        .parse(&liquid)
        .unwrap();

    let globals = liquid::object!({ "rows": events });

    let output = template.render(&globals).expect("error rendering");

    Html(output.to_string())
}

#[derive(Deserialize, Debug)]
pub struct HtmlEvent {
    pub name: String,
    pub date: String,
    pub location: String,
    pub teacher: String,
    pub prefects: String,
    pub info: String,
}

pub struct DbEvent {
    pub name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub prefects: String,
    pub participants: String,
    pub other_info: String,
}

impl TryFrom<HtmlEvent> for DbEvent {
    type Error = ();

    fn try_from(HtmlEvent {name, date, location, teacher, prefects, info}: HtmlEvent) -> Result<Self, Self::Error> {
        let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M").map_err(|_e| ())?; //we love not exposing data types lol

        Ok(Self {
            name, date, location, teacher, prefects, other_info: info, participants: String::new()
        })
    }
}

pub async fn root_form(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(event): Form<HtmlEvent>,
) -> Html<String> {
    let DbEvent { name, date, location, teacher, prefects, other_info, participants } = DbEvent::try_from(event).expect("error parsing html event");

    let mut conn = pool.acquire().await.expect("error getting database connection");

    sqlx::query!(
        "INSERT INTO events (name, date, location, teacher, prefects, participants, other_info) 
        VALUES ($1, $2, $3, $4, $5, $6, $7)",
        name,
        date,
        location,
        teacher,
        prefects,
        participants,
        other_info
    ).execute(&mut conn).await.expect("unable to add event");

    root(State(pool)).await
}
