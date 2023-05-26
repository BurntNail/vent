use std::sync::Arc;

use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::{compile, EnvFormatter},
};

use super::DbPerson;

#[derive(Deserialize)]
pub struct SmolDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: NaiveDateTime,
}
#[derive(Serialize)]
pub struct SmolFormattedDbEvent {
    pub id: i32,
    pub event_name: String,
    pub date: String,
}

impl From<SmolDbEvent> for SmolFormattedDbEvent {
    fn from(
        SmolDbEvent {
            id,
            event_name,
            date,
        }: SmolDbEvent,
    ) -> Self {
        Self {
            id,
            event_name,
            date: date.to_env_string(),
        }
    }
}

pub async fn get_remove_stuff(
    auth: Auth,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let mut people: Vec<DbPerson> = sqlx::query_as!(
        DbPerson,
        r#"
SELECT *
FROM people p
        "#
    )
    .fetch_all(&mut conn)
    .await?;
    people.sort_by_key(|x| x.surname.clone());
    people.sort_by_key(|x| x.form.clone());

    let events: Vec<SmolFormattedDbEvent> = sqlx::query_as!(
        SmolDbEvent,
        r#"
SELECT id, event_name, date
FROM events e
ORDER BY e.date
        "#
    )
    .fetch_all(&mut conn)
    .await?
    .into_iter()
    .map(SmolFormattedDbEvent::from)
    .collect();

    compile(
        "www/show_all.liquid",
        liquid::object!({ "people": people, "events": events, "auth": get_auth_object(auth) }),
    )
    .await
}

#[derive(Deserialize)]
pub struct RemovePerson {
    pub person_id: Vec<i32>,
}

#[derive(Deserialize)]
pub struct RemoveEvent {
    pub event_id: Vec<i32>,
}

pub async fn post_remove_person(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(RemovePerson { person_id }): Form<RemovePerson>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    for person_id in person_id {
        sqlx::query!(
            r#"
DELETE FROM public.people
WHERE id=$1
            "#,
            person_id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to("/show_all"))
}
pub async fn post_remove_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(RemoveEvent { event_id }): Form<RemoveEvent>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    for event_id in event_id {
        sqlx::query!(
            r#"
    DELETE FROM public.events
    WHERE id=$1
            "#,
            event_id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to("/show_all"))
}
