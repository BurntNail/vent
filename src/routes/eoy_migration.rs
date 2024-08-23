use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    state::VentState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::post,
    Form, Router,
};
use axum_login::permission_required;
use itertools::Itertools;
use serde::Deserialize;
use snafu::ResultExt;

#[axum::debug_handler]
async fn get_eoy_migration(
    auth: Auth,
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

    debug!("Compiling");

    let aa = get_auth_object(auth).await?;

    state
        .compile(
            "www/eoy_migration.liquid",
            liquid::object!({
                "auth": aa,
                "forms": forms
            }),
            Some("Migrating Forms".into()),
        )
        .await
}

#[derive(Deserialize)]
struct FormNameChange {
    pub old_name: String,
    pub new_name: String,
}

#[axum::debug_handler]
async fn post_eoy_migration(
    State(state): State<VentState>,
    Form(FormNameChange { old_name, new_name }): Form<FormNameChange>,
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

    Ok(Redirect::to("/eoy_migration"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route(
            "/eoy_migration",
            post(post_eoy_migration).get(get_eoy_migration),
        )
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
}
