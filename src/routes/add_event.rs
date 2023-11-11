//! Page to deal with adding events.
//!
//! NB: does not handle prefects/participants/images.
//!
//! It serves a simple form, and handles post requests to add that event to the DB.

use super::FormEvent;
use crate::{
    auth::{get_auth_object, Auth},
    error::{VentError, ParseTimeSnafu, SqlxAction, SqlxSnafu},
    liquid_utils::compile_with_newtitle,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;
use snafu::ResultExt;

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
#[instrument(level = "debug", skip(auth))]
#[axum::debug_handler]
pub async fn get_add_event_form(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    compile_with_newtitle(
        "www/add_event.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
        &state.settings.brand.instance_name,
        Some("New House Event".to_string()),
    )
    .await
}

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
#[instrument(level = "debug", skip(state, date, location, teacher, info))]
#[axum::debug_handler]
pub async fn post_add_event_form(
    State(state): State<VentState>,
    Form(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
    }): Form<FormEvent>,
) -> Result<impl IntoResponse, VentError> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")
        .context(ParseTimeSnafu { original: date })?;

    debug!("Fetching ID for update event");

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
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingEvent,
    })?
    .id;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_event/{id}"))) //redirect to the relevant update event page for that event
}
