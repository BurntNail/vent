use crate::{
    error::{IOAction, IOSnafu, ParseTimeSnafu, SqlxAction, SqlxSnafu, VentError},
    routes::FormEvent,
    state::VentState,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use http::StatusCode;
use serde::Deserialize;
use snafu::ResultExt;
use tokio::fs::remove_file;

#[axum::debug_handler]
async fn post_update_event(
    Path(event_id): Path<i32>,
    State(state): State<VentState>,
    Json(FormEvent {
        name,
        date,
        location,
        teacher,
        info,
    }): Json<FormEvent>,
) -> Result<impl IntoResponse, VentError> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M").context(ParseTimeSnafu {
        original: date.clone(),
    })?;

    sqlx::query!(
        r#"
UPDATE public.events
SET event_name=$2, date=$3, location=$4, teacher=$5, other_info=$6
WHERE id=$1
        "#,
        event_id,
        name,
        date,
        location,
        teacher,
        info
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingEvent(event_id),
    })?;

    state.update_events()?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct Removal {
    relation_id: i32,
}

#[axum::debug_handler]
async fn post_remove_prefect_from_event(
    State(state): State<VentState>,
    Json(Removal { relation_id }): Json<Removal>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        r#"
DELETE FROM prefect_events WHERE relation_id = $1 
"#,
        relation_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::RemovingPrefectOrPrefectFromEventByRI { relation_id },
    })?;

    state.update_events()?;

    Ok(StatusCode::OK)
}
#[axum::debug_handler]
async fn get_remove_participant_from_event(
    State(state): State<VentState>,
    Json(Removal { relation_id }): Json<Removal>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        r#"
    DELETE FROM participant_events WHERE relation_id = $1 
    "#,
        relation_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::RemovingPrefectOrPrefectFromEventByRI { relation_id },
    })?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn delete_image(
    Path(img_id): Path<i32>,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let event = sqlx::query!(
        r#"
DELETE FROM public.photos
WHERE id=$1
RETURNING path, event_id"#,
        img_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::RemovingPhoto(img_id),
    })?;

    if let Some(existing_zip_file) = sqlx::query!(
        r#"
SELECT zip_file
FROM events
WHERE id = $1"#,
        event.event_id
    )
    .fetch_one(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingEvent(event.event_id),
    })?
    .zip_file
    {
        sqlx::query!(
            r#"
    UPDATE events
    SET zip_file = NULL
    WHERE id = $1"#,
            event.event_id
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::UpdatingEvent(event.event_id),
        })?;

        remove_file(&existing_zip_file).await.context(IOSnafu {
            action: IOAction::DeletingFile(existing_zip_file.into()),
        })?;
    }

    remove_file(&event.path).await.context(IOSnafu {
        action: IOAction::DeletingFile(event.path.into()),
    })?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct VerifyPerson {
    event_id: i32,
    person_id: i32,
}

#[axum::debug_handler]
async fn post_verify_person(
    State(state): State<VentState>,
    Json(VerifyPerson {
        event_id,
        person_id,
    }): Json<VerifyPerson>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!("UPDATE participant_events SET is_verified = true WHERE event_id = $1 AND participant_id = $2", event_id, person_id).execute(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::UpdatingParticipantOrPrefect {person: person_id.into(), event_id} })?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn post_unverify_person(
    State(state): State<VentState>,
    Json(VerifyPerson {
        event_id,
        person_id,
    }): Json<VerifyPerson>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!("UPDATE participant_events SET is_verified = false WHERE event_id = $1 AND participant_id = $2", event_id, person_id).execute(&mut *state.get_connection().await?).await.context(SqlxSnafu { action: SqlxAction::UpdatingParticipantOrPrefect {person: person_id.into(), event_id} })?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct VerifyEveryone {
    event_id: i32,
}

#[axum::debug_handler]
async fn post_verify_everyone(
    State(state): State<VentState>,
    Json(VerifyEveryone { event_id }): Json<VerifyEveryone>,
) -> Result<impl IntoResponse, VentError> {
    sqlx::query!(
        "UPDATE participant_events SET is_verified = true WHERE event_id = $1",
        event_id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::MassVerifying { event_id },
    })?;
    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/update_event/:id", post(post_update_event))
        .route("/verify_all", post(post_verify_everyone))
        .route("/verify_participant", post(post_verify_person))
        .route("/unverify_participant", post(post_unverify_person))
        .route(
            "/remove_prefect_from_event",
            post(post_remove_prefect_from_event),
        )
        .route(
            "/remove_participant_from_event",
            post(get_remove_participant_from_event),
        )
        .route("/remove_img/:id", get(delete_image))
}
