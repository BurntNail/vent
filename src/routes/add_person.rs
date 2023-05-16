use crate::{error::KnotError, liquid_utils::compile};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub const LOCATION: &str = "/kingsleyisbest123/add_person";

///`GET` function to display the add person form
pub async fn get_add_person() -> Result<impl IntoResponse, KnotError> {
    compile("www/add_person.liquid", liquid::object!({})).await
}

#[derive(Deserialize)]
pub struct NoIDPerson {
    pub person_name: String,
    pub is_prefect: bool,
}

pub async fn post_add_person(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(NoIDPerson {
        person_name,
        is_prefect,
    }): Form<NoIDPerson>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    info!(?person_name, ?is_prefect);

    sqlx::query!(
        r#"
INSERT INTO public.people
(person_name, is_prefect)
VALUES($1, $2);    
    "#,
        person_name,
        is_prefect
    )
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to(LOCATION))
}
