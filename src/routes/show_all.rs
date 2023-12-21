use crate::{
    error::{SqlxAction, SqlxSnafu, VentError},
    state::VentState,
};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::hint::unreachable_unchecked;

async fn get_all_event_ids(State(state): State<VentState>) -> Result<impl IntoResponse, VentError> {
    let ids: Vec<i32> = sqlx::query!("SELECT id from events")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingAllEvents,
        })?
        .into_iter()
        .map(|x| x.id)
        .collect();

    Ok(Json(ids))
}

async fn get_all_person_ids(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let ids: Vec<i32> = sqlx::query!("SELECT id from people")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingAllEvents,
        })?
        .into_iter()
        .map(|x| x.id)
        .collect();

    Ok(Json(ids))
}

#[derive(Serialize, Debug)]
pub struct ParticipantEventRelation {
    pub participant_id: i32,
    pub event_id: i32,
    pub is_verified: bool,
}

#[derive(Serialize, Debug)]
pub struct SmolPerson {
    pub first_name: String,
    pub surname: String,
    pub form: String,
    pub id: i32,
    pub pts: usize,
    pub events: Vec<ParticipantEventRelation>,
}

async fn get_person(
    State(state): State<VentState>,
    Json(id): Json<i32>,
) -> Result<impl IntoResponse, VentError> {
    let person = sqlx::query!(
        r#"
SELECT first_name, surname, form, id, (SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true) as pts
FROM people p
WHERE id = $1
        "#,
        id
    )
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?;

    let events = sqlx::query_as!(ParticipantEventRelation, "SELECT participant_id, event_id, is_verified FROM participant_events WHERE participant_id = $1", id).fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::FindingEventsOnPeople {person: id.into()}})?;

    let pts = {
        let pts = person.pts.unwrap_or_default();
        if pts < 0 {
            unsafe { unreachable_unchecked() }
        }
        pts as usize
    };

    Ok(Json(SmolPerson {
        first_name: person.first_name,
        surname: person.surname,
        form: person.form,
        id,
        pts,
        events,
    }))
}

#[derive(Serialize, Debug)]
pub struct SmolEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: Option<String>,
    pub participants: Vec<ParticipantEventRelation>,
    pub prefects: Vec<i32>,
    pub photos: Vec<i32>,
}

async fn get_event(
    State(state): State<VentState>,
    Json(id): Json<i32>,
) -> Result<impl IntoResponse, VentError> {
    let event = sqlx::query!(
        r#"
SELECT event_name, date, location, teacher, other_info, id
FROM events
WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingEvent(id),
    })?;

    let participants = sqlx::query_as!(
        ParticipantEventRelation,
        "SELECT participant_id, event_id, is_verified FROM participant_events WHERE event_id = $1",
        id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingParticipantsOrPrefectsAtEvents { event_id: Some(id) },
    })?;
    let prefects: Vec<i32> = sqlx::query!(
        "SELECT prefect_id FROM prefect_events WHERE event_id = $1",
        id
    )
    .fetch_all(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingParticipantsOrPrefectsAtEvents { event_id: Some(id) },
    })?
    .into_iter()
    .map(|x| x.prefect_id)
    .collect();
    let photos: Vec<i32> = sqlx::query!("SELECT id FROM photos WHERE event_id = $1", id)
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPhotos(id.into()),
        })?
        .into_iter()
        .map(|x| x.id)
        .collect();

    Ok(Json(SmolEvent {
        id,
        event_name: event.event_name,
        date: event.date,
        location: event.location,
        teacher: event.teacher,
        other_info: event.other_info,
        participants,
        prefects,
        photos,
    }))
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
    Json(RemovePerson { person_id }): Json<RemovePerson>,
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

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn post_remove_event(
    State(state): State<VentState>,
    Json(RemoveEvent { event_id }): Json<RemoveEvent>,
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

    Ok(StatusCode::OK)
}

//TODO: Get photo

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/remove_person", post(post_remove_person))
        .route("/remove_event", post(post_remove_event))
        .route("/get_all_events", get(get_all_event_ids))
        .route("/get_all_people", get(get_all_person_ids))
        .route("/get_person", get(get_person))
        .route("/get_event", get(get_event))
}
