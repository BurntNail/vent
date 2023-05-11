use crate::{
    error::KnotError,
    liquid_utils::partials::{init_partials, PARTIALS},
};
use axum::response::Html;
use liquid::{Object, ParserBuilder};
use std::{fmt::Debug, path::Path};
use tokio::fs::read_to_string;

pub mod partials;

#[instrument]
pub async fn compile(
    path: impl AsRef<Path> + Debug,
    globals: Object,
) -> Result<Html<String>, KnotError> {
    let liquid = read_to_string(path).await?;
    let partial_compiler = PARTIALS.get_or_init(init_partials).await.to_compiler();

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
