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

#[derive(Deserialize, Clone, FromRow, Serialize)]
pub struct DbUser {
    id: i32,
    username: String,
    hashed_password: String,
}

impl AuthUser<i32, ()> for DbUser {
    fn get_id(&self) -> i32 {
        self.id
    }

    fn get_password_hash(&self) -> axum_login::secrecy::SecretVec<u8> {
        SecretVec::new(self.hashed_password.clone().into())
    }
}

pub type Auth = AuthContext<i32, DbUser, Store>;
pub type RequireAuth = RequireAuthorizationLayer<i32, DbUser>;
pub type Store = PostgresStore<DbUser, ()>;

#[derive(Deserialize)]
pub struct LoginDetails {
    username: String,
    unhashed_password: String,
}

pub async fn get_login() -> Result<impl IntoResponse, KnotError> {
    compile("www/login.liquid", liquid::object!({})).await
}

pub async fn get_login_failure() -> Result<impl IntoResponse, KnotError> {
    compile("www/failed_auth.liquid", liquid::object!({})).await
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
    let db_user = sqlx::query_as!(DbUser, "SELECT * FROM users WHERE username = $1", username)
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

pub async fn get_add_new_user(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    let globals = if let Some(user) = auth.current_user {
        liquid::object!({ "is_logged_in": true, "user": user })
    } else {
        liquid::object!({ "is_logged_in": false })
    };

    compile("www/add_new_user.liquid", globals).await
}
