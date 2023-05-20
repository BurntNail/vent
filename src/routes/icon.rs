use crate::error::KnotError;
use axum::{
    body::StreamBody,
    http::{header, HeaderMap},
    response::IntoResponse,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub const LOCATION: &str = "/favicon.ico";

pub async fn get_favicon() -> Result<impl IntoResponse, KnotError> {
    let file = File::open("favicon.ico").await?;

    let file_size = file.metadata().await?.len();

    let body = StreamBody::new(ReaderStream::new(file));

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/x-icon".parse()?);
    headers.insert(header::CONTENT_LENGTH, file_size.into());

    Ok((headers, body))
}

pub async fn get_manifest() -> Result<impl IntoResponse, KnotError> {
    let file = File::open("manifest.json").await?;

    let file_size = file.metadata().await?.len();

    let body = StreamBody::new(ReaderStream::new(file));

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
    headers.insert(header::CONTENT_LENGTH, file_size.into());

    Ok((headers, body))
}
