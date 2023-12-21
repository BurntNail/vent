use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    state::VentState,
};
use axum::{extract::State, response::{IntoResponse}, routing::post, Router, Json};
use axum::routing::get;
use http::StatusCode;
use itertools::Itertools;
use serde::Deserialize;
use snafu::ResultExt;

#[axum::debug_handler]
async fn get_form_names(
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Getting all forms");

    let forms: Vec<String> = sqlx::query!(r#"SELECT form FROM people"#)
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .into_iter()
        .map(|r| r.form)
        .unique()
        .collect();

    Ok(Json(forms))
}

#[derive(Deserialize)]
struct FormNameChange {
    pub old_name: String,
    pub new_name: String,
}

#[axum::debug_handler]
async fn post_mass_change_form_name(
    State(state): State<VentState>,
    Json(FormNameChange { old_name, new_name }): Json<FormNameChange>,
) -> Result<impl IntoResponse, VentError> {
    debug!("Sending DB query");
    sqlx::query!(
        r#"
UPDATE public.people
SET form = $2
WHERE form = $1
"#,
        &old_name,
        &new_name
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingForms { old_name, new_name },
    })?;

    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route(
            "/form_names",
            get(get_form_names)
        )
        .route("/migrate_form_name", post(post_mass_change_form_name))
}
