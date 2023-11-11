#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::items_after_statements,
    clippy::cast_possible_truncation
)]

mod auth;
mod cfg;
mod error;
mod liquid_utils;
mod routes;
mod state;

use crate::{
    auth::{
        add_password::{get_add_password, get_blank_add_password, post_add_password},
        login::{get_login, get_login_failure, post_login, post_logout},
        PermissionsRole,
    },
    error::not_found_fallback,
    liquid_utils::partials::reload_partials,
    routes::{
        add_event::{get_add_event_form, post_add_event_form},
        add_people_to_event::{post_add_participant_to_event, post_add_prefect_to_event},
        add_person::{get_add_person, post_add_person},
        calendar::get_calendar_feed,
        edit_person::{get_edit_person, post_edit_person, post_reset_password},
        edit_self::{get_edit_user, post_edit_user},
        eoy_migration::{get_eoy_migration, post_eoy_migration},
        images::{get_all_images, post_add_photo, serve_image},
        import_export::{
            export_events_to_csv, export_people_to_csv, get_import_export_csv,
            post_import_events_from_csv, post_import_people_from_csv,
        },
        index::get_index,
        public::{
            get_256, get_512, get_events_csv_example, get_favicon, get_log, get_manifest,
            get_offline, get_people_csv_example, get_sw,
        },
        rewards::{get_rewards, post_add_reward},
        show_all::{get_show_all, post_remove_event, post_remove_person},
        spreadsheets::get_spreadsheet,
        update_events::{
            delete_image, get_remove_participant_from_event, get_remove_prefect_from_event,
            get_update_event, post_unverify_person, post_update_event, post_verify_everyone,
            post_verify_person,
        },
    },
    state::KnotState,
};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use liquid_utils::partials::PARTIALS;
use sqlx::postgres::PgPoolOptions;
use std::{env::var, net::SocketAddr, time::Duration};
use axum_login::{AuthManagerLayer, permission_required};
use sqlx::PgPool;
use tokio::signal;
use tower::limit::ConcurrencyLimitLayer;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};
use tower_sessions::{Expiry, PostgresStore, SessionManagerLayer};
use tower_sessions::sqlx::Postgres;
use crate::auth::backend::KnotAuthBackend;
use crate::auth::PermissionsTarget;
use crate::routes::public;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate async_trait;

// https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal(state: KnotState) {
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

    state.send_stop_notices();
    warn!("signal received, starting graceful shutdown");
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() {
    dotenvy::dotenv().expect("unable to get env variables");

    tracing::subscriber::set_global_default(
        Registry::default()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer().json()),
    )
    .unwrap();

    PARTIALS.write().await.reload().await;

    let db_url = var("DATABASE_URL").expect("DB URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(98) //100 - 2 to keep space for an emergency dbeaver instance etc (100 from here: https://www.postgresql.org/docs/current/runtime-config-connection.html)
        .connect(&db_url)
        .await
        .expect("cannot connect to DB");

    let session_layer = SessionManagerLayer::new(PostgresStore::new(pool.clone()))
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let state = KnotState::new(pool).await;

    let router = Router::new()
        .route("/show_all", get(get_show_all))
        .route("/ical", get(get_calendar_feed))
        .route("/spreadsheet", get(get_spreadsheet))
        .route("/", get(get_index))
        .merge(public::router())
        .merge(add_password::router())
        .fallback(not_found_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)) //50MB i think
        .layer(AuthManagerLayer::new(KnotAuthBackend::new(state.clone()), session_layer))
        .layer(ConcurrencyLimitLayer::new(512)) //limit to 512 inflight reqs
        .with_state(state.clone());


    let port: SocketAddr = var("KNOT_SERVER_IP")
        .expect("need KNOT_SERVER_IP env var")
        .parse()
        .expect("need KNOT_SERVER_IP to be valid");

    info!(?port, "Serving: ");

    axum::Server::bind(&port)
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .unwrap();
}
