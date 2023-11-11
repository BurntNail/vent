use crate::{
    auth::{
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, Auth,
    },
    error::{KnotError, SerdeJsonAction, SerdeJsonSnafu, SqlxAction, SqlxSnafu},
    liquid_utils::compile,
    state::{db_objects::DbPerson, KnotState},
};
use axum::{extract::{Path, State}, response::{IntoResponse, Redirect}, Form, Router};
use bcrypt::verify;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use crate::error::LoginFailureReason;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub unhashed_password: String,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
}

#[axum::debug_handler]
pub async fn get_login(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/login.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
        &state.settings.brand.instance_name,
    )
    .await
}

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum FailureReason {
    #[serde(rename = "bad_password")]
    BadPassword,
    #[serde(rename = "no_numbers")]
    NoNumbers,
    #[serde(rename = "user_not_found")]
    UserNotFound,
    #[serde(rename = "failed_numbers")]
    FailedNumbers,
    #[serde(rename = "password_already_set")]
    PasswordAlreadySet,
    #[serde(rename = "failed_turnstile")]
    FailedTurnstile,
}

impl FailureReason {
    pub fn status_code(self) -> StatusCode {
        match self {
            Self::NoNumbers | Self::PasswordAlreadySet => StatusCode::BAD_REQUEST,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::FailedTurnstile | Self::FailedNumbers | Self::BadPassword => {
                StatusCode::FORBIDDEN
            }
        }
    }
}

#[axum::debug_handler]
pub async fn get_login_failure(
    auth: Auth,
    Path(was_password_related): Path<FailureReason>,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let html = compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": get_auth_object(auth), "was_password_related": was_password_related }),
        &state.settings.brand.instance_name
    )
    .await?;

    Ok((was_password_related.status_code(), html).into_response())
}

pub struct LoginCreds {
    pub username: String,
    pub unhashed_password: String,
}

#[axum::debug_handler]
pub async fn post_login(
    mut auth: Auth,
    State(state): State<KnotState>,
    remote_ip: GrabCFRemoteIP,
    Form(LoginForm {
             username, unhashed_password, cf_turnstile_response
         }): Form<LoginForm>,
) -> Result<impl IntoResponse, KnotError> {
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

    Ok(Redirect::to(match auth.authenticate(LoginCreds {username: username.clone(), unhashed_password}).await {
        Ok(Some(x)) => {
            auth.login(&x);
            "/"
        },
        Ok(None) => {
            "/login_failure/user_not_found"
        },
        Err(KnotError::LoginFailure {reason}) => match reason {
            LoginFailureReason::PasswordIsNotSet => "/add_password",
            LoginFailureReason::IncorrectPassword => {
                error!(username=?username, "Wrong password for trying to login");
                "/login_failure/bad_password"
            }
        },
        Err(e) => Err(e)?,
    }))
}

#[axum::debug_handler]
pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, KnotError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}

pub fn router () -> Router<KnotState> {
    Router::new()
        .
}
