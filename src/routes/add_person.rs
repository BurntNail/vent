//! Module that deals with adding a person - publishes a `GET` method with a form, and a `POST` method that deals with the form.

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

///`GET` function to display the add person form
pub async fn get_add_person(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_person.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
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
    Form(NoIDPerson {
        first_name,
        surname,
        form,
        is_prefect,
    }): Form<NoIDPerson>,
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

    Ok(Redirect::to("/add_person"))
}
