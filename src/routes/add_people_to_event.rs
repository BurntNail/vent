//! Module that publishes 2 `POST` methods that deal with adding prefects and participants to events based off of path parameters. This is a fair bit easier than an invisible form.

use std::sync::Arc;
use crate::error::KnotError;
use axum::{
    extract::{State},
    response::{IntoResponse, Redirect},
};
use axum::extract::Form;
use sqlx::{Pool, Postgres};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AddPerson {
	person_id: i32,
	event_id: i32
}

///`POST` method that adds a prefect to an event
#[axum::debug_handler]
pub async fn post_add_prefect_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(AddPerson {event_id, person_id: prefect_id}): Form<AddPerson>,
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
    .is_none() //if we can't find anything assoiated with this prefect and this event
    {
        //then we add the prefect to the event
        sqlx::query!(
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

    Ok(Redirect::to(&format!("/update_event/{event_id}"))) //redirect back to the update event page
}

///`POST` method that adds a participant
#[axum::debug_handler]
pub async fn post_add_participant_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(AddPerson {event_id, person_id: participant_id}): Form<AddPerson>,
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
    .is_none() //if we can't find anything assoiated with this participant and this event
    {
        //then we add the participant to the event
        sqlx::query!(
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

    Ok(Redirect::to(&format!("/update_event/{event_id}"))) //then back to the update event page
}
