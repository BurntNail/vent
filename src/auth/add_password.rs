use crate::{
    auth::{
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, Auth,
    },
    error::KnotError,
    liquid_utils::compile,
    routes::DbPerson,
    state::KnotState,
};
use axum::{
    extract::{Path, State, Query},
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use serde::Deserialize;

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
    Query(link_thingie): Query<i32>
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
            "auth": get_auth_object(auth),
            "link_id": link_thingie
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
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

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
