use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object,
    },
    error::{ALError, VentError, LoginFailureReason},
    liquid_utils::compile,
    state::VentState,
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Form, Router,
};
use axum_login::login_required;
use http::StatusCode;
use serde::{Deserialize, Serialize};

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
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;
    compile(
        "www/login.liquid",
        liquid::object!({ "auth": aa }),
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
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;
    let html = compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": aa, "was_password_related": was_password_related }),
        &state.settings.brand.instance_name,
    )
    .await?;

    Ok((was_password_related.status_code(), html).into_response())
}

#[derive(Clone)]
pub struct LoginCreds {
    pub username: String,
    pub unhashed_password: String,
}

#[axum::debug_handler]
pub async fn post_login(
    mut auth: Auth,
    remote_ip: GrabCFRemoteIP,
    Form(LoginForm {
        username,
        unhashed_password,
        cf_turnstile_response,
    }): Form<LoginForm>,
) -> Result<impl IntoResponse, VentError> {
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

    Ok(Redirect::to(
        match auth
            .authenticate(LoginCreds {
                username: username.clone(),
                unhashed_password,
            })
            .await
        {
            Ok(Some(x)) => {
                auth.login(&x).await?;
                "/"
            }
            Ok(None) => "/login_failure/user_not_found",
            Err(error) => {
                if let ALError::Backend(VentError::LoginFailure { reason }) = error {
                    match reason {
                        LoginFailureReason::PasswordIsNotSet => "/add_password",
                        LoginFailureReason::IncorrectPassword => {
                            error!(username = ? username, "Wrong password for trying to login");
                            "/login_failure/bad_password"
                        }
                    }
                } else {
                    return Err(error.into());
                }
            }
        },
    ))
}

#[axum::debug_handler]
pub async fn get_logout(mut auth: Auth) -> Result<impl IntoResponse, VentError> {
    auth.logout()?;
    Ok(Redirect::to("/"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/logout", get(get_logout))
        .route_layer(login_required!(VentAuthBackend, login_url = "/login"))
        .route("/login", get(get_login).post(post_login))
        .route(
            "/login_failure/:was_password_related",
            get(get_login_failure),
        )
}
