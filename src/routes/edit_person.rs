use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect},
    Form,
};
use sqlx::{Pool, Postgres};

use crate::{error::KnotError, liquid_utils::compile, routes::DbPerson};

use super::add_person::NoIDPerson;

pub async fn get_edit_person(
    Path(id): Path<i32>,
    State(pool): State<Arc<Pool<Postgres>>>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?;

    let person = sqlx::query_as!(DbPerson, r#"SELECT * FROM people WHERE id = $1"#, id)
        .fetch_one(&mut conn)
        .await?;

    compile(
        "www/edit_person.liquid",
        liquid::object!({ "person": person }),
    )
    .await
}

pub async fn post_edit_person(
    Path(id): Path<i32>,
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
    .execute(&mut conn)
    .await?;

    Ok(Redirect::to(&format!("/edit_person/{id}")))
}
