//! Page to deal with adding events.
//!
//! NB: does not handle prefects/participants/images.
//!
//! It serves a simple form, and handles post requests to add that event to the DB.

use super::FormEvent;
use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile,
    state::KnotState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
#[instrument(level = "trace")]
pub async fn get_add_event_form(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_event.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
}

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
#[instrument(level = "trace")]
pub async fn post_add_event_form(
    State(state): State<KnotState>,
    Form(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
    }): Form<FormEvent>,
) -> Result<impl IntoResponse, KnotError> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")?;

    let id = sqlx::query!(
        r#"
INSERT INTO public.events
(event_name, "date", "location", teacher, other_info)
VALUES($1, $2, $3, $4, $5)
RETURNING id
        "#,
        name,
        date,
        location,
        teacher,
        info
    )
    .fetch_one(&mut state.get_connection().await?) //add the event to the db
    .await?
    .id;

    Ok(Redirect::to(&format!("/update_event/{id}"))) //redirect to the relevant update event page for that event
}
