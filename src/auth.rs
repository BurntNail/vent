pub mod cloudflare_turnstile;
pub mod pg_session;

use self::cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP};
use crate::{error::KnotError, liquid_utils::compile, routes::DbPerson, state::KnotState};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use axum_login::{
    extractors::AuthContext, secrecy::SecretVec, AuthUser, PostgresStore, RequireAuthorizationLayer,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use serde::{Deserialize, Serialize};

#[derive(
    sqlx::Type, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Debug,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum PermissionsRole {
    Participant,
    Prefect,
    Admin,
    Dev,
}

impl AuthUser<i32, PermissionsRole> for DbPerson {
    fn get_id(&self) -> i32 {
        self.id
    }

    fn get_password_hash(&self) -> axum_login::secrecy::SecretVec<u8> {
        let Some(hp) = self.hashed_password.clone() else {
            error!(?self, "Missing Password");
            panic!("Missing Password!");
        };
        SecretVec::new(hp.into())
    }

    fn get_role(&self) -> Option<PermissionsRole> {
        Some(self.permissions)
    }
}

pub type Auth = AuthContext<i32, DbPerson, Store, PermissionsRole>;
pub type RequireAuth = RequireAuthorizationLayer<i32, DbPerson, PermissionsRole>;
pub type Store = PostgresStore<DbPerson, PermissionsRole>;

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

#[derive(Serialize, Deserialize)]
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
}

pub async fn get_login_failure(
    auth: Auth,
    Path(was_password_related): Path<FailureReason>,
) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/failed_auth.liquid",
        liquid::object!({ "auth": get_auth_object(auth), "was_password_related": was_password_related }),
    )
    .await
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
    verify_turnstile(cf_turnstile_response, remote_ip).await?;

    let db_user = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _" 
FROM people 
WHERE username = $1
        "#,
        username
    )
    .fetch_optional(&mut state.get_connection().await?)
    .await?;

    let Some(db_user) = db_user else {
        return Ok(Redirect::to("/login_failure/user_not_found"))
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

//tried to use an Option<Path<_>>, but didn't work
pub async fn get_blank_add_password(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_password.liquid",
        liquid::object!({
            "is_authing_user": false,
            "auth": get_auth_object(auth),
        }),
    )
    .await
}

pub async fn get_add_password(
    auth: Auth,
    State(state): State<KnotState>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, KnotError> {
    if sqlx::query!("SELECT password_link_id FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await?
        .password_link_id
        .is_none()
    {
        return Ok(Redirect::to("/login_failure/no_numbers").into_response());
    }
    if sqlx::query!("SELECT hashed_password FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await?
        .hashed_password
        .is_some()
    {
        return Ok(Redirect::to("/login_failure/password_already_set").into_response());
    }

    let person = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _" 
FROM people 
WHERE id = $1"#,
        id
    )
    .fetch_one(&mut state.get_connection().await?)
    .await?;

    Ok(compile(
        "www/add_password.liquid",
        liquid::object!({
            "is_authing_user": true,
            "person": person,
            "auth": get_auth_object(auth)
        }),
    )
    .await?
    .into_response())
}

#[derive(Deserialize)]
pub struct AddPasswordForm {
    pub id: i32,
    pub unhashed_password: String,
    pub password_link_id: i32,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
}

pub async fn post_add_password(
    mut auth: Auth,
    State(state): State<KnotState>,
    remote_ip: GrabCFRemoteIP,
    Form(AddPasswordForm {
        id,
        unhashed_password,
        password_link_id,
        cf_turnstile_response,
    }): Form<AddPasswordForm>,
) -> Result<impl IntoResponse, KnotError> {
    verify_turnstile(cf_turnstile_response, remote_ip).await?;

    if sqlx::query!("SELECT hashed_password FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await?
        .hashed_password
        .is_some()
    {
        return Ok(Redirect::to("/login_failure/password_already_set"));
    }

    let expected = sqlx::query!("SELECT password_link_id FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await?
        .password_link_id;
    let Some(expected) = expected else {
        return Ok(Redirect::to("/login_failure/no_numbers"));
    };
    if expected != password_link_id {
        return Ok(Redirect::to("/login_failure/failed_numbers"));
    };

    //from here, we assume we're all good

    let hashed = hash(&unhashed_password, DEFAULT_COST)?;
    let person: DbPerson = sqlx::query_as!(
        DbPerson,
        r#"
UPDATE people
SET hashed_password = $1
WHERE id = $2
RETURNING id, first_name, surname, username, form, hashed_password, permissions as "permissions: _" 
    "#,
        hashed,
        id
    )
    .fetch_one(&mut state.get_connection().await?)
    .await?;

    auth.login(&person).await?;

    Ok(Redirect::to("/"))
}

pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, KnotError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}

pub fn get_auth_object(auth: Auth) -> liquid::Object {
    if let Some(user) = auth.current_user {
        let perms = liquid::object!({
            "dev_access": user.permissions >= PermissionsRole::Dev,
            "import_csv": user.permissions >= PermissionsRole::Admin,
            "run_migrations": user.permissions >= PermissionsRole::Admin,
            "edit_people": user.permissions >= PermissionsRole::Admin,
            "edit_events": user.permissions >= PermissionsRole::Prefect,
            "add_photos": user.permissions >= PermissionsRole::Prefect,
            "edit_prefects_on_events": user.permissions >= PermissionsRole::Prefect,
            "edit_participants_on_events": user.permissions >= PermissionsRole::Participant,
            "see_photos": user.permissions >= PermissionsRole::Participant,
            "export_csv": user.permissions >= PermissionsRole::Participant,
        });

        liquid::object!({ "role": user.permissions, "permissions": perms, "user": user })
    } else {
        let perms = liquid::object!({
            "dev_access": false,
            "import_csv": false,
            "run_migrations": false,
            "edit_people": false,
            "edit_events": false,
            "add_photos": false,
            "edit_prefects_on_events": false,
            "edit_participants_on_events": false,
            "see_photos": false,
            "export_csv": false,
        });

        liquid::object!({"role": "visitor", "permissions": perms })
    }
}
