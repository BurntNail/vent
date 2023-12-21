use crate::{
    error::{
        FileIdentifier, HeadersSnafu, HttpAction, HttpSnafu, IOAction, IOSnafu, VentError,
        SerdeJsonAction, SerdeJsonSnafu, UnknownMIMESnafu,
    },
    state::VentState,
};
use axum::{
    body::{Body, Bytes},
    http::header,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use http::{HeaderValue, Response};
use new_mime_guess::{from_path, Mime};
use serde_json::{from_str, Value};
use snafu::{OptionExt, ResultExt};
use std::{fmt::Debug, path::PathBuf};
use tokio::{
    fs::{read_to_string, File},
    io::{AsyncRead, AsyncReadExt},
};
use crate::error::CommonHeaders;

pub async fn serve_static_file(
    path: impl Into<PathBuf> + Debug,
) -> Result<impl IntoResponse, VentError> {
    let path = path.into();

    let file = File::open(path.clone()).await.context(IOSnafu {
        action: IOAction::OpeningFile(path.clone().into()),
    })?;

    let mime = from_path(path.clone())
        .first()
        .context(UnknownMIMESnafu { path: path.clone() })?;

    serve_read(mime, file, IOAction::ReadingFile(path.clone().into())).await
}

pub async fn serve_read(
    mime: Mime,
    mut reader: impl AsyncRead + Unpin,
    io_action_for_errors: IOAction,
) -> Result<impl IntoResponse, VentError> {
    let mut contents = vec![];
    let mut tmp = [0; 1024];
    loop {
        let n = reader.read(&mut tmp).await.context(IOSnafu {
            action: io_action_for_errors.clone(),
        })?;
        match n {
            0 => break,
            n => {
                contents.extend_from_slice(&tmp[0..n]);
            }
        }
    }

    drop(reader);

    let file_size = contents.len();

    let rsp = Response::builder()
        .header(
            header::CONTENT_TYPE,
            HeaderValue::try_from(mime.essence_str()).context(HeadersSnafu {
                which_header: CommonHeaders::ContentType,
            })?,
        )
        .header(header::CONTENT_LENGTH, file_size)
        .body(Body::from(Bytes::from(contents)))
        .context(HttpSnafu {
            action: HttpAction::BuildingResponse,
        })?;

    Ok(rsp)
}

#[axum::debug_handler]
pub async fn get_log() -> Result<Json<Vec<Value>>, VentError> {
    let contents = read_to_string("./precipice-log.json")
        .await
        .context(IOSnafu {
            action: IOAction::ReadingAndOpening(FileIdentifier::Const("./precipice-log.json")),
        })?;
    let items = contents
        .lines()
        .map(from_str)
        .rev()
        .collect::<Result<_, _>>()
        .context(SerdeJsonSnafu {
            action: SerdeJsonAction::ParsingLogFile,
        })?;
    Ok(Json(items))
}

macro_rules! get_x {
    ($func_name:ident, $path:expr) => {
        #[axum::debug_handler]
        pub async fn $func_name() -> Result<impl IntoResponse, VentError> {
            serve_static_file($path).await
        }
    };
}


pub fn router() -> Router<VentState> {
    Router::new()
        .route("/logs", get(get_log))
}
