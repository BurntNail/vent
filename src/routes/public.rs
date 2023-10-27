use crate::{
    auth::cloudflare_turnstile::CommonHeaders,
    error::{
        FileIdentifier, HeadersSnafu, IOAction, IOSnafu, KnotError, SerdeJsonAction,
        SerdeJsonSnafu, UnknownMIMESnafu,
    },
};
use axum::{
    body::StreamBody,
    http::{header, HeaderMap},
    response::{IntoResponse, Json},
};
use http::HeaderValue;
use new_mime_guess::from_path;
use serde_json::{from_str, Value};
use snafu::{OptionExt, ResultExt};
use std::{fmt::Debug, path::PathBuf};
use tokio::fs::{read_to_string, File};
use tokio_util::io::ReaderStream;

#[instrument(level = "trace")]
pub async fn serve_static_file(
    path: impl Into<PathBuf> + Debug,
) -> Result<impl IntoResponse, KnotError> {
    trace!("Getting file contents/details");

    let path = path.into();

    trace!(?path);

    let file = File::open(path.clone()).await.context(IOSnafu {
        action: IOAction::OpeningFile(path.clone().into()),
    })?;
    let file_size = file
        .metadata()
        .await
        .context(IOSnafu {
            action: IOAction::ReadingMetadata,
        })?
        .len();
    let body = StreamBody::new(ReaderStream::new(file));

    let mime = from_path(path.clone())
        .first()
        .context(UnknownMIMESnafu { path })?;

    trace!("Building headers");

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::try_from(mime.essence_str()).context(HeadersSnafu {
            which_header: CommonHeaders::ContentType,
        })?,
    );
    headers.insert(header::CONTENT_LENGTH, file_size.into());

    trace!("Serving");

    Ok((headers, body))
}

#[axum::debug_handler]
pub async fn get_log() -> Result<Json<Vec<Value>>, KnotError> {
    let contents = read_to_string("./precipice-log.json")
        .await
        .context(IOSnafu {
            action: IOAction::ReadingAndOpening(FileIdentifier::Const("./precipice-log.json")),
        })?;
    let items = contents
        .lines()
        .map(from_str)
        .collect::<Result<_, _>>()
        .context(SerdeJsonSnafu {
            action: SerdeJsonAction::ParsingLogFile,
        })?;
    Ok(Json(items))
}

macro_rules! get_x {
    ($func_name:ident, $path:expr) => {
        #[axum::debug_handler]
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
get_x!(get_people_csv_example, "public/people_example.csv");
get_x!(get_events_csv_example, "public/events_example.csv");
