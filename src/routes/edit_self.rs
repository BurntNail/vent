use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile_with_newtitle,
    state::KnotState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use serde::Deserialize;

pub async fn get_edit_user(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    compile_with_newtitle(
        "../../www/edit_self.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
        &state.settings.brand.instance_name,
        Some("Edit Profile".into())
    )
    .await
}

#[derive(Deserialize)]
pub struct LoginDetails {
    pub first_name: String,
    pub surname: String,
    pub unhashed_password: String,
}

#[instrument(
    level = "debug",
    skip(auth, state, first_name, surname, unhashed_password)
)]
pub async fn post_edit_user(
    auth: Auth,
    State(state): State<KnotState>,
    Form(LoginDetails {
        first_name,
        surname,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let current_id = auth.current_user.unwrap().id;

    debug!(%current_id, "Hashing password");

    let hashed = hash(&unhashed_password, DEFAULT_COST)?;

    debug!("Updating in DB");

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
    .execute(&mut state.get_connection().await?)
    .await?;

    Ok(Redirect::to("/"))
}
