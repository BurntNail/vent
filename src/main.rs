#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::items_after_statements)]

mod error;
mod liquid_utils;
mod routes;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use liquid_utils::partials::{init_partials, PARTIALS};
use routes::{
    add_event::{get_add_event_form, post_add_event_form},
    add_people_to_event::{post_add_participant_to_event, post_add_prefect_to_event},
    add_person::{get_add_person, post_add_person},
    calendar::get_calendar_feed,
    index::get_index,
    public::get_favicon,
    show_all::{get_remove_stuff, post_remove_event, post_remove_person},
    update_event_and_person::{
        get_remove_participant_from_event, get_remove_prefect_from_event, get_update_event,
        post_update_event,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::{env::var, net::SocketAddr, sync::Arc};
use tokio::signal;
use tower_http::trace::TraceLayer;

use crate::routes::{
    edit_person::{get_edit_person, post_edit_person},
    images::{get_all_images, post_add_photo, serve_image},
    public::{get_256, get_512, get_manifest, get_offline, get_sw},
    spreadsheets::get_spreadsheet,
    update_event_and_person::delete_image,
};

#[macro_use]
extern crate tracing;

// https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    warn!("signal received, starting graceful shutdown");
}

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
            .max_connections(100)
            .connect(&db_url)
            .await
            .expect("cannot connect to DB"),
    );

    let app = Router::new()
        .route("/", get(get_index))
        .route("/favicon.ico", get(get_favicon).head(get_favicon))
        .route("/manifest.json", get(get_manifest).head(get_manifest))
        .route("/sw.js", get(get_sw).head(get_sw))
        .route("/offline.html", get(get_offline).head(get_offline))
        .route("/512x512.png", get(get_512).head(get_512))
        .route("/256x256.png", get(get_256).head(get_256))
        .route(
            "/add_event",
            get(get_add_event_form).post(post_add_event_form),
        )
        .route("/add_participant", post(post_add_participant_to_event))
        .route("/add_prefect", post(post_add_prefect_to_event))
        .route("/add_person", get(get_add_person).post(post_add_person))
        .route("/show_all", get(get_remove_stuff))
        .route("/remove_person", post(post_remove_person))
        .route("/remove_event", post(post_remove_event))
        .route("/remove_img/:id", get(delete_image))
        .route("/ical", get(get_calendar_feed))
        .route("/spreadsheet", get(get_spreadsheet))
        .route(
            "/update_event/:id",
            get(get_update_event).post(post_update_event),
        )
        .route(
            "/edit_person/:id",
            get(get_edit_person).post(post_edit_person),
        )
        .route(
            "/remove_prefect_from_event",
            post(get_remove_prefect_from_event),
        )
        .route(
            "/remove_participant_from_event",
            post(get_remove_participant_from_event),
        )
        .route("/add_image/:event_id", post(post_add_photo))
        .route("/get_all_imgs/:event_id", get(get_all_images))
        .route("/uploads/:img", get(serve_image))
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)) //50MB i think
        .with_state(pool);

    let port: SocketAddr = var("KNOT_SERVER_IP")
        .expect("need KNOT_SERVER_IP env var")
        .parse()
        .expect("need KNOT_SERVER_IP to be valid");

    info!(?port, "Serving: ");

    axum::Server::bind(&port)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}
