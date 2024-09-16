use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object,
    },
    error::{ALError, LoginFailureReason, SqlxAction, SqlxSnafu, VentError},
    state::VentState,
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Form, Router,
};
use axum::extract::Query;
use axum_login::login_required;
use bcrypt::{hash, DEFAULT_COST};
use http::StatusCode;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

#[derive(Debug, Deserialize)]
pub struct NextUrl {
    next: Option<String>
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub unhashed_password: String,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
    next: Option<String>,
}

#[axum::debug_handler]
pub async fn get_login(
    auth: Auth,
    State(state): State<VentState>,
    Query(NextUrl {next}): Query<NextUrl>,
) -> Result<impl IntoResponse, VentError> {
    if sqlx::query!("SELECT COUNT(*) FROM people")
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?
        .count
        .unwrap_or_default()
        == 0
    {
        let password = {
            const OPTIONS: &[u8] = b"qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXVBNM123456789!$%^&*()-=[];#,._+{}:@<>?";
            const LEN: usize = 24;

            let mut rng = thread_rng();
            let mut string = String::with_capacity(LEN);

            for _ in 0..LEN {
                let index = rng.gen_range(0..OPTIONS.len());
                string.push(char::from(OPTIONS[index]));
            }

            string
        };

        const USERNAME: &str = "admin";

        println!("Created admin user with password {password:?}");

        let hashed = hash(&password, DEFAULT_COST)?;
        sqlx::query!(
            r#"
INSERT INTO public.people
(permissions, first_name, surname, username, form, hashed_password)
VALUES('dev', 'Admin', 'Admin', $1, 'Staff', $2);
        "#,
            USERNAME,
            hashed
        )
        .execute(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::AddingPerson,
        })?;
    }

    let aa = get_auth_object(auth).await?;
    let next = match next {
        Some(x) => liquid::object!({"next_exists": true, "next": x}),
        None => liquid::object!({"next_exists": false}),
    };
    state.compile(
        "www/login.liquid",
        liquid::object!({ "auth": aa, "tech_support_person": state.settings.tech_support_person.clone(), "next": next}),
        None
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
    let html = state
        .compile(
            "www/failed_auth.liquid",
            liquid::object!({ "auth": aa, "was_password_related": was_password_related }),
            None,
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
        next
    }): Form<LoginForm>,
) -> Result<impl IntoResponse, VentError> {
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

    Ok(Redirect::to(&
        match auth
            .authenticate(LoginCreds {
                username: username.clone(),
                unhashed_password,
            })
            .await
        {
            Ok(Some(x)) => {
                auth.login(&x).await?;
                next.unwrap_or_else(|| "/".to_string())
            }
            Ok(None) => "/login_failure/user_not_found".to_string(),
            Err(error) => {
                if let ALError::Backend(VentError::LoginFailure { reason }) = error {
                    match reason {
                        LoginFailureReason::PasswordIsNotSet => "/add_password",
                        LoginFailureReason::IncorrectPassword => {
                            error!(username = ? username, "Wrong password for trying to login");
                            "/login_failure/bad_password"
                        }
                    }.to_string()
                } else {
                    return Err(error.into());
                }
            }
        },
    ))
}

#[axum::debug_handler]
pub async fn get_logout(mut auth: Auth) -> Result<impl IntoResponse, VentError> {
    auth.logout().await?;
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
