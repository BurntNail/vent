use crate::{
    error::KnotError,
    liquid_utils::partials::{PARTIALS}, PROJECT_NAME,
};
use axum::response::Html;
use chrono::NaiveDateTime;
use liquid::{Object, ParserBuilder, model::Value};
use std::{env, fmt::Debug, path::Path};
use tokio::fs::read_to_string;

pub mod partials;

#[instrument]
pub async fn compile(
    path: impl AsRef<Path> + Debug,
    mut globals: Object,
) -> Result<Html<String>, KnotError> {
    let liquid = read_to_string(path).await?;
    let partial_compiler = PARTIALS.read().await.to_compiler();

    globals.insert("instance_name".into(), Value::scalar(&PROJECT_NAME));

    Ok(tokio::task::spawn_blocking(move || {
        ParserBuilder::with_stdlib()
            .partials(partial_compiler)
            .build()?
            .parse(&liquid)?
            .render(&globals)
    })
    .await??)
    .map(Html)
}

pub trait EnvFormatter {
    fn to_env_string(&self) -> String;
}
impl EnvFormatter for NaiveDateTime {
    fn to_env_string(&self) -> String {
        self.format(&env::var("DATE_TIME_FORMAT").unwrap_or_else(|e| {
            warn!(%e, "Missing DATE_TIME_FORMAT");
            "%c".into()
        }))
        .to_string()
    }
}
