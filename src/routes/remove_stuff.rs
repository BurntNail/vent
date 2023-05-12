use std::sync::Arc;

use axum::{response::{IntoResponse, Redirect}, extract::State, Form};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{error::KnotError, liquid_utils::compile};

use super::Person;

pub const LOCATION: &str = "/remove_stuff";

#[derive(Serialize, Deserialize)]
pub struct SmolDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
}

pub async fn get_remove_stuff (State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let people: Vec<Person> = sqlx::query_as!(
        Person,
        r#"
SELECT *
FROM people
        "#
    ).fetch_all(&mut conn).await?;

    let events: Vec<SmolDbEvent> = sqlx::query_as!(
        SmolDbEvent,
        r#"
SELECT id, event_name, date
FROM events
        "#
    ).fetch_all(&mut conn).await?;


    let globals = liquid::object!({
        "people": people,
        "events": events
    });

    compile("www/remove_stuff.liquid", globals).await
}

#[derive(Deserialize)]
pub struct RemovePerson {
    pub person_id: i32
}

#[derive(Deserialize)]
pub struct RemoveEvent {
    pub event_id: i32
}

pub async fn post_remove_person (State(pool): State<Arc<Pool<Postgres>>>, Form(RemovePerson { person_id }): Form<RemovePerson>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    sqlx::query!(
        r#"
DELETE FROM public.people
WHERE id=$1
        "#,
        person_id
    ).execute(&mut conn).await?;

    Ok(Redirect::to(LOCATION))
}
pub async fn post_remove_event (State(pool): State<Arc<Pool<Postgres>>>, Form(RemoveEvent { event_id }): Form<RemoveEvent>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    sqlx::query!(
        r#"
DELETE FROM public.events
WHERE id=$1
        "#,
        event_id
    ).execute(&mut conn).await?;


    Ok(Redirect::to(LOCATION))
}