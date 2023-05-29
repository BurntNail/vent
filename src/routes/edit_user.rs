use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{
    auth::{get_auth_object, Auth, LoginDetails, cloudflare_turnstile::{GrabCFRemoteIP, turnstile_verified}},
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
    remote_ip: GrabCFRemoteIP,
    auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(LoginDetails {
        username,
        unhashed_password,
        cf_turnstile_response,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    if !turnstile_verified(cf_turnstile_response, remote_ip).await? {
        return Err(KnotError::FailedTurnstile);
    }

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
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to("/"))
}
