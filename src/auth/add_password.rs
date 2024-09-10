use crate::{
    auth::{
        backend::{Auth, VentAuthBackend},
        cloudflare_turnstile::{verify_turnstile, GrabCFRemoteIP},
        get_auth_object, PermissionsTarget,
    },
    error::{SqlxAction, SqlxSnafu, VentError},
    state::{db_objects::DbPerson, mail::EmailToSend, VentState},
};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
    routing::get,
    Form, Router,
};
use axum_login::permission_required;
use bcrypt::{hash, DEFAULT_COST};
use itertools::Itertools;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{pool::PoolConnection, Postgres};
use std::time::Duration;
use tokio::time::sleep;

//tried to use an Option<Path<_>>, but didn't work
#[axum::debug_handler]
async fn get_blank_add_password(
    auth: Auth,
    State(state): State<VentState>,
) -> Result<impl IntoResponse, VentError> {
    let aa = get_auth_object(auth).await?;
    state
        .compile(
            "www/add_password.liquid",
            liquid::object!({
                "is_authing_user": false,
                "auth": aa,
            }),
            None,
        )
        .await
}

#[derive(Debug, Deserialize)]
struct Link {
    code: i32,
}

#[axum::debug_handler]
async fn get_add_password(
    auth: Auth,
    State(state): State<VentState>,
    Path(id): Path<i32>,
    Query(Link { code: link_thingie }): Query<Link>,
) -> Result<impl IntoResponse, VentError> {
    let correct_code = sqlx::query!("SELECT password_link_id FROM people WHERE id = $1", id)
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPerson(id.into()),
        })?
        .password_link_id;

    let Some(correct_code) = correct_code else {
        return Ok(Redirect::to("/login_failure/no_numbers").into_response());
    };
    if correct_code != link_thingie {
        return Ok(Redirect::to("/login_failure/failed_numbers").into_response());
    };

    if sqlx::query!("SELECT hashed_password FROM people WHERE id = $1", id)
        .fetch_one(&mut *state.get_connection().await?)
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
    .fetch_one(&mut *state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::FindingPerson(id.into()) })?;

    let aa = get_auth_object(auth).await?;

    Ok(state
        .compile(
            "www/add_password.liquid",
            liquid::object!({
                "is_authing_user": true,
                "person": person,
                "auth": aa,
                "link_id": link_thingie
            }),
            None,
        )
        .await?
        .into_response())
}

#[derive(Deserialize)]
struct AddPasswordForm {
    pub id: i32,
    pub unhashed_password: String,
    pub password_link_id: i32,
    #[serde(rename = "cf-turnstile-response")]
    pub cf_turnstile_response: String,
}

#[axum::debug_handler]
async fn post_add_password(
    mut auth: Auth,
    State(state): State<VentState>,
    remote_ip: GrabCFRemoteIP,
    Form(AddPasswordForm {
        id,
        unhashed_password,
        password_link_id,
        cf_turnstile_response,
    }): Form<AddPasswordForm>,
) -> Result<impl IntoResponse, VentError> {
    if !verify_turnstile(cf_turnstile_response, remote_ip).await? {
        return Ok(Redirect::to("/login_failure/failed_turnstile"));
    }

    if sqlx::query!("SELECT hashed_password FROM people WHERE id = $1", id)
        .fetch_one(&mut *state.get_connection().await?)
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
        .fetch_one(&mut *state.get_connection().await?)
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
    .fetch_one(&mut *state.get_connection().await?)
    .await.context(SqlxSnafu { action: SqlxAction::UpdatingPerson(id.into()) })?;

    auth.login(&(person.into())).await?;

    Ok(Redirect::to("/"))
}

pub async fn get_email_to_be_sent_for_reset_password(
    mut connection: PoolConnection<Postgres>,
    user_id: i32,
) -> Result<EmailToSend, VentError> {
    let current_ids =
        sqlx::query!(r#"SELECT password_link_id FROM people WHERE password_link_id <> NULL"#)
            .fetch_all(&mut *connection)
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
    .execute(&mut *connection)
    .await
    .context(SqlxSnafu {
        action: SqlxAction::UpdatingPerson(id.into()),
    })?;

    let person = sqlx::query!(
        "SELECT username, first_name, surname FROM people WHERE id = $1",
        user_id
    )
    .fetch_one(&mut *connection)
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

pub async fn spam_password_emails(State(state): State<VentState>) -> Result<Redirect, VentError> {
    let ids: Vec<_> = sqlx::query!("SELECT id FROM people WHERE hashed_password IS NULL")
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(SqlxSnafu {
            action: SqlxAction::FindingPeople,
        })?;

    tokio::spawn(async move {
        for id in ids {
            let id = id.id;
            sleep(Duration::from_secs(60 * 5)).await;
            if let Err(e) = state.reset_password(id).await {
                error!(?e, ?id, "Error resetting password");
            }
        }
    });

    Ok(Redirect::to("/"))
}

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/all_passwords", get(spam_password_emails))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::EditPeople
        ))
        .route("/add_password", get(get_blank_add_password))
        .route(
            "/add_password/:user_id",
            get(get_add_password).post(post_add_password),
        )
}
