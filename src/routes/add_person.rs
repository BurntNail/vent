//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    auth::{
        backend::{Auth, KnotAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{KnotError, SqlxAction, SqlxSnafu},
    liquid_utils::compile_with_newtitle,
    routes::FormPerson,
    state::KnotState,
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
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let aa = get_auth_object(auth).await?;

    compile_with_newtitle(
        "www/add_person.liquid",
        liquid::object!({"auth": aa}),
        &state.settings.brand.instance_name,
        Some("New Person".into()),
    )
    .await
}

#[axum::debug_handler]
async fn post_add_person(
    State(state): State<KnotState>,
    Form(FormPerson {
        first_name,
        surname,
        username,
        form,
        permissions,
    }): Form<FormPerson>,
) -> Result<impl IntoResponse, KnotError> {
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

pub fn router() -> Router<KnotState> {
    Router::new()
        .route("/add_person", get(get_add_person).post(post_add_person))
        .route_layer(permission_required!(
            KnotAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
}
