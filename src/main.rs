#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::items_after_statements,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

mod auth;
mod cfg;
mod error;
mod image_format;
mod liquid_utils;
mod routes;
mod state;

pub use http;

use crate::{
    auth::{add_password, backend::VentAuthBackend, login, pg_session::PostgresStore},
    error::not_found_fallback,
    liquid_utils::partials,
    routes::{
        add_event, add_people_to_event, add_person, calendar::get_calendar_feed, csv_import_export,
        edit_person, edit_self, eoy_migration, give_bonus_point, images, index::get_index, public,
        rewards, show_bonus_points, show_events, show_people, spreadsheets::get_spreadsheet,
        update_bonus_point, update_events,
    },
    state::VentState,
};
use axum::{
    extract::{DefaultBodyLimit, Request},
    response::IntoResponse,
    routing::get,
    Router,
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
use std::{
    env::var,
    fs::{File, OpenOptions},
    io::Stdout,
    net::SocketAddr,
    path::Path,
};
use time::Duration;
use tokio::{net::TcpListener, signal, sync::watch};
use tower::{limit::ConcurrencyLimitLayer, Service};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{
    fmt::MakeWriter, prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry,
};

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate async_trait;

#[derive(Debug)]
struct SyncFileAndStdoutWriter {
    stdout: Stdout,
    file: File,
}

impl SyncFileAndStdoutWriter {
    pub fn new(file_name: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        Ok(Self {
            stdout: std::io::stdout(),
            file: OpenOptions::new().append(true).open(file_name)?,
        })
    }
}

impl std::io::Write for SyncFileAndStdoutWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.file.write(buf)?;
        self.stdout.write_all(&buf[..n])?;

        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.stdout.flush()?;

        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.file.write_all(buf)?;
        self.stdout.write_all(buf)?;

        Ok(())
    }
}

struct WriterMaker<P: AsRef<Path>> {
    name: P,
}

impl<P: AsRef<Path>> WriterMaker<P> {
    fn new(name: P) -> Result<Self, std::io::Error> {
        File::create(&name)?; //ensure file exists, clear old file
        Ok(Self { name })
    }
}

impl<'w, P: AsRef<Path>> MakeWriter<'w> for WriterMaker<P> {
    type Writer = SyncFileAndStdoutWriter;

    fn make_writer(&'w self) -> Self::Writer {
        SyncFileAndStdoutWriter::new(&self.name).unwrap()
    }
}

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
        () = ctrl_c => {},
        () = terminate => {},
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
    if option_env!("DOCKER_BUILD").is_none() {
        dotenvy::dotenv().expect("unable to get env variables");
    }

    tracing::subscriber::set_global_default(
        Registry::default()
            .with(EnvFilter::from_default_env())
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(WriterMaker::new("log.json").expect("unable to create writer")),
            ),
    )
    .unwrap();

    PARTIALS.write().await.reload().await;

    let db_url = var("DATABASE_URL").expect("DB URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(98) //100 - 2 to keep space for an emergency dbeaver instance etc (100 from here: https://www.postgresql.org/docs/current/runtime-config-connection.html)
        .connect(&db_url)
        .await
        .expect("cannot connect to DB");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("cannot run migrations.");

    let session_layer = SessionManagerLayer::new(PostgresStore::new(pool.clone()))
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let state = VentState::new(pool).await;

    let auth_layer =
        AuthManagerLayerBuilder::new(VentAuthBackend::new(state.clone()), session_layer).build();

    let router = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/ical", get(get_calendar_feed))
        .route("/spreadsheet", get(get_spreadsheet))
        .route("/", get(get_index))
        .merge(public::router())
        .merge(add_password::router())
        .merge(login::router())
        .merge(partials::router())
        .merge(csv_import_export::router())
        .merge(edit_self::router())
        .merge(rewards::router())
        .merge(add_event::router())
        .merge(add_people_to_event::router())
        .merge(add_person::router())
        .merge(edit_person::router())
        .merge(eoy_migration::router())
        .merge(images::router())
        .merge(show_people::router())
        .merge(show_events::router())
        .merge(update_events::router())
        .merge(give_bonus_point::router())
        .merge(update_bonus_point::router())
        .merge(show_bonus_points::router())
        .merge(state::router())
        .fallback(not_found_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)) //50MB i think
        .layer(auth_layer)
        .layer(ConcurrencyLimitLayer::new(512)) //limit to 512 inflight reqs
        .with_state(state.clone());

    let port: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    info!(?port, "Serving: ");

    serve(router, TcpListener::bind(port).await.unwrap(), state).await;
}

//https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn serve(app: Router, listener: TcpListener, state: VentState) {
    let (close_tx, close_rx) = watch::channel(());

    loop {
        let (socket, _remote_addr) = tokio::select! {
            result = listener.accept() => {
                result.unwrap()
            },
            () = shutdown_signal(state.clone()) => {
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
                            error!(?err, "Failed to serve connection :(");
                        }
                        break;
                    },
                    () = shutdown_signal(state.clone()) => {
                        conn.as_mut().graceful_shutdown();
                    }
                }
            }

            drop(close_rx);
        });
    }

    drop(close_rx);
    drop(listener);

    close_tx.closed().await;
}
