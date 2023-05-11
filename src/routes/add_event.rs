use std::{fmt::Write, sync::Arc};

use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use axum_extra::extract::Form;

use crate::{error::KnotError, liquid_utils::compile};

pub const LOCATION: &str = "/add_event";
//TODO: just use this lol: https://docs.rs/axum-extra/latest/axum_extra/routing/struct.Resource.html

pub async fn get_add_event_form(
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    #[derive(Serialize)]
    struct Prefect {
        pub person_name: String,
        pub id: i32,
    }

    let prefects: Vec<Prefect> = sqlx::query_as!(
        Prefect,
        r#"
        SELECT person_name, id
        FROM people
        WHERE people.is_prefect = TRUE
        "#
    )
    .fetch_all(&mut conn)
    .await?;

    let globals = liquid::object!({ "prefects": prefects });

    info!("here");

    compile("www/add_event.liquid", globals).await
}

#[derive(Debug, Deserialize)]
pub struct DbEvent {
    pub name: String,
    pub date: NaiveDateTime,
    pub location: String,
    pub teacher: String,
    pub info: String,
    pub prefects: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct FormEvent {
    pub name: String,
    pub date: String,
    pub location: String,
    pub teacher: String,
    pub info: String,
    pub prefects: Vec<String>,
}

impl TryFrom<FormEvent> for DbEvent {
    type Error = KnotError;

    fn try_from(
        FormEvent {
            name,
            date,
            location,
            teacher,
            info,
            prefects,
        }: FormEvent,
    ) -> Result<Self, Self::Error> {
        let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")?;

        Ok(Self {
            name,
            date,
            location,
            teacher,
            info,
            prefects: prefects
                .into_iter()
                .map(|s| s.parse())
                .collect::<Result<_, _>>()?,
        })
    }
}

pub async fn post_add_event_form(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(event): Form<FormEvent>,
) -> Result<impl IntoResponse, KnotError> {
    info!(?event);
    let mut conn = pool.acquire().await?;
    let DbEvent {
        name,
        date,
        location,
        teacher,
        info,
        prefects,
    } = DbEvent::try_from(event).unwrap(); //TODO: Fix error handling

    let event_id: i32 = sqlx::query!(
        r#"
INSERT INTO public.events
(event_name, "date", "location", teacher, other_info)
VALUES($1, $2, $3, $4, $5)
RETURNING id
        "#,
        name,
        date,
        location,
        teacher,
        info
    )
    .fetch_one(&mut conn)
    .await?
    .id;

    for prefect in prefects {
        sqlx::query!(
            r#"
INSERT INTO public.prefect_events
(prefect_id, event_id)
VALUES($1, $2);            
            "#,
            prefect,
            event_id
        )
        .execute(&mut conn)
        .await?;
    }

    Ok(Redirect::to(LOCATION))
}
