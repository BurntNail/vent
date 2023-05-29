use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Serialize;
use sqlx::{Pool, Postgres};

use crate::{
    auth::{get_auth_object, Auth, cloudflare_turnstile::{GrabCFRemoteIP, turnstile_verified}},
    error::KnotError,
    liquid_utils::{compile, EnvFormatter},
    routes::DbPerson,
};

use super::add_person::NoIDPerson;

pub async fn get_edit_person(
    auth: Auth,
    Path(id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let person = sqlx::query_as!(DbPerson, r#"SELECT * FROM people WHERE id = $1"#, id)
        .fetch_one(pool.as_ref())
        .await?;

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
    remote_ip: GrabCFRemoteIP,
    Path(id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(NoIDPerson {
        first_name,
        surname,
        form,
        is_prefect,
        cf_turnstile_response
    }): Form<NoIDPerson>,
) -> Result<impl IntoResponse, KnotError> {
    if !turnstile_verified(cf_turnstile_response, remote_ip).await? {
        return Err(KnotError::FailedTurnstile);
    }

    sqlx::query!(
        r#"
UPDATE public.people
SET is_prefect=$5, first_name=$2, surname=$3, form=$4
WHERE id=$1
        "#,
        id,
        first_name,
        surname,
        form,
        is_prefect
    )
    .execute(pool.as_ref())
    .await?;

    Ok(Redirect::to(&format!("/edit_person/{id}")))
}
