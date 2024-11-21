use crate::{
    auth::{backend::VentAuthBackend, cloudflare_turnstile::CommonHeaders, PermissionsTarget},
    error::{
        FileIdentifier, HeadersSnafu, HttpAction, HttpSnafu, IOAction, IOSnafu, SerdeJsonAction,
        SerdeJsonSnafu, UnknownMIMESnafu, VentError,
    },
    state::VentState,
};
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::header,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use axum_login::permission_required;
use http::HeaderValue;
use new_mime_guess::from_path;
use serde_json::{from_str, Value};
use snafu::{OptionExt, ResultExt};
use std::{fmt::Debug, path::PathBuf};
use tokio::{
    fs::{
        read_to_string
    },
    io::{AsyncRead, AsyncReadExt},
};
use tower_http::services::ServeDir;
use crate::error::{ConvertingWhatToString, ToStrSnafu};

pub async fn serve_static_file_from_s3(
    path: impl Into<PathBuf> + Debug,
    state: &VentState,
) -> Result<impl IntoResponse, VentError> {
    let path = path.into();
    let path_str = path.to_str().context(ToStrSnafu {
        what: ConvertingWhatToString::PathBuffer(path.clone()),
    })?;

    let contents = state.bucket.read_file(path_str).await?;

    let mime = from_path(path.clone())
        .first()
        .context(UnknownMIMESnafu { path: path.clone() })?;

    serve_bytes_with_mime(contents, mime.essence_str()).await
}

#[allow(dead_code)]
pub async fn serve_read(
    mime: &str,
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

    serve_bytes_with_mime(contents, mime).await
}

pub async fn serve_bytes_with_mime(contents: Vec<u8>, mime: &str) -> Result<Response, VentError> {
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            HeaderValue::try_from(mime).context(HeadersSnafu {
                which_header: CommonHeaders::ContentType,
            })?,
        )
        .header(header::CONTENT_LENGTH, contents.len())
        .body(Body::from(Bytes::from(contents)))
        .context(HttpSnafu {
            action: HttpAction::BuildingResponse,
        })
}

#[axum::debug_handler]
pub async fn get_log() -> Result<Json<Vec<Value>>, VentError> {
    let contents = read_to_string("./log.json") //TODO: work out way to sync logs between docker instances
        .await
        .context(IOSnafu {
            action: IOAction::ReadingAndOpening(FileIdentifier::Const("./log.json")),
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
        pub async fn $func_name(State(state): State<VentState>) -> Result<impl IntoResponse, VentError> {
            serve_static_file_from_s3($path, &state).await
        }
    };
}

get_x!(get_favicon, "public/favicon.ico");
get_x!(get_manifest, "public/manifest.json");
get_x!(get_sw, "public/sw.js");
get_x!(get_offline, "public/offline.html");
get_x!(get_robots_txt, "public/robots.txt");

pub fn router() -> Router<VentState> {
    Router::new()
        .route("/logs", get(get_log))
        .route_layer(permission_required!(
            VentAuthBackend,
            login_url = "/login",
            PermissionsTarget::DevAccess
        ))
        .route("/favicon.ico", get(get_favicon))
        .route("/manifest.json", get(get_manifest))
        .route("/sw.js", get(get_sw))
        .route("/offline.html", get(get_offline))
        .route("/robots.txt", get(get_robots_txt))
        .nest_service("/assets/", ServeDir::new("public/"))
}
