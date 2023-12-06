//! Module that publishes 2 `POST` methods that deal with adding prefects and participants to events based off of path parameters. This is a fair bit easier than an invisible form.

use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        PermissionsRole, PermissionsTarget,
    },
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::post,
    Router,
};
use axum_extra::extract::Form;
use axum_login::{login_required, permission_required};
use chrono::Utc;
use serde::Deserialize;
use snafu::ResultExt;

#[derive(Deserialize)]
pub struct AddPerson {
    person_ids: Vec<i32>,
    event_id: i32,
}

///`POST` method that adds a prefect to an event
#[axum::debug_handler]
async fn post_add_prefect_to_event(
    State(state): State<VentState>,
    Form(AddPerson {
        event_id,
        person_ids,
    }): Form<AddPerson>,
) -> Result<impl IntoResponse, VentError> {
    for prefect_id in person_ids {
        if sqlx::query!(
            r#"
    SELECT * FROM public.prefect_events
    WHERE prefect_id = $1
    AND event_id = $2"#,
            prefect_id,
            event_id
        )
        .fetch_optional(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(prefect_id.into()),
        })?
        .is_none()
        //if we can't find anything associated with this prefect and this event
        {
            debug!(%prefect_id, %event_id, "Adding prefect to event");

            //then we add the prefect to the event
            sqlx::query!(
                r#"
    INSERT INTO public.prefect_events
    (prefect_id, event_id)
    VALUES($1, $2);            
                "#,
                prefect_id,
                event_id
            )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::AddingParticipantOrPrefect {
                    person: prefect_id.into(),
                    event_id,
                },
            })?;
        } else {
            warn!(%prefect_id, %event_id, "Prefect already in event");
        }
    }

    Ok(Redirect::to(&format!("/update_event/{event_id}"))) //redirect back to the update event page
}

///`POST` method that adds a participant
#[axum::debug_handler]
async fn post_add_participant_to_event(
    auth: Auth,
    State(state): State<VentState>,
    Form(AddPerson {
        event_id,
        person_ids,
    }): Form<AddPerson>,
) -> Result<impl IntoResponse, VentError> {
    let current_user = auth.user.expect("need to be logged in to add participants");

    let event_date = sqlx::query!("SELECT date FROM events WHERE id = $1", event_id)
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingEvent(event_id),
        })?
        .date;

    if event_date < (Utc::now() + chrono::Duration::hours(1)).naive_local()
        && current_user.permissions < PermissionsRole::Prefect
    {
        warn!("Student {person_ids:?} tried to add to {event_id}, but event out of date.");
        return Ok(Redirect::to(&format!("/update_event/{event_id}")));
    }

    for participant_id in person_ids {
        if sqlx::query!(
            r#"
    SELECT * FROM public.participant_events
    WHERE participant_id = $1
    AND event_id = $2"#,
            participant_id,
            event_id
        )
        .fetch_optional(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingParticipantOrPrefect {
                person: participant_id.into(),
                event_id,
            },
        })?
        .is_none()
        //if we can't find anything assoiated with this participant and this event
        {
            if current_user.permissions < PermissionsRole::Prefect
                && current_user.id != participant_id
            {
                warn!(?participant_id, perp=?current_user.id, "Participant did POST magic to get other participant, but failed.");
                continue;
            }

            debug!(%participant_id, %event_id, "Adding participant to event");
            //then we add the participant to the event
            sqlx::query!(
                r#"
    INSERT INTO public.participant_events
    (participant_id, event_id, is_verified)
    VALUES($1, $2, false);            
                "#,
                participant_id,
                event_id
            )
            .execute(&mut *state.get_connection().await?)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::AddingParticipantOrPrefect {
                    person: participant_id.into(),
                    event_id,
                },
            })?;
        } else {
            warn!(%participant_id, %event_id, "Participant already in event.");
        }
    }

    state.update_events()?;

    Ok(Redirect::to(&format!("/update_event/{event_id}"))) //then back to the update event page
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_prefect", post(post_add_prefect_to_event))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPrefectsOnEvents
        ))
        .route("/add_participant", post(post_add_participant_to_event))
        .route_layer(login_required!(VentAuthBackend)) //TODO: actually use `PermissionsTarget::AddRmSelfToEvent`
}
