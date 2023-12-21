use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};
use axum::{extract::State, response::{IntoResponse, Redirect}, routing::{get, post}, Router, Json};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;


async fn get_all_event_ids (State(state): State<VentState>) -> Result<impl IntoResponse, VentError> {
    let ids: Vec<i32> = sqlx::query!("SELECT id from events").fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })?.into_iter().map(|x| x.id).collect();

    Ok(Json(ids))
}

async fn get_all_person_ids(State(state): State<VentState>) -> Result<impl IntoResponse, VentError> {
    let ids: Vec<i32> = sqlx::query!("SELECT id from people").fetch_all(&mut *state.get_connection().await?).await.context(SqlxSnafu {
        action: SqlxAction::FindingAllEvents,
    })?.into_iter().map(|x| x.id).collect();

    Ok(Json(ids))
}

#[derive(Serialize)]
pub struct SmolPerson {
    pub first_name: String,
    pub surname: String,
    pub form: String,
    pub id: i32,
    pub pts: Option<i64>,
}

async fn get_person (State(state): State<VentState>, Json(id): Json<i32>) -> Result<impl IntoResponse, VentError> {
    let person = sqlx::query_as!(
        SmolPerson,
        r#"
SELECT first_name, surname, form, id, (SELECT COUNT(participant_id) FROM participant_events WHERE participant_id = $1 AND is_verified = true) as pts
FROM people p
WHERE id = $1
        "#,
        id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?;

    Ok(Json(person))
}


#[derive(Serialize)]
pub struct SmolEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub other_info: Option<String>
}

async fn get_event (State(state): State<VentState>, Json(id): Json<i32>) -> Result<impl IntoResponse, VentError> {
    let event = sqlx::query_as!(
        SmolEvent,
        r#"
SELECT event_name, date, location, teacher, other_info, id
FROM events
WHERE id = $1
        "#,
        id
    )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingEvent(id),
        })?;

    Ok(Json(event))
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

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/remove_person", post(post_remove_person))
        .route("/remove_event", post(post_remove_event))
        .route("/get_all_events", get(get_all_event_ids))
        .route("/get_all_people", get(get_all_person_ids))
        .route("/get_person", get(get_person))
        .route("/get_event", get(get_event))
}
