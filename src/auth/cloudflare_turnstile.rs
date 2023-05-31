use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderValue},
    response::Html,
};
use once_cell::sync::Lazy;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::{collections::HashMap, env::var};

use crate::error::{get_error_page, KnotError};

static CFT_SECRETKEY: Lazy<String> =
    Lazy::new(|| var("CFT_SECRETKEY").expect("missing environment variable `CFT_SECRETKEY`"));

pub struct GrabCFRemoteIP(HeaderValue);

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for GrabCFRemoteIP {
    type Rejection = (StatusCode, Html<String>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if cfg!(debug_assertions) {
            return Ok(Self(HeaderValue::from_static("127.0.0.1")));
        }

        if let Some(cfrip) = parts.headers.get("CF-Connecting-IP") {
            Ok(Self(cfrip.clone()))
        } else {
            Err(get_error_page(
                StatusCode::FORBIDDEN,
                "Missing Cloudflare IP.",
            ))
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum TurnstileError {
    #[serde(rename = "missing-input-secret")]
    MissingInputSecret,
    #[serde(rename = "invalid-input-secret")]
    InvalidInputSecret,
    #[serde(rename = "missing-input-response")]
    MissingInputResponse,
    #[serde(rename = "invalid-input-secret")]
    InvalidInputResponse,
    #[serde(rename = "invalid-widget-id")]
    InvalidWidgetID,
    #[serde(rename = "missing-parsed-secret")]
    InvalidParsedSecret,
    #[serde(rename = "bad-request")]
    BadRequest,
    #[serde(rename = "timeout-or-duplicate")]
    TimeoutOrDuplicate,
    #[serde(rename = "internal")]
    InternalError,
}

#[derive(Deserialize, Debug)]
struct TurnstileResponse {
    pub success: bool,
    pub challenge_ts: Option<String>,
    pub hostname: Option<String>,
    #[serde(rename = "error-codes")]
    pub error_codes: Vec<TurnstileError>,
    pub action: Option<String>,
    pub cdata: Option<String>,
}

#[instrument]
pub async fn verify_turnstile(
    cf_turnstile_response: String,
    GrabCFRemoteIP(remote_ip): GrabCFRemoteIP,
) -> Result<(), KnotError> {
    if cfg!(debug_assertions) {
        return Ok(());
    }

    let mut body = HashMap::new();
    body.insert("secret", CFT_SECRETKEY.as_str());
    body.insert("response", &cf_turnstile_response);
    body.insert("remoteip", remote_ip.to_str()?);

    let post_response = Client::new()
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<TurnstileResponse>()
        .await?;

    trace!(?post_response.hostname, ?post_response.cdata, ?post_response.action, ?post_response.challenge_ts, "Got CFT response");

    if post_response.success {
        return Ok(());
    }

    if !post_response.error_codes.is_empty() {
        error!(?post_response.error_codes, "CFT Response Error");
    }

    Err(KnotError::FailedTurnstile)
}
