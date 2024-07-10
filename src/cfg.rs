use config::{Config, ConfigError, File};
use dotenvy::var;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::task::spawn_blocking;

#[derive(Debug, Deserialize, Clone)]
pub struct BrandSettings {
    pub instance_name: String,
    pub domain: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NicheSettings {
    pub date_time_format: String,
    pub tech_support: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub niche: NicheSettings,
    pub brand: BrandSettings,
    pub mail: MailSettings,
    pub timezone_id: String,
    pub tech_support_person: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MailSettings {
    pub username: String,
    pub password: String,
    pub smtp: String,
    pub username_domain: String,
}

impl Settings {
    pub async fn new() -> Result<Self, ConfigError> {
        let file_name = var("CONFIG_LOCATION").unwrap_or_else(|e| {
            error!(
                ?e,
                "Unable to get CONFIG_LOCATION - defaulting to config/local.toml"
            );
            "config/local.toml".to_string()
        });

        let builder = Config::builder()
            .set_default("date_time_format", "%c")?
            .set_default("instance_name", "House Events Manager")?
            .set_default("tech_support", "https://google.com")?
            .set_default("timezone_id", "Europe/London")?;

        spawn_blocking(move || {
            builder
                .add_source(File::from(PathBuf::from(file_name)))
                .build()
                .and_then(Config::try_deserialize)
        })
        .await
        .expect("unable to join spawn_blocking thread")
    }
}
