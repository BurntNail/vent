use config::{Config, ConfigError, File};
use dotenvy::var;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::task::spawn_blocking;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub date_time_format: String,
    pub instance_name: String,
    pub tech_support: Url,
    pub username: String,
    pub domain: String,
    pub mail: MailSettings,
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
                "Unable to get CONFIG_LOCATION - defaulting to config/local.yml"
            );
            "config/local.yml".to_string()
        });

        let builder = Config::builder()
            .set_default("date_time_format", "%c")?
            .set_default("instance_name", "House Events Manager")?
            .set_default("tech_support", "https://google.com")?;

        spawn_blocking(move || {
            builder
                .add_source(File::from(PathBuf::from(file_name)))
                .build()
                .and_then(|config| config.try_deserialize::<Settings>())
        })
        .await
        .expect("unable to join spawn_blocking thread")
    }
}
