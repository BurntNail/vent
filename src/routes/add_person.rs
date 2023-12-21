//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    error::{VentError, SqlxAction, SqlxSnafu},
    routes::FormPerson,
    state::VentState,
};
use axum::{extract::State, response::{IntoResponse}, routing::post, Router, Json};
use http::StatusCode;
use snafu::ResultExt;

#[axum::debug_handler]
async fn post_add_person(
    State(state): State<VentState>,
    Json(FormPerson {
        first_name,
        surname,
        username,
        form,
        permissions,
    }): Json<FormPerson>,
) -> Result<impl IntoResponse, VentError> {
    info!("Inserting new person into DB");
    sqlx::query!(
        r#"
INSERT INTO public.people
(permissions, first_name, surname, username, form)
VALUES($1, $2, $3, $4, $5);    
    "#,
        permissions as _,
        first_name,
        surname,
        username,
        form,
    )
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingPerson,
    })?;

    Ok(StatusCode::OK)
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_person", post(post_add_person))
}
