use std::sync::Arc;

use axum::{extract::{Path, State}, response::{IntoResponse, Redirect}};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{error::KnotError, liquid_utils::compile, routes::DbEvent};

use super::index;

pub async fn get_update_event (Path(event_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let DbEvent {
        id: _,
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
        pub person_name: String,
        pub relation_id: i32
    }

    let prefects = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id
FROM people p
INNER JOIN prefect_events pe ON pe.event_id = $1 AND pe.prefect_id = p.id "#,
    event_id
    ).fetch_all(&mut conn).await?;

    let participants = sqlx::query_as!(
        PersonPlusRelID,
        r#"
SELECT p.person_name, pe.relation_id
FROM people p
INNER JOIN participant_events pe ON pe.event_id = $1 AND pe.participant_id = p.id "#,
    event_id
    ).fetch_all(&mut conn).await?;

    let globals = liquid::object!({
        "event": liquid::object!({
            "event_name": event_name,
            "date": date.to_string(),
            "location": location,
            "teacher": teacher,
            "other_info": other_info.unwrap_or_default()
        }),
        "prefects": prefects,
        "participants": participants
    });
    compile("www/update_event.liquid", globals).await
}

pub async fn get_remove_prefect_from_event (Path(relation_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    sqlx::query!(
        r#"
DELETE FROM prefect_events WHERE relation_id = $1 
"#, relation_id
    ).execute(&mut conn).await?;

    Ok(Redirect::to(index::LOCATION))
}
pub async fn get_remove_participant_from_event (Path(relation_id): Path<i32>, State(pool): State<Arc<Pool<Postgres>>>) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    sqlx::query!(
        r#"
DELETE FROM participant_events WHERE relation_id = $1 
"#, relation_id
    ).execute(&mut conn).await?;

    Ok(Redirect::to(index::LOCATION))
}