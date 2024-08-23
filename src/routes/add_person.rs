//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    routes::FormPerson,
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
    Form, Router,
};
use axum_login::permission_required;
use snafu::ResultExt;

///`GET` function to display the add person form
#[axum::debug_handler]
async fn get_add_person(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;

    state.compile(
        "www/add_person.liquid",
        liquid::object!({"auth": aa}),
        Some("New Person".into()),
    )
    .await
}

#[axum::debug_handler]
async fn post_add_person(
    State(state): State<VentState>,
    Form(FormPerson {
        first_name,
        surname,
        username,
        form,
        permissions,
    }): Form<FormPerson>,
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

    Ok(Redirect::to("/add_person"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/add_person", get(get_add_person).post(post_add_person))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
}
