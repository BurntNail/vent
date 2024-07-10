use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    liquid_utils::{compile_with_newtitle, CustomFormat},
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;
use axum_login::permission_required;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

#[derive(Deserialize)]
struct SmolDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
}
#[derive(Serialize)]
struct SmolFormattedDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: String,
}

impl<'a> From<(SmolDbEvent, &'a str)> for SmolFormattedDbEvent {
    fn from(
        (
            SmolDbEvent {
                id,
                event_name,
                date,
            },
            fmt,
        ): (SmolDbEvent, &'a str),
    ) -> Self {
        Self {
            id,
            event_name,
            date: date.to_env_string(fmt),
        }
    }
}

#[axum::debug_handler]
async fn get_(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    trace!("Getting events");

    let events: Vec<SmolFormattedDbEvent> = sqlx::query_as!(
        SmolDbEvent,
        r#"
SELECT id, event_name, date
FROM events e
ORDER BY e.date DESC
        "#
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })?
    .into_iter()
    .map(|event| {
        SmolFormattedDbEvent::from((event, state.settings.niche.date_time_format.as_str()))
    })
    .collect();

    trace!("Compiling");

    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/show_events.liquid",
        liquid::object!({ "events": events, "auth": aa }),
        &state.settings.brand.instance_name,
        Some("All Events".into()),
    )
        .await
}

#[derive(Deserialize)]
struct RemovePerson {
    pub person_id: Vec<i32>,
}

#[derive(Deserialize)]
struct RemoveEvent {
    pub event_id: Vec<i32>,
}

#[axum::debug_handler]
async fn post_remove_person(
    State(state): State<VentState>,
    Form(RemovePerson { person_id }): Form<RemovePerson>,
) -> Result<impl IntoResponse, VentError> {
    for person_id in person_id {
        trace!(?person_id, "Removing");
        sqlx::query!(
            r#"
DELETE FROM public.people
WHERE id=$1
            "#,
            person_id
        )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::RemovingPerson(person_id.into()),
            })?;
    }

    Ok(Redirect::to("/show_events"))
}

#[axum::debug_handler]
async fn post_remove_event(
    State(state): State<VentState>,
    Form(RemoveEvent { event_id }): Form<RemoveEvent>,
) -> Result<impl IntoResponse, VentError> {
    for event_id in event_id {
        trace!(?event_id, "Removing");
        sqlx::query!(
            r#"
    DELETE FROM public.events
    WHERE id=$1
            "#,
            event_id
        )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::RemovingEvent(event_id),
            })?;
    }

    Ok(Redirect::to("/show_events"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/remove_event", post(post_remove_event))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditEvents
        ))
        .route("/show_events", get(get_))
}
