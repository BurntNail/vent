use crate::{error::KnotError, liquid_utils::compile};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use super::DbEvent;

pub const LOCATION: &str = "/add_event";

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
pub async fn get_add_event_form() -> Result<impl IntoResponse, KnotError> {
    compile("www/add_event.liquid", liquid::object!({})).await
}

///Struct to hold the event that comes back from the [`add_event`] form
#[derive(Debug, Deserialize)]
pub struct FormEvent {
    pub name: String,
    pub date: String,
    pub location: String,
    pub teacher: String,
    pub info: String,
}

impl TryFrom<FormEvent> for DbEvent {
    type Error = KnotError;

    ///Get a [`DbEvent`] from a [`FormEvent`], can fail if we can't parse the date.
    fn try_from(
        FormEvent {
            name,
            date,
            location,
            teacher,
            info,
        }: FormEvent,
    ) -> Result<Self, Self::Error> {
        let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")?;

        Ok(Self {
            id: -1, //no ID for events to be added
            event_name: name,
            date,
            location,
            teacher,
            other_info: Some(info),
        })
    }
}

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
pub async fn post_add_event_form(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(event): Form<FormEvent>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let DbEvent {
        id: _,
        event_name,
        date,
        location,
        teacher,
        other_info: info,
    } = DbEvent::try_from(event)?;

    sqlx::query!(
        r#"
INSERT INTO public.events
(event_name, "date", "location", teacher, other_info)
VALUES($1, $2, $3, $4, $5)
RETURNING id
        "#,
        event_name,
        date,
        location,
        teacher,
        info
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(Redirect::to(LOCATION))
}
