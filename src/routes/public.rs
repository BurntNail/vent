use crate::error::KnotError;
use axum::{
    body::StreamBody,
    http::{header, HeaderMap},
    response::IntoResponse,
};
use new_mime_guess::from_path;
use std::path::PathBuf;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub async fn serve_static_file(path: impl Into<PathBuf>) -> Result<impl IntoResponse, KnotError> {
    let path = path.into();

    let file = File::open(path.clone()).await?;
    let file_size = file.metadata().await?.len();
    let body = StreamBody::new(ReaderStream::new(file));

    let mime = from_path(path.clone()).first().ok_or(KnotError::UnknownMIME(path))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.essence_str().try_into()?);
    headers.insert(header::CONTENT_LENGTH, file_size.into());

    Ok((headers, body))
}

macro_rules! get_x {
    ($func_name:ident, $path:expr) => {
        pub async fn $func_name() -> Result<impl IntoResponse, KnotError> {
            serve_static_file($path).await
        }
    };
}

get_x!(get_favicon, "public/favicon.ico");
get_x!(get_manifest, "public/manifest.json");
get_x!(get_sw, "public/sw.js");
get_x!(get_offline, "public/offline.html");
get_x!(get_512, "public/512x512.png");
get_x!(get_256, "public/256x256.png");
