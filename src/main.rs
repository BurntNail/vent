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

pub use http;

use crate::{
    auth::{add_password, backend::KnotAuthBackend, login, pg_session::PostgresStore},
    error::not_found_fallback,
    liquid_utils::partials,
    routes::{
        add_event, add_people_to_event, add_person, calendar::get_calendar_feed, edit_person,
        edit_self, eoy_migration, images, import_export, index::get_index, public, rewards,
        show_all, spreadsheets::get_spreadsheet, update_events,
    },
    state::KnotState,
};
use axum::{
    error_handling::HandleErrorLayer,
    extract::{DefaultBodyLimit, Request},
    response::IntoResponse,
    routing::get,
    BoxError, Router,
};
use axum_login::{
    tower_sessions::{Expiry, SessionManagerLayer},
    AuthManagerLayerBuilder,
};
use http::StatusCode;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::TokioIo;
use liquid_utils::partials::PARTIALS;
use sqlx::postgres::PgPoolOptions;
use std::{env::var, net::SocketAddr};
use time::Duration;
use tokio::{net::TcpListener, signal, sync::watch};
use tower::{limit::ConcurrencyLimitLayer, Service, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};

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

#[axum::debug_handler]
async fn healthcheck() -> impl IntoResponse {
    StatusCode::OK
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

    let auth_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            error!(?e, "Error in handling errors layer!");
            StatusCode::BAD_REQUEST
        }))
        .layer(
            AuthManagerLayerBuilder::new(KnotAuthBackend::new(state.clone()), session_layer)
                .build(),
        );

    let router = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/ical", get(get_calendar_feed))
        .route("/spreadsheet", get(get_spreadsheet))
        .route("/", get(get_index))
        .merge(public::router())
        .merge(add_password::router())
        .merge(login::router())
        .merge(partials::router())
        .merge(import_export::router())
        .merge(edit_self::router())
        .merge(rewards::router())
        .merge(add_event::router())
        .merge(add_people_to_event::router())
        .merge(add_person::router())
        .merge(edit_person::router())
        .merge(eoy_migration::router())
        .merge(images::router())
        .merge(show_all::router())
        .merge(update_events::router())
        .fallback(not_found_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)) //50MB i think
        .layer(auth_service)
        .layer(ConcurrencyLimitLayer::new(512)) //limit to 512 inflight reqs
        .with_state(state.clone());

    let port: SocketAddr = var("KNOT_SERVER_IP")
        .expect("need KNOT_SERVER_IP env var")
        .parse()
        .expect("need KNOT_SERVER_IP to be valid");

    info!(?port, "Serving: ");

    serve(router, TcpListener::bind(port).await.unwrap(), state).await;
}

//https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn serve(app: Router, listener: TcpListener, state: KnotState) {
    let (close_tx, close_rx) = watch::channel(());

    loop {
        let (socket, _remote_addr) = tokio::select! {
            result = listener.accept() => {
                result.unwrap()
            },
            _ = shutdown_signal(state.clone()) => {
                break;
            }
        };

        let tower_service = app.clone();
        let close_rx = close_rx.clone();
        let state = state.clone();

        tokio::spawn(async move {
            let socket = TokioIo::new(socket);
            let hyper_service =
                service_fn(move |req: Request<Incoming>| tower_service.clone().call(req));

            let conn = hyper::server::conn::http1::Builder::new()
                .serve_connection(socket, hyper_service)
                .with_upgrades();

            let mut conn = std::pin::pin!(conn);

            loop {
                tokio::select! {
                    result = conn.as_mut() => {
                        if let Err(err) = result {
                            error!(?err, "Failed to serve connection :(")
                        }
                        break;
                    },
                    _ = shutdown_signal(state.clone()) => {
                        conn.as_mut().graceful_shutdown();
                    }
                }
            }

            drop(close_rx)
        });
    }

    drop(close_rx);
    drop(listener);

    close_tx.closed().await
}
