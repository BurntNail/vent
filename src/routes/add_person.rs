use crate::{error::KnotError, liquid_utils::compile};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub const LOCATION: &str = "/add_person";

///`GET` function to display the add person form
pub async fn get_add_person() -> Result<impl IntoResponse, KnotError> {
    compile("www/add_person.liquid", liquid::object!({})).await
}

#[derive(Deserialize)]
pub struct NoIDPerson {
    pub first_name: String,
    pub surname: String,
    pub form: Option<String>,
    pub is_prefect: bool,
}

pub async fn post_add_person(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(NoIDPerson { first_name, surname, form, is_prefect }): Form<NoIDPerson>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    sqlx::query!(
        r#"
INSERT INTO public.people
(is_prefect, first_name, surname, form)
VALUES($1, $2, $3, $4);    
    "#,
        is_prefect,
        first_name,
        surname,
        form,
    )
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to(LOCATION))
}
