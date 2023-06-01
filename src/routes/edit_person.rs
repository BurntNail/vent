use std::sync::Arc;

use crate::{
    auth::{get_auth_object, Auth, PermissionsRole},
    error::KnotError,
    liquid_utils::{compile, EnvFormatter},
    routes::DbPerson,
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use super::add_person::NoIDPerson;

pub async fn get_edit_person(
    auth: Auth,
    Path(id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    #[derive(Serialize)]
    pub struct SmolPerson {
        pub id: i32,
        pub permissions: PermissionsRole,
        pub first_name: String,
        pub surname: String,
        pub password_is_set: bool,
        pub form: String,
    }

    let person = sqlx::query_as!(
        DbPerson,
        r#"
SELECT id, first_name, surname, form, hashed_password, permissions as "permissions: _"
FROM people WHERE id = $1
        "#,
        id
    )
    .fetch_one(pool.as_ref())
    .await?;
    let person = SmolPerson {
        id: person.id,
        permissions: person.permissions,
        first_name: person.first_name,
        surname: person.surname,
        form: person.form,
        password_is_set: person.hashed_password.is_some(),
    };

    #[derive(Serialize)]
    struct Event {
        name: String,
        date: String,
        id: i32,
    }

    let events_supervised = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e 
INNER JOIN prefect_events pe
ON pe.event_id = e.id AND pe.prefect_id = $1
        "#,
        person.id
    )
    .fetch_all(pool.as_ref())
    .await?
    .into_iter()
    .map(|r| Event {
        name: r.event_name,
        date: r.date.to_env_string(),
        id: r.id,
    })
    .collect::<Vec<_>>();

    let events_participated = sqlx::query!(
        r#"
SELECT date, event_name, id FROM events e 
INNER JOIN participant_events pe
ON pe.event_id = e.id AND pe.participant_id = $1
        "#,
        person.id
    )
    .fetch_all(pool.as_ref())
    .await?
    .into_iter()
    .map(|r| Event {
        name: r.event_name,
        date: r.date.to_env_string(),
        id: r.id,
    })
    .collect::<Vec<_>>();

    compile("www/edit_person.liquid", liquid::object!({ "person": person, "supervised": events_supervised, "participated": events_participated, "auth": get_auth_object(auth) })).await
}

pub async fn post_edit_person(
    Path(id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(NoIDPerson {
        first_name,
        surname,
        form,
        permissions,
    }): Form<NoIDPerson>,
) -> Result<impl IntoResponse, KnotError> {
    sqlx::query!(
        r#"
UPDATE public.people
SET permissions=$5, first_name=$2, surname=$3, form=$4
WHERE id=$1
        "#,
        id,
        first_name,
        surname,
        form,
        permissions as _
    )
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to(&format!("/edit_person/{id}")))
}

#[derive(Deserialize)]
pub struct PasswordReset {
    id: i32,
}

pub async fn post_reset_password(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(PasswordReset { id }): Form<PasswordReset>,
) -> Result<impl IntoResponse, KnotError> {
    sqlx::query!(
        r#"
UPDATE public.people
SET hashed_password = NULL
WHERE id=$1
        "#,
        id
    )
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to("/show_all"))
}
