//! Page to deal with adding events.
//!
//! NB: does not handle prefects/participants/images.
//!
//! It serves a simple form, and handles post requests to add that event to the DB.

use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{EncodeStep, ParseTimeSnafu, SqlxAction, SqlxSnafu, VentError},
    routes::FormEvent,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use chrono::NaiveDateTime;
use snafu::ResultExt;

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
#[axum::debug_handler]
async fn get_add_event_form(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;

    state
        .compile(
            "www/add_event.liquid",
            liquid::object!({"auth": aa}),
            Some("New House Event".to_string()),
        )
        .await
}

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
#[axum::debug_handler]
async fn post_add_event_form(
    State(state): State<VentState>,
    Form(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
        is_locked,
    }): Form<FormEvent>,
) -> Result<impl IntoResponse, VentError> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M").context(ParseTimeSnafu {
        original: date,
        how_got_in: EncodeStep::Encode,
    })?;

    debug!("Fetching ID for update event");

    let id = sqlx::query!(
        r#"
INSERT INTO public.events
(event_name, "date", "location", teacher, other_info, is_locked)
VALUES($1, $2, $3, $4, $5, $6)
RETURNING id
        "#,
        name,
        date,
        location,
        teacher,
        info,
        is_locked
    )
    .fetch_one(&mut *state.get_connection().await?) //add the event to the db
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingEvent,
    })?
    .id;

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_event/{id}"))) //redirect to the relevant update event page for that event
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route(
            "/add_event",
            get(get_add_event_form).post(post_add_event_form),
        )
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditEvents
        ))
}
