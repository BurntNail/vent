use crate::{
    auth::{
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, Auth,
    },
    error::{VentError, SerdeJsonAction, SerdeJsonSnafu, SqlxAction, SqlxSnafu},
    liquid_utils::compile,
    state::{db_objects::DbPerson, VentState},
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::verify;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

#[derive(Deserialize)]
pub struct LoginDetails {
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
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let html = compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": get_auth_object(auth), "was_password_related": was_password_related }),
        &state.settings.brand.instance_name
    )
    .await?;

    Ok((was_password_related.status_code(), html).into_response())
}

#[axum::debug_handler]
pub async fn post_login(
    mut auth: Auth,
    State(state): State<VentState>,
    remote_ip: GrabCFRemoteIP,
    Form(LoginDetails {
        username,
        unhashed_password,
        cf_turnstile_response,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, VentError> {
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
    .await.context(SqlxSnafu {action: SqlxAction::FindingPerson(username.into())})?;

    let Some(db_user) = db_user else {
        return Ok(Redirect::to("/login_failure/user_not_found"));
    };

    Ok(match &db_user.hashed_password {
        //some of the code below looks weird as otherwise borrow not living long enough
        Some(pw) => Redirect::to(if verify(unhashed_password, pw)? {
            auth.login(&db_user).await.context(SerdeJsonSnafu {
                action: SerdeJsonAction::TryingToLogin,
            })?;
            "/"
        } else {
            error!(username=?db_user.username, "Wrong password for trying to login");
            "/login_failure/bad_password"
        }),
        None => {
            state.reset_password(db_user.id).await?;

            Redirect::to("/add_password")
        }
    })
}

#[axum::debug_handler]
pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, VentError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}
