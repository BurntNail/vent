//! Page to deal with adding events.
//!
//! NB: does not handle prefects/participants/images.
//!
//! It serves a simple form, and handles post requests to add that event to the DB.

use crate::{
    auth::{get_auth_object, Auth},
    error::KnotError,
    liquid_utils::compile,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use super::{DbEvent, FormEvent};

///`GET` method for the `add_event` form - just compiles and returns the liquid `www/add_event.liquid`
pub async fn get_add_event_form(auth: Auth) -> Result<impl IntoResponse, KnotError> {
    compile(
        "www/add_event.liquid",
        liquid::object!({"auth": get_auth_object(auth)}),
    )
    .await
}

///`POST` method to add an event from a form to the database. Redirects back to the [`get_add_event_form`]
pub async fn post_add_event_form(
    State(pool): State<Arc<Pool<Postgres>>>,
    Form(event): Form<FormEvent>,
) -> Result<impl IntoResponse, KnotError> {
    let mut conn = pool.acquire().await?; //get a database connection

    let DbEvent {
        id: _,
        event_name,
        date,
        location,
        teacher,
        other_info: info,
    } = DbEvent::try_from(event)?; //get the info from the event form, parsed into a form that works for the DB

    let id = sqlx::query!(
        r#"
INSERT INTO public.events
(event_name, "date", "location", teacher, other_info)
VALUES($1, $2, $3, $4, $5)
RETURNING id
        "#,
        event_name,
        date,
        location,
        teacher,
        info
    )
    .fetch_one(&mut conn) //add the event to the db
    .await?
    .id;

    Ok(Redirect::to(&format!("/update_event/{id}"))) //redirect to the relevant update event page for that event
}
