mod index;

use std::net::SocketAddr;
use sqlx::postgres::PgPoolOptions;
use axum::{routing::get, Router};
use index::{root, root_form};

#[macro_use] extern crate tracing;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("unable to get env variables");
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", get(root).post(root_form));

    let db_url = std::env::var("DATABASE_URL").expect("DB URL must be set");
    let pool = PgPoolOptions::new().max_connections(5).connect(&db_url).await.expect("cannot connect to DB");

    axum::Server::bind(&SocketAddr::from(([127, 0, 0, 1], 8080)))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
