use crate::{
    auth::{
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, Auth,
    },
    error::{KnotError, SerdeJsonAction, SerdeJsonSnafu, SqlxAction, SqlxSnafu},
    liquid_utils::compile,
    routes::DbPerson,
    state::{mail::EmailToSend, KnotState},
};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
    Form,
};
use bcrypt::{hash, DEFAULT_COST};
use itertools::Itertools;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Postgres};

//tried to use an Option<Path<_>>, but didn't work
#[axum::debug_handler]
pub async fn get_blank_add_password(
    auth: Auth,
    State(state): State<KnotState>,
) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_password.liquid",
        liquid::object!({
            "is_authing_user": false,
            "auth": get_auth_object(auth),
        }),
        &state.settings.brand.instance_name,
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct Link {
    code: i32,
}

#[axum::debug_handler]
pub async fn get_add_password(
    auth: Auth,
    State(state): State<KnotState>,
    Path(id): Path<i32>,
    Query(Link { code: link_thingie }): Query<Link>,
) -> Result<impl IntoResponse, KnotError> {
    if sqlx::query!("SELECT password_link_id FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?
        .password_link_id
        .is_none()
    {
        return Ok(Redirect::to("/login_failure/no_numbers").into_response());
    }
    if sqlx::query!("SELECT hashed_password FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?
        .hashed_password
        .is_some()
    {
        return Ok(Redirect::to("/login_failure/password_already_set").into_response());
    }

    let person = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
FROM people 
WHERE id = $1"#,
        id
    )
    .fetch_one(&mut state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::FindingPerson(id.into()) })?;

    Ok(compile(
        "www/add_password.liquid",
        liquid::object!({
            "is_authing_user": true,
            "person": person,
            "auth": get_auth_object(auth),
            "link_id": link_thingie
        }),
        &state.settings.brand.instance_name,
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

#[axum::debug_handler]
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
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?
        .hashed_password
        .is_some()
    {
        return Ok(Redirect::to("/login_failure/password_already_set"));
    }

    let expected = sqlx::query!("SELECT password_link_id FROM people WHERE id = $1", id)
        .fetch_one(&mut state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?
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
RETURNING id, first_name, surname, username, form, hashed_password, permissions as "permissions: _", was_first_entry
    "#,
        hashed,
        id
    )
    .fetch_one(&mut state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::UpdatingPerson(id.into()) })?;

    auth.login(&person).await.context(SerdeJsonSnafu {
        action: SerdeJsonAction::TryingToLogin,
    })?;

    Ok(Redirect::to("/"))
}

pub async fn get_email_to_be_sent_for_reset_password(
    mut connection: PoolConnection<Postgres>,
    user_id: i32,
) -> Result<EmailToSend, KnotError> {
    let current_ids =
        sqlx::query!(r#"SELECT password_link_id FROM people WHERE password_link_id <> NULL"#)
            .fetch_all(&mut connection)
            .await
            .context(SqlxSnafu {
                action: SqlxAction::FindingPerson(user_id.into()),
            })?
            .into_iter()
            .map(|x| x.password_link_id.unwrap()) //we check for null above so fine
            .collect_vec();

    let id: i32 = {
        let mut rng = thread_rng();
        let mut tester = rng.gen::<u16>();
        while current_ids.contains(&(tester.into())) {
            tester = rng.gen::<u16>();
        }
        tester
    }
    .into(); //ensure always positive

    sqlx::query!(
        "UPDATE people SET password_link_id = $1, hashed_password = NULL WHERE id = $2",
        id,
        user_id
    )
    .execute(&mut connection)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingPerson(id.into()),
    })?;

    let person = sqlx::query!(
        "SELECT username, first_name, surname FROM people WHERE id = $1",
        user_id
    )
    .fetch_one(&mut connection)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::FindingPerson(id.into()),
    })?;

    Ok(EmailToSend {
        to_username: person.username,
        to_id: user_id,
        to_fullname: format!("{} {}", person.first_name, person.surname),
        unique_id: id,
    })
}
