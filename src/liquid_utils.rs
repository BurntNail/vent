use crate::{error::KnotError, liquid_utils::partials::PARTIALS};
use axum::response::Html;
use chrono::NaiveDateTime;
use liquid::{model::Value, Object, ParserBuilder};
use once_cell::sync::Lazy;
use std::{env::var, fmt::Debug, path::Path};
use tokio::fs::read_to_string;
use tracing::Instrument;

pub mod partials;

pub static CFT_SITEKEY: Lazy<String> =
    Lazy::new(|| var("CFT_SITEKEY").expect("missing environment variable `CFT_SITEKEY`"));
pub static DOMAIN: Lazy<(bool, String)> = Lazy::new(|| {
    if let Ok(dom) = var("DOMAIN") {
        (true, dom)
    } else {
        (false, String::new())
    }
});

#[instrument(level = "debug", skip(globals))]
pub async fn compile_with_newtitle(
    path: impl AsRef<Path> + Debug,
    mut globals: Object,
    project_name: &str,
    title_additional_info: Option<String>,
) -> Result<Html<String>, KnotError> {
    let (liquid, partial_compiler) = async move {
        debug!("Reading in file + partials");
        (
            read_to_string(path).await,
            PARTIALS.read().await.to_compiler(),
        )
    }
    .instrument(debug_span!("compile_preparations"))
    .await;
    let liquid = liquid?;

    debug!("Inserting globals");

    let title = match title_additional_info {
        None => project_name.to_string(),
        Some(x) => x.to_string(),
    };

    globals.insert("cft_sitekey".into(), Value::scalar(CFT_SITEKEY.as_str()));
    globals.insert(
        "siteinfo".into(),
        Value::Object(liquid::object!({
            "instance_name": project_name,
            "html_title": title,
            "domain_exists": DOMAIN.0,
            "domain": DOMAIN.1.as_str()
        })),
    );

    let html = tokio::task::spawn_blocking(move || {
        debug!("Compiling");
        ParserBuilder::with_stdlib()
            .partials(partial_compiler)
            .build()?
            .parse(&liquid)?
            .render(&globals)
    })
    .instrument(debug_span!("acc_compilation"))
    .await?;

    Ok(html?).map(Html)
}

#[instrument(level = "debug", skip(globals))]
pub async fn compile(
    path: impl AsRef<Path> + Debug,
    globals: Object,
    project_name: &str,
) -> Result<Html<String>, KnotError> {
    compile_with_newtitle(path, globals, project_name, None).await
}

pub trait EnvFormatter {
    fn to_env_string(&self, format: &str) -> String;
}
impl EnvFormatter for NaiveDateTime {
    fn to_env_string(&self, format: &str) -> String {
        self.format(format).to_string()
    }
}
