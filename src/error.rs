use crate::auth::cloudflare_turnstile::CommonHeaders;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use http::Uri;
use image::ImageFormat;
use snafu::Snafu;
use std::{
    error::Error,
    ffi::OsString,
    fmt::{Debug, Display, Formatter},
    path::PathBuf,
};

#[derive(Debug)]
pub enum ImageAction {
    GuessingFormat,
}

#[derive(Debug)]
pub enum IOAction {
    ReadingFile(String),
    OpeningFile(PathBuf),
    CreatingFile(String),
    WritingToFile,
    ReadingMetadata,
}

#[derive(Debug)]
pub enum ReqwestAction {
    CloudflareTurntile,
    ErrorForStatus(Option<StatusCode>),
    ConvertToJson(SerdeJsonAction),
}

#[derive(Debug)]
pub enum ConvertingWhatToString {
    FileName(OsString),
    PathBuffer(PathBuf),
    Header(CommonHeaders),
}

#[derive(Debug)]
pub enum SerdeJsonAction {
    TryingToLogin,
    CloudflareTurnstileResponse,
}

#[derive(Debug)]
pub enum LiquidAction {
    BuildingCompiler,
    Parsing { text: String },
    Rendering,
}

#[derive(Debug)]
pub enum ThreadReason {
    LiquidCompiler,
}

#[derive(Debug)]
pub enum DatabaseIDMethod {
    Id(i32),
    Username(String),
}

#[derive(Debug)]
pub enum SqlxAction {
    //TODO: possible refactor to 2 enums - action (eg. find, update), objects involved (eg. event, person)
    FindingPerson(DatabaseIDMethod),
    UpdatingPerson(DatabaseIDMethod),
    FindingPeople,
    UpdatingForms {
        old_name: String,
        new_name: String,
    },
    AddingPerson,

    FindingEvent(i32),
    UpdatingEvent(i32),
    FindingAllEvents,
    AddingEvent,

    FindingParticipantOrPrefect {
        person: DatabaseIDMethod,
        event_id: i32,
    },
    AddingParticipantOrPrefect {
        person: DatabaseIDMethod,
        event_id: i32,
    },
    FindingParticipantsOrPrefectsAtEvents {
        event_id: Option<i32>,
    },
    FindingEventsOnPeople {
        person: DatabaseIDMethod,
    },

    FindingPhotos {
        path: String,
    },
    AddingPhotos,

    FindingSecret,
    AddingSecret,

    AcquiringConnection,
}

#[derive(Debug, Copy, Clone)]
pub struct MissingImageFormat;

impl Display for MissingImageFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("No image format"))
    }
}
impl Error for MissingImageFormat {}

#[derive(Snafu, Debug)]
#[snafu(visibility(pub))]
pub enum KnotError {
    //external errors
    #[snafu(display("Database Error: {source:?}. Cause: {action:?}"))]
    Sqlx {
        source: sqlx::Error,
        action: SqlxAction,
    },
    #[snafu(display("Liquid Error: {source:?} caused by {attempt:?}"))]
    Liquid {
        source: liquid::Error,
        attempt: LiquidAction,
    },
    #[snafu(display("IO Error: {source:?} doing {action:?}"))]
    IO {
        source: std::io::Error,
        action: IOAction,
    },
    #[snafu(display("Tokio Join Error: {source:?} which was started to {title:?}"))]
    Join {
        source: tokio::task::JoinError,
        title: ThreadReason,
    },
    #[snafu(display("Error Parsing Integer: {source:?}"))]
    ParseInt { source: std::num::ParseIntError },
    #[snafu(display("Error Parsing Bool: {source:?}"))]
    ParseBool { source: std::str::ParseBoolError },
    #[snafu(display("Error Parsing {original:?} - {source:?}"))]
    ParseTime {
        source: chrono::ParseError,
        original: String,
    },
    #[snafu(display("Error in Headers: {source:?}"))]
    Headers {
        source: http::header::InvalidHeaderValue,
        which_header: CommonHeaders,
    },
    #[snafu(display("Multipart Error: {source:?}"), context(false))]
    Multipart {
        source: axum::extract::multipart::MultipartError,
    },
    #[snafu(display("Invalid Image: {source:?}"))]
    Image {
        source: image::error::ImageError,
        action: ImageAction,
    },
    #[snafu(display("Missing Image Extension: {extension:?}"))]
    NoImageExtension { extension: ImageFormat },
    #[snafu(display("Error creating Zip File: {source:?}"))]
    Zip { source: async_zip::error::ZipError },
    #[snafu(display("Error with XLSX: {source:?}"))]
    Xlsx { source: rust_xlsxwriter::XlsxError },
    #[snafu(display("Error with Encrypting: {source:?}"), context(false))]
    Bcrypt { source: bcrypt::BcryptError },
    #[snafu(display("Error converting {what:?} to string"))]
    ToStr { what: ConvertingWhatToString },
    #[snafu(display("Error converting {header:?} to string due to {source:?}"))]
    HeaderToStr {
        source: http::header::ToStrError,
        header: CommonHeaders,
    },
    #[snafu(display("Error reqwest-ing: {source:?} whilst trying to {action:?}"))]
    Reqwest {
        source: reqwest::Error,
        action: ReqwestAction,
    },
    #[snafu(display("Error parsing email address: {source:?}"))]
    LettreAddress {
        source: lettre::address::AddressError,
    },
    #[snafu(display("Error with Emails: {source:?}"))]
    LettreEmail { source: lettre::error::Error },
    #[snafu(display("Error with SMTP: {source:?}"))]
    LettreSMTP {
        source: lettre::transport::smtp::Error,
    },
    #[snafu(display("Error with CSV Files: {source:?}"))]
    Csv { source: csv_async::Error },
    #[snafu(display("JSON error: {source:?} whilst trying to {action:?}"))]
    SerdeJson {
        source: serde_json::Error,
        action: SerdeJsonAction,
    },
    #[snafu(display("Random Eyre Error: {source:?}"))]
    Eyre { source: eyre::Error }, //thanks axum_login ;)
    #[snafu(display("Not able page {was_looking_for:?}"))]
    PageNotFound { was_looking_for: Uri },

    // internal errors
    #[snafu(display("Missing Extension on: {was_looking_for:?}"))]
    MissingExtension { was_looking_for: PathBuf },
    #[snafu(display("Unknown MIME Type for File: {path:?}"))]
    UnknownMIME { path: PathBuf },
    #[snafu(display("CSV incorrect format"))]
    MalformedCSV,
    #[snafu(display("Missing Cloudflare IP in headers"))]
    MissingCFIP,
}

#[allow(clippy::needless_pass_by_value)]
pub fn get_error_page(error_code: StatusCode, content: KnotError) -> (StatusCode, Html<String>) {
    error!(
        ?content,
        ?error_code,
        "Dealing with Error page: {content:#?}"
    );

    (
        error_code,
        Html(format!(
            include_str!("../www/server_error.html"),
            error = content,
            code = error_code
        )),
    )
}

pub async fn not_found_fallback(uri: Uri) -> (StatusCode, Html<String>) {
    get_error_page(
        StatusCode::NOT_FOUND,
        KnotError::PageNotFound {
            was_looking_for: uri,
        },
    )
}

impl IntoResponse for KnotError {
    fn into_response(self) -> axum::response::Response {
        let code = match &self {
            KnotError::Sqlx {
                source: _,
                action: trying_to_do,
            } if !matches!(trying_to_do, SqlxAction::AcquiringConnection) => StatusCode::NOT_FOUND,
            KnotError::ParseInt { .. }
            | KnotError::ParseBool { .. }
            | KnotError::ParseTime { .. }
            | KnotError::Headers { .. }
            | KnotError::Multipart { .. }
            | KnotError::Image { .. }
            | KnotError::NoImageExtension { .. }
            | KnotError::MalformedCSV
            | KnotError::MissingCFIP => StatusCode::BAD_REQUEST,
            KnotError::PageNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        get_error_page(code, self).into_response()
    }
}
