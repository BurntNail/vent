use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use itertools::Itertools;
use serde::Deserialize;
use snafu::ResultExt;

use crate::{
    auth::{get_auth_object, Auth},
    error::{KnotError, SqlxAction, SqlxSnafu},
    liquid_utils::compile_with_newtitle,
    state::KnotState,
};

#[instrument(level = "debug", skip(auth, state))]
#[axum::debug_handler]
pub async fn get_eoy_migration(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    debug!("Getting all forms");

    let forms: Vec<String> = sqlx::query!(r#"SELECT form FROM people"#)
        .fetch_all(&mut state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .into_iter()
        .map(|r| r.form)
        .unique()
        .collect();

    debug!("Compiling");

    compile_with_newtitle(
        "www/eoy_migration.liquid",
        liquid::object!({
            "auth": get_auth_object(auth),
            "forms": forms
        }),
        &state.settings.brand.instance_name,
        Some("Migrating Forms".into()),
    )
    .await
}

#[derive(Deserialize)]
pub struct FormNameChange {
    pub old_name: String,
    pub new_name: String,
}

#[instrument(level = "debug", skip(state))]
#[axum::debug_handler]
pub async fn post_eoy_migration(
    State(state): State<KnotState>,
    Form(FormNameChange { old_name, new_name }): Form<FormNameChange>,
) -> Result<impl IntoResponse, KnotError> {
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
    .execute(&mut state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingForms { old_name, new_name },
    })?;

    Ok(Redirect::to("/eoy_migration"))
}
