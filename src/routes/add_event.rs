use crate::{error::KnotError, liquid_utils::compile};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use super::{DbEvent, FormEvent};

pub const LOCATION: &str = "/add_event";

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
pub async fn get_add_event_form() -> Result<impl IntoResponse, KnotError> {
    compile("www/add_event.liquid", liquid::object!({})).await
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
