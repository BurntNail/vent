use crate::PROJECT_NAME;
use async_zip::error::ZipError;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use once_cell::sync::Lazy;
use std::{env::var, path::PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum KnotError {
    //external errors
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
    #[error("Multipart Error")]
    Multipart(#[from] axum::extract::multipart::MultipartError),
    #[error("Invalid Image")]
    ImageFormat(#[from] image::error::ImageError),
    #[error("Missing Image Extension: {0:?}")]
    NoImageExtension(image::ImageFormat),
    #[error("Error creating Zip File")]
    Zip(#[from] ZipError),
    #[error("Error with XLSX")]
    Csv(#[from] rust_xlsxwriter::XlsxError),
    #[error("Error with Encrypting")]
    Bcrypt(#[from] bcrypt::BcryptError),
    #[error("Random Eyre Error")]
    Eyre(#[from] eyre::Error), //thanks axum_login ;)

    // internal errors
    #[error("Missing File: {0:?}")]
    MissingFile(String),
    #[error("Unknown MIME Type for File: {0:?}")]
    UnknownMIME(PathBuf),
    #[error("Encountered Invalid UTF-8")]
    InvalidUTF8,
}

impl IntoResponse for KnotError {
    fn into_response(self) -> axum::response::Response {
        static TS_URL: Lazy<String> =
            Lazy::new(|| var("TECH_SUPPORT").unwrap_or_else(|_e| "https://google.com".into()));

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!(
                include_str!("../www/server_error.html"),
                instance_name = PROJECT_NAME.as_str(),
                tech_support = TS_URL.as_str(),
                error = self
            )),
        )
            .into_response()
    }
}
