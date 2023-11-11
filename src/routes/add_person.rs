//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    auth::{get_auth_object, Auth, PermissionsRole},
    error::{VentError, SqlxAction, SqlxSnafu},
    liquid_utils::compile_with_newtitle,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use snafu::ResultExt;

///`GET` function to display the add person form
#[instrument(level = "debug", skip(auth))]
#[axum::debug_handler]
pub async fn get_add_person(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    compile_with_newtitle(
        "www/add_person.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
        &state.settings.brand.instance_name,
        Some("New Person".into()),
    )
    .await
}

#[derive(Deserialize)]
pub struct NoIDPerson {
    pub first_name: String,
    pub surname: String,
    pub username: String,
    pub form: Option<String>,
    pub permissions: PermissionsRole,
}

#[instrument(level = "info", skip(state, first_name, surname))]
#[axum::debug_handler]
pub async fn post_add_person(
    State(state): State<VentState>,
    Form(NoIDPerson {
        first_name,
        surname,
        username,
        form,
        permissions,
    }): Form<NoIDPerson>,
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
    .execute(&mut state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::AddingPerson,
    })?;

    Ok(Redirect::to("/add_person"))
}
