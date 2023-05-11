use std::sync::Arc;

use crate::{error::KnotError, liquid_utils::compile, routes::Person};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect}
};
use axum_extra::extract::Form;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

pub const LOCATION: &str = "/add_people_to_event";

pub async fn get_add_people_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let prefects: Vec<Person> = sqlx::query_as!(
        Person,
        r#"
SELECT person_name, id, is_prefect
FROM people
WHERE people.is_prefect = TRUE
        "#
    )
    .fetch_all(&mut conn)
    .await?;
    let participants: Vec<Person> = sqlx::query_as!(
        Person,
        r#"
SELECT person_name, id, is_prefect
FROM people
    "#
    )
    .fetch_all(&mut conn)
    .await?;

    #[derive(Serialize, Deserialize)]
    struct NeededDBEvent {
        pub event_name: String,
        pub id: i32,
    }

    let events: Vec<NeededDBEvent> = sqlx::query_as!(
        NeededDBEvent,
        r#"
SELECT id, event_name
FROM events
"#
    )
    .fetch_all(&mut conn)
    .await?;

    let globals = liquid::object!({
        "events": events,
        "prefects": prefects,
        "participants": participants
    });

    compile("www/add_people_to_event.liquid", globals).await
}

#[derive(Deserialize)]
pub struct AddPrefects {
    pub event_id: i32,
    pub prefects: Vec<String>,
}

pub async fn post_add_prefects_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(AddPrefects { event_id, prefects }): Form<AddPrefects>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;
    info!(?event_id, ?prefects);

    let prefects: Vec<i32> = prefects
        .into_iter()
        .map(|x| x.parse())
        .collect::<Result<_, _>>()?;

    for prefect_id in prefects {
        sqlx::query!(
            r#"
INSERT INTO public.prefect_events
(prefect_id, event_id)
VALUES($2, $1);            
            "#,
            event_id,
            prefect_id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to(LOCATION))
}

#[derive(Deserialize)]
pub struct AddParticipants {
    pub event_id: i32,
    pub people: Vec<String>,
}

pub async fn post_add_participants_to_event(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(AddParticipants { event_id, people }): Form<AddParticipants>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;
    info!(?event_id, ?people);

    let people: Vec<i32> = people
        .into_iter()
        .map(|x| x.parse())
        .collect::<Result<_, _>>()?;

    for id in people {
        sqlx::query!(
            r#"
INSERT INTO public.participant_events
(participant_id, event_id)
VALUES($2, $1);        
            "#,
            event_id,
            id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to(LOCATION))
}
