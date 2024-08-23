use std::env::var;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;
use axum::response::Html;
use liquid::model::Value;
use liquid::{Object, ParserBuilder};
use once_cell::sync::Lazy;
use snafu::ResultExt;
use tokio::sync::Mutex;
use crate::cfg::Settings;
use crate::error::{JoinSnafu, LiquidAction, LiquidSnafu, ThreadReason, VentError};
use crate::liquid_utils::partials::PARTIALS;
use crate::state::cache::VentCache;

pub static CFT_SITEKEY: Lazy<String> =
    Lazy::new(|| var("CFT_SITEKEY").expect("missing environment variable `CFT_SITEKEY`"));
pub static DOMAIN: Lazy<(bool, String)> = Lazy::new(|| {
    if let Ok(dom) = var("DOMAIN") {
        (true, dom)
    } else {
        (false, String::new())
    }
});

#[derive(Debug, Clone)]
pub struct VentCompiler;

impl VentCompiler {
    pub async fn compile_with_newtitle(
        &self, //not used rn, but maybe in the future (???)
        path: impl AsRef<Path> + Debug,
        mut globals: Object,
        title_additional_info: Option<String>,
        settings: &Settings,
        cache: Arc<Mutex<VentCache>>
    ) -> Result<Html<String>, VentError> {
        debug!("Reading in file + partials");

        let liquid = {
            let mut cache = cache.lock().await;
            cache.get(path).await?.to_string()
        };
        let partial_compiler = PARTIALS.read().await.to_compiler();

        debug!("Inserting globals");

        let project_name = settings.brand.instance_name.clone();
        let title = match title_additional_info {
            None => project_name.clone(),
            Some(x) => x.to_string(),
        };

        let show_bonus_points = var("HIDE_BONUS_POINTS").is_err();
        let show_different_awards = var("DISABLE_DIFFERENT_AWARD_THRESHOLDS").is_err();

        globals.insert("cft_sitekey".into(), Value::scalar(CFT_SITEKEY.as_str()));
        globals.insert(
            "siteinfo".into(),
            Value::Object(liquid::object!({
            "instance_name": project_name,
            "html_title": title,
            "domain_exists": DOMAIN.0,
            "domain": DOMAIN.1.as_str(),
            "show_bonus_points": show_bonus_points,
            "show_different_awards": show_different_awards
        })),
        );

        let html: Result<String, VentError> = tokio::task::spawn_blocking(move || {
            debug!("Compiling");
            let res = ParserBuilder::with_stdlib()
                .partials(partial_compiler)
                .build()
                .context(LiquidSnafu {
                    attempt: LiquidAction::BuildingCompiler,
                })?
                .parse(&liquid)
                .with_context(|_e| LiquidSnafu {
                    attempt: LiquidAction::Parsing { text: liquid },
                })?
                .render(&globals)
                .context(LiquidSnafu {
                    attempt: LiquidAction::Rendering,
                })?;
            Ok(res)
        })
            .await
            .context(JoinSnafu {
                title: ThreadReason::LiquidCompiler,
            })?;

        Ok(Html(html?))
    }
}