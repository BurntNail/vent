
mod index;

use std::net::SocketAddr;

use axum::{Router, routing::get};
use index::root;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", get(root));

    axum::Server::bind(&SocketAddr::from(([127, 0, 0, 1], 8080)))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
