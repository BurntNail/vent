use axum::{extract::State, response::{IntoResponse}};
use itertools::Itertools;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{auth::{Auth, get_auth_object}, error::KnotError, liquid_utils::compile};

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

    compile("www/eoy_migration.liquid", liquid::object!({
        "auth": get_auth_object(auth),
        "forms": forms
    })).await
}
