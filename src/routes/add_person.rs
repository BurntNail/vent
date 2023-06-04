//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    auth::{get_auth_object, Auth, PermissionsRole},
    error::KnotError,
    liquid_utils::compile, state::KnotState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;

///`GET` function to display the add person form
pub async fn get_add_person(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_person.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
}

#[derive(Deserialize)]
pub struct NoIDPerson {
    pub first_name: String,
    pub surname: String,
    pub form: Option<String>,
    pub permissions: PermissionsRole,
}

pub async fn post_add_person(
    State(state): State<KnotState>,
    Form(NoIDPerson {
        first_name,
        surname,
        form,
        permissions,
    }): Form<NoIDPerson>,
) -> Result<impl IntoResponse, KnotError> {
    sqlx::query!(
        r#"
INSERT INTO public.people
(permissions, first_name, surname, form)
VALUES($1, $2, $3, $4);    
    "#,
        permissions as _,
        first_name,
        surname,
        form,
    )
    .execute(&mut state.get_connection().await?)
    .await?;

    Ok(Redirect::to("/add_person"))
}
