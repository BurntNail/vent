use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub async fn get_edit_user(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/edit_user.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
}

#[derive(Deserialize)]
pub struct LoginDetails {
    pub first_name: String,
    pub surname: String,
    pub unhashed_password: String,
}

pub async fn post_edit_user(
    auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(LoginDetails {
        first_name,
        surname,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let current_id = auth.current_user.unwrap().id;
    let hashed = hash(&unhashed_password, DEFAULT_COST)?;

    sqlx::query!(
        r#"
UPDATE people
SET first_name=$1, surname = $2, hashed_password=$3
WHERE id=$4;
        "#,
        first_name,
        surname,
        hashed,
        current_id
    )
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to("/"))
}
