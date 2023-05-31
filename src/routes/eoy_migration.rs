use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use itertools::Itertools;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile,
};

pub async fn get_eoy_migration(
    auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let forms: Vec<String> = sqlx::query!(r#"SELECT form FROM people"#)
        .fetch_all(pool.as_ref())
        .await?
        .into_iter()
        .map(|r| r.form)
        .unique()
        .collect();

    compile(
        "www/eoy_migration.liquid",
        liquid::object!({
            "auth": get_auth_object(auth),
            "forms": forms
        }),
    )
    .await
}

#[derive(Deserialize)]
pub struct FormNameChange {
    pub old_name: String,
    pub new_name: String,
}

pub async fn post_eoy_migration(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(FormNameChange { old_name, new_name }): Form<FormNameChange>,
) -> Result<impl IntoResponse, KnotError> {
    sqlx::query!(
        r#"
UPDATE public.people
SET form = $2
WHERE form = $1
"#,
        old_name,
        new_name
    )
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to("/eoy_migration"))
}
