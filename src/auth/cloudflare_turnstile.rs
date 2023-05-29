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

#[derive(Deserialize)]
pub enum TurnstileError {
    MissingInputSecret,
    InvalidInputSecret,
    MissingInputResponse,
    InvalidInputResponse,
    InvalidWidgetID,
    InvalidParsedSecret,
    BadRequest,
    TimeoutOrDuplicate,
    InternalError
}

#[derive(Deserialize, Debug)]
struct TurnstileResponse {
    pub success: bool,
    pub challenge_ts: String,
    pub hostname: String,
    #[serde(rename = "error-codes")]
    pub error_codes: Vec<TurnstileError>,
    pub action: String,
    pub cdata: String,
}

pub async fn turnstile_verified(
    cf_turnstile_response: String,
    GrabCFRemoteIP(remote_ip): GrabCFRemoteIP,
) -> Result<bool, KnotError> {
    if cfg!(debug_assertions) {
        return Ok(true);
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
        .json::<TurnstileResponse>().await?;

    info!(?post_response, "Got response from CF");

    Ok(post_response.success)
}
