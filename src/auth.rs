use std::sync::Arc;

use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use axum_login::{
    extractors::AuthContext, secrecy::SecretVec, AuthUser, PostgresStore, RequireAuthorizationLayer,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Postgres};

use crate::{error::KnotError, liquid_utils::compile};

#[derive(sqlx::Type, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum PermissionsRole {
    Participant,
    Prefect,
    Admin,
    Dev,
}

#[derive(Deserialize, Clone, FromRow, Serialize)]
pub struct DbUser {
    pub id: i32,
    pub username: String,
    pub hashed_password: String,
    pub permissions: PermissionsRole,
}

impl AuthUser<i32, PermissionsRole> for DbUser {
    fn get_id(&self) -> i32 {
        self.id
    }

    fn get_password_hash(&self) -> axum_login::secrecy::SecretVec<u8> {
        SecretVec::new(self.hashed_password.clone().into())
    }

    fn get_role(&self) -> Option<PermissionsRole> {
        Some(self.permissions)
    }
}

pub type Auth = AuthContext<i32, DbUser, Store, PermissionsRole>;
pub type RequireAuth = RequireAuthorizationLayer<i32, DbUser, PermissionsRole>;
pub type Store = PostgresStore<DbUser, PermissionsRole>;

#[derive(Deserialize)]
pub struct LoginDetails {
    pub username: String,
    pub unhashed_password: String,
}

pub async fn get_login(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile("www/login.liquid", liquid::object!({"auth": get_auth_object(auth)})).await
}

pub async fn get_login_failure(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile("www/failed_auth.liquid", liquid::object!({"auth": get_auth_object(auth)})).await
}

pub async fn post_login(
    mut auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(LoginDetails {
        username,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;
    let db_user = sqlx::query_as!(DbUser, r#"SELECT id, username, hashed_password, permissions as "permissions: _" FROM users WHERE username = $1"#, username) //https://github.com/launchbadge/sqlx/issues/1004
        .fetch_one(&mut conn)
        .await?;
    Ok(Redirect::to(
        if verify(unhashed_password, &db_user.hashed_password)? {
            auth.login(&db_user).await?;
            "/"
        } else {
            error!(%username, "USER FAILED TO LOGIN!!!");
            "/login_failure"
        },
    ))
}

pub async fn post_logout(mut auth: Auth) -> Result<impl IntoResponse, KnotError> {
    auth.logout().await;
    Ok(Redirect::to("/"))
}

pub async fn post_add_new_user(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(LoginDetails {
        username: name,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let hashed = hash(&unhashed_password, DEFAULT_COST)?;
    sqlx::query!(
        r#"
INSERT INTO public.users
(username, hashed_password)
VALUES($1, $2);
    "#,
        name,
        hashed
    )
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to("/"))
}

pub fn get_auth_object(auth: Auth) -> liquid::Object {
    if let Some(user) = auth.current_user {
        let perms = liquid::object!({
            "dev_access": user.permissions >= PermissionsRole::Dev,
            "edit_users": user.permissions >= PermissionsRole::Admin,
            "edit_people": user.permissions >= PermissionsRole::Admin,
            "edit_events": user.permissions >= PermissionsRole::Prefect,
            "add_photos": user.permissions >= PermissionsRole::Prefect,
            "edit_prefects_on_events": user.permissions >= PermissionsRole::Prefect,
            "edit_participants_on_events": user.permissions >= PermissionsRole::Participant,
            "see_photos": user.permissions >= PermissionsRole::Participant,
        });

        liquid::object!({ "role": user.permissions, "permissions": perms, "user": user })
    } else {
        let perms = liquid::object!({
            "dev_access": false,
            "edit_users": false,
            "edit_people": false,
            "edit_events": false,
            "add_photos": false,
            "edit_prefects_on_events": false,
            "edit_participants_on_events": false,
            "see_photos": false,
        });

        liquid::object!({"role": "visitor", "permissions": perms })
    }
}

pub async fn get_add_new_user(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_new_user.liquid",
        liquid::object!({ "auth": get_auth_object(auth) }),
    )
    .await
}
