//! Module that publishes 2 `POST` methods that deal with adding prefects and participants to events based off of path parameters. This is a fair bit easier than an invisible form.

use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};
use axum::{extract::State, response::IntoResponse, routing::post, Router, Json};
use http::StatusCode;
use serde::Deserialize;
use snafu::ResultExt;

#[derive(Deserialize)]
pub struct AddPerson {
    person_ids: Vec<i32>,
    event_id: i32,
}

///`POST` method that adds a prefect to an event
#[axum::debug_handler]
async fn post_add_prefects_to_event(
    State(state): State<VentState>,
    Json(AddPerson {
        event_id,
        person_ids,
    }): Json<AddPerson>,
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

    Ok(StatusCode::OK)
}

///`POST` method that adds a participant
#[axum::debug_handler]
async fn post_add_participants_to_event(
    State(state): State<VentState>,
    Json(AddPerson {
        event_id,
        person_ids,
    }): Json<AddPerson>,
) -> Result<impl IntoResponse, VentError> {
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

    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_prefect", post(post_add_prefects_to_event))
        .route("/add_participant", post(post_add_participants_to_event))
}
