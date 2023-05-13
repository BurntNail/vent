use std::sync::Arc;

use axum::{extract::{Path, State}, response::{IntoResponse, Redirect}};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use crate::{error::KnotError, liquid_utils::compile, routes::{DbEvent, Person}};

pub async fn get_update_event (Path(event_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let DbEvent {
        id,
        event_name,
        date,
        location,
        teacher,
        other_info,
    } = sqlx::query_as!(
        DbEvent,
        r#"
SELECT * FROM events WHERE id = $1
"#,
        event_id
    ).fetch_one(&mut conn).await?;
    let date = date.to_string();

    #[derive(Deserialize, Serialize, Debug)]
    struct PersonPlusRelID {
        pub id: i32,
        pub person_name: String,
        pub relation_id: i32
    }

    let existing_prefects = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id, p.id
FROM people p
INNER JOIN prefect_events pe ON pe.event_id = $1 AND pe.prefect_id = p.id"#,
    event_id
    ).fetch_all(&mut conn).await?;

    let existing_participants = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id, p.id
FROM people p
INNER JOIN participant_events pe ON pe.event_id = $1 AND pe.participant_id = p.id"#,
    event_id
    ).fetch_all(&mut conn).await?;


    let possible_prefects = sqlx::query_as!(
        Person,
        r#"
SELECT p.id, p.person_name, p.is_prefect
FROM people p
WHERE p.is_prefect = true"#
    ).fetch_all(&mut conn).await?.into_iter().filter(|p| !existing_prefects.iter().any(|e| e.id == p.id)).collect::<Vec<_>>();
    let possible_participants = sqlx::query_as!(
        Person,
        r#"
SELECT p.id, p.person_name, p.is_prefect
FROM people p
"#
    ).fetch_all(&mut conn).await?.into_iter().filter(|p| !existing_participants.iter().any(|e| e.id == p.id)).collect::<Vec<_>>();



    let globals = liquid::object!({
        "event": liquid::object!({
            "id": id,
            "event_name": event_name,
            "date": date.to_string(),
            "location": location,
            "teacher": teacher,
            "other_info": other_info.unwrap_or_default()
        }),
        "existing_prefects": existing_prefects,
        "existing_participants": existing_participants,
        "prefects": possible_prefects,
        "participants": possible_participants
    });
    compile("www/update_event.liquid", globals).await
}

pub async fn get_remove_prefect_from_event (Path(relation_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let id = sqlx::query!(
        r#"
DELETE FROM prefect_events WHERE relation_id = $1 
RETURNING event_id
"#, relation_id
    ).fetch_one(&mut conn).await?.event_id;

    Ok(Redirect::to(&format!("/update_event/{id}")))
}
pub async fn get_remove_participant_from_event (Path(relation_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let id = sqlx::query!(
        r#"
DELETE FROM participant_events WHERE relation_id = $1 
RETURNING event_id
"#, relation_id
    ).fetch_one(&mut conn).await?.event_id;

    Ok(Redirect::to(&format!("/update_event/{id}")))
}