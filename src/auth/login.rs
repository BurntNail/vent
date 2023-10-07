use crate::{
    auth::{
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, Auth,
    },
    error::KnotError,
    liquid_utils::compile,
    routes::DbPerson,
    state::KnotState,
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::verify;
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LoginDetails {
    pub username: String,
    pub unhashed_password: String,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
}

pub async fn get_login(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/login.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
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

pub async fn get_login_failure(
    auth: Auth,
    Path(was_password_related): Path<FailureReason>,
) -> Result<impl IntoResponse, KnotError> {
    let html = compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": get_auth_object(auth), "was_password_related": was_password_related }),
    )
    .await?;

    Ok((was_password_related.status_code(), html).into_response())
}

pub async fn post_login(
    mut auth: Auth,
    State(state): State<KnotState>,
    remote_ip: GrabCFRemoteIP,
    Form(LoginDetails {
        username,
        unhashed_password,
        cf_turnstile_response,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

    let db_user = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people 
WHERE LOWER(username) = LOWER($1)
        "#,
        username
    )
    .fetch_optional(&mut state.get_connection().await?)
    .await?;

    let Some(db_user) = db_user else {
        return Ok(Redirect::to("/login_failure/user_not_found"));
    };

    Ok(match &db_user.hashed_password {
        //some of the code below looks weird as otherwise borrow not living long enough
        Some(pw) => Redirect::to(if verify(unhashed_password, pw)? {
            auth.login(&db_user).await?;
            "/"
        } else {
            error!("USER FAILED TO LOGIN!!!");
            "/login_failure/bad_password"
        }),
        None => {
            state.reset_password(db_user.id).await?;

            Redirect::to("/add_password")
        }
    })
}

pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, KnotError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}
