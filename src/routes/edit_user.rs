use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{
    auth::{get_auth_object, Auth, LoginDetails},
    error::KnotError,
    liquid_utils::compile,
};

pub async fn get_edit_user(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/edit_user.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
}

pub async fn post_edit_user(
    auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(LoginDetails {
        username,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let current_id = auth.current_user.unwrap().id;
    let hashed = hash(&unhashed_password, DEFAULT_COST)?;

    sqlx::query!(
        r#"
UPDATE public.users
SET username=$1, hashed_password=$2
WHERE id=$3;
        "#,
        username,
        hashed,
        current_id
    )
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to("/"))
}
