//! Page to deal with adding events.
//!
//! NB: does not handle prefects/participants/images.
//!
//! It serves a simple form, and handles post requests to add that event to the DB.

use crate::{
    error::{VentError, ParseTimeSnafu, SqlxAction, SqlxSnafu},
    routes::FormEvent,
    state::VentState,
};
use axum::{extract::State, response::{IntoResponse}, Router, Json};
use axum::routing::post;
use chrono::NaiveDateTime;
use snafu::ResultExt;

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
#[axum::debug_handler]
async fn post_add_event(
    State(state): State<VentState>,
    Json(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
    }): Json<FormEvent>,
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
    .fetch_one(&mut *state.get_connection().await?) //add the event to the db
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingEvent,
    })?
    .id;

    state.update_events()?;

    Ok(Json(id))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route(
            "/add_event",
            post(post_add_event),
        )
}
