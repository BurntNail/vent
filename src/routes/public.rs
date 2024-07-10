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
    http::header,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use axum_login::permission_required;
use http::{HeaderValue, Response};
use new_mime_guess::{from_path};
use serde_json::{from_str, Value};
use snafu::{OptionExt, ResultExt};
use std::{fmt::Debug, path::PathBuf};
use tokio::{
    fs::{read_to_string, File},
    io::{AsyncRead, AsyncReadExt},
};

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

    serve_read(mime.essence_str(), file, IOAction::ReadingFile(path.clone().into())).await
}

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

    let file_size = contents.len();

    let rsp = Response::builder()
        .header(
            header::CONTENT_TYPE,
            HeaderValue::try_from(mime).context(HeadersSnafu {
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

get_x!(get_favicon, "public/favicon.ico");
get_x!(get_manifest, "public/manifest.json");
get_x!(get_sw, "public/sw.js");
get_x!(get_offline, "public/offline.html");
get_x!(get_512, "public/512x512.png");
get_x!(get_256, "public/256x256.png");
get_x!(get_people_csv_example, "public/people_example.csv");
get_x!(get_events_csv_example, "public/events_example.csv");
get_x!(get_robots_txt, "public/robots.txt");
get_x!(get_buttons, "public/purple.css");

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
        .route("/512x512.png", get(get_512))
        .route("/256x256.png", get(get_256))
        .route("/people_example.csv", get(get_people_csv_example))
        .route("/events_example.csv", get(get_events_csv_example))
        .route("/robots.txt", get(get_robots_txt))
        .route("/purple.css", get(get_buttons))
}
