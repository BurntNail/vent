#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::items_after_statements)]

mod error;
mod liquid_utils;
mod routes;

use axum::{
    routing::{get, post},
    Router, response::IntoResponse,
};
use error::KnotError;
use liquid_utils::{partials::{init_partials, PARTIALS}, compile};
use routes::{
    add_event::{self, get_add_event_form, post_add_event_form},
    add_people_to_event::{get_add_participant_to_event, get_add_prefect_to_event},
    add_person::{self, get_add_person, post_add_person},
    calendar::{self, get_calendar_feed},
    index::{self, get_index},
    remove_stuff::{self, get_remove_stuff, post_remove_event, post_remove_person},
};
use sqlx::postgres::PgPoolOptions;
use std::{env::var, net::SocketAddr, sync::Arc};

use crate::routes::update_event_and_person::{
    get_remove_participant_from_event, get_remove_prefect_from_event, get_update_event,
};

#[macro_use]
extern crate tracing;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("unable to get env variables");
    tracing_subscriber::fmt::init();

    PARTIALS
        .set(init_partials().await)
        .expect("unable to set partials");

    let db_url = std::env::var("DATABASE_URL").expect("DB URL must be set");
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .expect("cannot connect to DB"),
    );

    let app = Router::new()
        .route("/", get(public_index))
        .route(index::LOCATION, get(get_index))
        .route(
            add_event::LOCATION,
            get(get_add_event_form).post(post_add_event_form),
        )
        .route(
            "/kingsleyisbest123/add_participant/:event_id/:participant_id",
            get(get_add_participant_to_event),
        )
        .route(
            "/kingsleyisbest123/add_prefect/:event_id/:prefect_id",
            get(get_add_prefect_to_event),
        )
        .route(
            add_person::LOCATION,
            get(get_add_person).post(post_add_person),
        )
        .route(remove_stuff::LOCATION, get(get_remove_stuff))
        .route("/kingsleyisbest123/remove_person", post(post_remove_person))
        .route("/kingsleyisbest123/remove_event", post(post_remove_event))
        .route(calendar::LOCATION, get(get_calendar_feed))
        .route("/kingsleyisbest123/update_event/:id", get(get_update_event))
        .route(
            "/kingsleyisbest123/remove_prefect_from_event/:relation_id",
            get(get_remove_prefect_from_event),
        )
        .route(
            "/kingsleyisbest123/remove_participant_from_event/:relation_id",
            get(get_remove_participant_from_event),
        )
        .with_state(pool);

    let port: SocketAddr = var("KNOT_SERVER_IP")
        .expect("need KNOT_SERVER_IP env var")
        .parse()
        .expect("need KNOT_SERVER_IP to be valid");

    info!(?port, "Serving: ");

    axum::Server::bind(&port)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub async fn public_index () -> Result<impl IntoResponse, KnotError> {
    compile("www/public_index.liquid", liquid::object!({})).await
}