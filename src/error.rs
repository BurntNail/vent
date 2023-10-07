use async_zip::error::ZipError;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use std::{fmt::Debug, path::PathBuf};

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
    #[error("Error Parsing Bool")]
    ParseBool(#[from] std::str::ParseBoolError),
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
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
    #[error("Error with Encrypting")]
    Bcrypt(#[from] bcrypt::BcryptError),
    #[error("Error converting Header to string, possibly invalid UTF-8")]
    HeaderToStr(#[from] http::header::ToStrError),
    #[error("Error reqwest-ing")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error parsing email address")]
    LettreAddress(#[from] lettre::address::AddressError),
    #[error("Error with Emails")]
    LettreEmail(#[from] lettre::error::Error),
    #[error("Error with SMTP")]
    LettreSMTP(#[from] lettre::transport::smtp::Error),
    #[error("Error with CSV Files")]
    Csv(#[from] csv_async::Error),
    #[error("JSON error")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Random Eyre Error")]
    Eyre(#[from] eyre::Error), //thanks axum_login ;)

    // internal errors
    #[error("Missing File: {0:?}")]
    MissingFile(String),
    #[error("Unknown MIME Type for File: {0:?}")]
    UnknownMIME(PathBuf),
    #[error("Encountered Invalid UTF-8")]
    InvalidUTF8,
    #[error("CSV incorrect format")]
    MalformedCSV,
    #[error("Missing Cloudflare IP in headers")]
    MissingCFIP,
}

#[allow(clippy::needless_pass_by_value)]
pub fn get_error_page(
    error_code: StatusCode,
    content: KnotError,
) -> (StatusCode, Html<&'static str>) {
    error!(?content, "Dealing with Error page: {content:#?}");

    (error_code, Html(include_str!("../www/server_error.html")))
}

impl IntoResponse for KnotError {
    fn into_response(self) -> axum::response::Response {
        get_error_page(StatusCode::INTERNAL_SERVER_ERROR, self).into_response()
    }
}
