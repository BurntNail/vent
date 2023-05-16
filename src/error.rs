use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};

#[derive(thiserror::Error, Debug)]
pub enum KnotError {
    #[error("Database Error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Liquid Error")]
    Liquid(#[from] liquid::Error),
    #[error("IO Error")]
    IO(#[from] std::io::Error),
    #[error("Tokio Join Error")]
    Join(#[from] tokio::task::JoinError),
    #[error("Error Parsing Integer")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("Error Parsing Time")]
    ParseTime(#[from] chrono::ParseError),
    #[error("Error in Headers")]
    Headers(#[from] axum::http::header::InvalidHeaderValue),
}

impl IntoResponse for KnotError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Internal Server Error: {self:?}")),
        )
            .into_response()
    }
}
