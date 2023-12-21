use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};
use axum::{extract::State, response::{IntoResponse}, Router, Json};
use axum::routing::post;
use bcrypt::{hash, DEFAULT_COST};
use http::StatusCode;
use serde::Deserialize;
use snafu::ResultExt;


#[derive(Deserialize)]
pub struct LoginDetails {
    pub id: i32,
    pub unhashed_password: String,
}
#[axum::debug_handler]
pub async fn post_edit_password(
    State(state): State<VentState>,
    Json(LoginDetails {
        id,
        unhashed_password,
    }): Json<LoginDetails>,
) -> Result<impl IntoResponse, VentError> {
    let hashed = hash(&unhashed_password, DEFAULT_COST)?;

    sqlx::query!(
        r#"
UPDATE people
SET hashed_password=$1
WHERE id=$2
        "#,
        hashed,
        id
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingPerson(id.into()),
    })?;

    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/edit_password", post(post_edit_password))
}
