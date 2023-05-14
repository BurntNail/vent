use std::sync::Arc;

use crate::{error::KnotError};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
};
use sqlx::{Pool, Postgres};

///`GET` method that acts like a `POST` method that adds a prefect associated with the given `prefect_id` (seconnd argument) to a given `event_id` (first argument). Ensures to avoid duplicates.
pub async fn get_add_prefect_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Path((event_id, prefect_id)): Path<(i32, i32)>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?; //get a connection

    if sqlx::query!(
        r#"
SELECT * FROM public.prefect_events
WHERE prefect_id = $1
AND event_id = $2"#,
        prefect_id,
        event_id
    )
    .fetch_optional(&mut conn)
    .await?
    .is_none() //if we can't find anything
    {
        sqlx::query!( //add away
            r#"
INSERT INTO public.prefect_events
(prefect_id, event_id)
VALUES($1, $2);            
            "#,
            prefect_id,
            event_id
        )
        .execute(&mut conn)
        .await?;
    }
    
    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}

///`GET` method that acts like a `POST` method that adds a prefect associated with the given `participant_id` (seconnd argument) to a given `event_id` (first argument). Ensures to avoid duplicates.
pub async fn get_add_participant_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Path((event_id, participant_id)): Path<(i32, i32)>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?; //get db connection

    if sqlx::query!(
        r#"
SELECT * FROM public.participant_events
WHERE participant_id = $1
AND event_id = $2"#,
    participant_id,
        event_id
    )
    .fetch_optional(&mut conn)
    .await?
    .is_none() //if we can't find anything
    {
        sqlx::query!( //add away
            r#"
INSERT INTO public.participant_events
(participant_id, event_id)
VALUES($1, $2);            
            "#,
            participant_id,
            event_id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to(&format!("/update_event/{event_id}")))
}
