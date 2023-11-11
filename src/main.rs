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
        get_secret,
        login::{get_login, get_login_failure, post_login, post_logout},
        pg_session::PostgresSessionStore,
        PermissionsRole, RequireAuth, Store,
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
    state::VentState,
};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use axum_login::{axum_sessions::SessionLayer, AuthLayer};
use liquid_utils::partials::PARTIALS;
use sqlx::postgres::PgPoolOptions;
use std::{env::var, net::SocketAddr, time::Duration};
use tokio::signal;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate async_trait;

// https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal(state: VentState) {
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

    let db_url = std::env::var("DATABASE_URL").expect("DB URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(98) //100 - 2 to keep space for an emergency dbeaver instance etc (100 from here: https://www.postgresql.org/docs/current/runtime-config-connection.html)
        .connect(&db_url)
        .await
        .expect("cannot connect to DB");

    let secret = get_secret(&pool).await.expect("unable to get secret");

    let session_layer = SessionLayer::new(PostgresSessionStore::new(pool.clone()), &secret)
        .with_session_ttl(Some(Duration::from_secs(60 * 60 * 24 * 60))); // 60 days - around 2 months, and the same as MS iirc
    let auth_layer = AuthLayer::new(
        Store::new(pool.clone()).with_query(
            r#"
SELECT *
FROM people WHERE id = $1
        "#,
        ),
        &secret,
    );

    let state = VentState::new(pool).await;

    let app = Router::new()
        .route("/reload_partials", get(reload_partials))
        .route("/logs", get(get_log))
        .route_layer(RequireAuth::login_with_role(PermissionsRole::Dev..)) //dev ^
        .route("/add_person", get(get_add_person).post(post_add_person))
        .route("/remove_person", post(post_remove_person))
        .route(
            "/eoy_migration",
            get(get_eoy_migration).post(post_eoy_migration),
        )
        .route("/reset_password", post(post_reset_password))
        .route("/import_people_from_csv", post(post_import_people_from_csv))
        .route("/import_events_from_csv", post(post_import_events_from_csv))
        .route("/add_reward", post(post_add_reward))
        .route_layer(RequireAuth::login_with_role(PermissionsRole::Admin..)) //admin ^
        .route(
            "/add_event",
            get(get_add_event_form).post(post_add_event_form),
        )
        .route("/add_prefect", post(post_add_prefect_to_event))
        .route("/remove_event", post(post_remove_event))
        .route(
            "/remove_prefect_from_event",
            post(get_remove_prefect_from_event),
        )
        .route("/remove_img/:id", get(delete_image))
        .route("/verify_participant", post(post_verify_person))
        .route("/unverify_participant", post(post_unverify_person))
        .route("/verify_all", post(post_verify_everyone))
        .route("/spreadsheet", get(get_spreadsheet))
        .route_layer(RequireAuth::login_with_role(PermissionsRole::Prefect..)) //prefect ^
        .route("/add_image/:event_id", post(post_add_photo))
        .route("/add_participant", post(post_add_participant_to_event))
        .route(
            "/remove_participant_from_event",
            post(get_remove_participant_from_event),
        )
        .route("/get_all_imgs/:event_id", get(get_all_images))
        .route("/uploads/:img", get(serve_image))
        .route("/edit_user", get(get_edit_user).post(post_edit_user))
        .route("/logout", get(post_logout))
        .route("/csv", get(get_import_export_csv))
        .route("/csv_people", get(export_people_to_csv))
        .route("/csv_events", get(export_events_to_csv))
        .route(
            "/edit_person/:id",
            get(get_edit_person).post(post_edit_person),
        )
        .route("/add_reward", get(get_rewards))
        .route_layer(RequireAuth::login()) //^ REQUIRE LOGIN ^
        .route("/", get(get_index))
        .route("/add_password", get(get_blank_add_password))
        .route(
            "/add_password/:user_id",
            get(get_add_password).post(post_add_password),
        )
        .route(
            "/update_event/:id",
            get(get_update_event).post(post_update_event),
        )
        .route("/favicon.ico", get(get_favicon))
        .route("/manifest.json", get(get_manifest))
        .route("/sw.js", get(get_sw))
        .route("/offline.html", get(get_offline))
        .route("/512x512.png", get(get_512))
        .route("/256x256.png", get(get_256))
        .route("/people_example.csv", get(get_people_csv_example))
        .route("/events_example.csv", get(get_events_csv_example))
        .route("/show_all", get(get_show_all))
        .route("/ical", get(get_calendar_feed))
        .route(
            "/login_failure/:was_password_related",
            get(get_login_failure),
        )
        .route("/login", get(get_login).post(post_login))
        .fallback(not_found_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)) //50MB i think
        .layer(auth_layer)
        .layer(session_layer)
        .layer(ConcurrencyLimitLayer::new(512)) //limit to 512 inflight reqs
        .with_state(state.clone());

    let port: SocketAddr = var("VENT_SERVER_IP")
        .expect("need VENT_SERVER_IP env var")
        .parse()
        .expect("need VENT_SERVER_IP to be valid");

    info!(?port, "Serving: ");

    axum::Server::bind(&port)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .unwrap();
}
