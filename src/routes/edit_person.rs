use crate::{
    error::{SqlxAction, SqlxSnafu, VentError},
    routes::FormPerson,
    state::VentState,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use http::StatusCode;
use snafu::ResultExt;

#[axum::debug_handler]
async fn post_edit_person(
    Path(id): Path<i32>,
    State(state): State<VentState>,
    Json(FormPerson {
        first_name,
        surname,
        form,
        username,
        permissions,
    }): Json<FormPerson>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Editing person");
    sqlx::query!(
        r#"
UPDATE public.people
SET permissions=$6, first_name=$2, surname=$3, form=$4, username=$5
WHERE id=$1
        "#,
        id,
        first_name,
        surname,
        form,
        username,
        permissions as _
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingPerson(id.into()),
    })?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn post_reset_password(
    State(state): State<VentState>,
    Json(id): Json<i32>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Logging out.");

    debug!("Sending password reset");
    state.reset_password(id).await?;
    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/edit_person/:id", post(post_edit_person))
        .route("/reset_password", post(post_reset_password))
}
