use crate::{
    auth::{
        backend::{Auth, KnotAuthBackend},
        get_auth_object,
    },
    error::{KnotError, SqlxAction, SqlxSnafu},
    liquid_utils::compile_with_newtitle,
    state::KnotState,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
    Form, Router,
};
use axum_login::login_required;
use bcrypt::{hash, DEFAULT_COST};
use serde::Deserialize;
use snafu::ResultExt;

#[axum::debug_handler]
pub async fn get_edit_user(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    let aa = get_auth_object(auth).await?;
    compile_with_newtitle(
        "www/edit_self.liquid",
        liquid::object!({"auth": aa}),
        &state.settings.brand.instance_name,
        Some("Edit Profile".into()),
    )
    .await
}

#[derive(Deserialize)]
pub struct LoginDetails {
    pub first_name: String,
    pub surname: String,
    pub unhashed_password: String,
}
#[axum::debug_handler]
pub async fn post_edit_user(
    auth: Auth,
    State(state): State<KnotState>,
    Form(LoginDetails {
        first_name,
        surname,
        unhashed_password,
    }): Form<LoginDetails>,
) -> Result<impl IntoResponse, KnotError> {
    let current_id = auth.user.unwrap().id;

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
    .execute(&mut *state.get_connection().await?)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingPerson(current_id.into()),
    })?;

    Ok(Redirect::to("/"))
}

pub fn router() -> Router<KnotState> {
    Router::new()
        .route("/edit_user", get(get_edit_user).post(post_edit_user))
        .route_layer(login_required!(KnotAuthBackend, login_url = "/login"))
}
