use axum::{http::StatusCode, response::IntoResponse, Json};
use http::Uri;
use image::ImageFormat;
use snafu::Snafu;
use std::{
    ffi::OsString,
    fmt::{Debug, Display, Formatter},
    path::PathBuf,
};

#[derive(Copy, Clone, Debug)]
pub enum CommonHeaders {
    ContentType,
}

#[derive(Copy, Clone, Debug)]
pub enum HttpAction {
    BuildingResponse,
}

#[derive(Copy, Clone, Debug)]
pub enum ChannelReason {
    SendUpdateCalMessage,
}

#[derive(Copy, Clone, Debug)]
pub enum LettreAction {
    BuildMessage,
}

#[derive(Copy, Clone, Debug)]
pub enum ImageAction {
    GuessingFormat,
}

#[derive(Debug, Clone)]
pub enum FileIdentifier {
    Const(&'static str),
    Runtime(String),
    PB(PathBuf),
}

impl From<&'static str> for FileIdentifier {
    fn from(value: &'static str) -> Self {
        Self::Const(value)
    }
}
impl From<String> for FileIdentifier {
    fn from(value: String) -> Self {
        Self::Runtime(value)
    }
}
impl From<PathBuf> for FileIdentifier {
    fn from(value: PathBuf) -> Self {
        Self::PB(value)
    }
}

#[derive(Debug, Clone)]
pub enum IOAction {
    ReadingFile(FileIdentifier),
    OpeningFile(FileIdentifier),
    CreatingFile(FileIdentifier),
    DeletingFile(FileIdentifier),
    ReadingAndOpening(FileIdentifier),
    WritingToFile,
}

#[derive(Debug)]
pub enum ConvertingWhatToString {
    FileName(OsString),
    PathBuffer(PathBuf),
    // Header(CommonHeaders),
}

#[derive(Copy, Clone, Debug)]
pub enum SerdeJsonAction {
    ParsingLogFile,
}

#[derive(Copy, Clone, Debug)]
pub enum ThreadReason {
    FindingExistingFilesWithWalkDir,
    BuildSpreadsheet,
}

#[derive(Debug, Clone)]
pub enum DatabaseIDMethod {
    Id(i32),
    Username(String),
    Path(FileIdentifier),
}
impl From<i32> for DatabaseIDMethod {
    fn from(value: i32) -> Self {
        Self::Id(value)
    }
}
impl From<FileIdentifier> for DatabaseIDMethod {
    fn from(value: FileIdentifier) -> Self {
        Self::Path(value)
    }
}
impl From<String> for DatabaseIDMethod {
    fn from(value: String) -> Self {
        Self::Username(value)
    }
}

#[derive(Debug, Clone)]
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
    RemovingPerson(DatabaseIDMethod),

    FindingEvent(i32),
    UpdatingEvent(i32),
    FindingAllEvents,
    RemovingEvent(i32),
    AddingEvent,

    FindingParticipantOrPrefect {
        person: DatabaseIDMethod,
        event_id: i32,
    },
    UpdatingParticipantOrPrefect {
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
    MassVerifying {
        event_id: i32,
    },

    FindingPhotos(DatabaseIDMethod),
    RemovingPhoto(i32),
    AddingPhotos,

    AcquiringConnection,

    GettingRewards,
    GettingRewardsReceived(Option<DatabaseIDMethod>),
    AddingReward,
}

#[derive(Debug, Copy, Clone)]
pub struct MissingImageFormat;

impl Display for MissingImageFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("No image format"))
    }
}
impl std::error::Error for MissingImageFormat {}

#[derive(Snafu, Debug)]
#[snafu(visibility(pub))]
pub enum VentError {
    //external errors
    #[snafu(display("Database Error: {source:?}. Cause: {action:?}"))]
    Sqlx {
        source: sqlx::Error,
        action: SqlxAction,
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
    #[snafu(display("Error creating Zip File: {source:?}"), context(false))]
    Zip { source: async_zip::error::ZipError },
    #[snafu(display("Error with XLSX: {source:?}"), context(false))]
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
    #[snafu(display("Error parsing email address: {source:?}"), context(false))]
    LettreAddress {
        source: lettre::address::AddressError,
    },
    #[snafu(display("Error with Emails: {source:?}"))]
    LettreEmail {
        source: lettre::error::Error,
        trying_to: LettreAction,
    },
    #[snafu(display("Error with SMTP: {source:?}"), context(false))]
    LettreSMTP {
        source: lettre::transport::smtp::Error,
    },
    #[snafu(display("Error with CSV Files: {source:?}"), context(false))]
    Csv { source: csv_async::Error },
    #[snafu(display("JSON error: {source:?} whilst trying to {action:?}"))]
    SerdeJson {
        source: serde_json::Error,
        action: SerdeJsonAction,
    },
    #[snafu(display("Error with tower_sessions: {source:?}"))]
    TowerSessions {
        source: tower_sessions::session::Error,
    },
    #[snafu(display("Not able page {was_looking_for:?}"))]
    PageNotFound { was_looking_for: Uri },
    #[snafu(display("Unable to send message {source:?} trying to {reason:?}"))]
    SendError {
        source: tokio::sync::mpsc::error::SendError<()>,
        reason: ChannelReason,
    },
    #[snafu(display("Error with HTTP trying to {action:?} due to {source:?}"))]
    Http {
        source: http::Error,
        action: HttpAction,
    },

    // internal errors
    #[snafu(display("Missing Extension on: {was_looking_for:?}"))]
    MissingExtension { was_looking_for: PathBuf },
    #[snafu(display("Unknown MIME Type for File: {path:?}"))]
    UnknownMIME { path: PathBuf },
    #[snafu(display("Missing Cloudflare IP in headers"))]
    MissingCFIP,
}

#[axum::debug_handler]
pub async fn not_found_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}

impl IntoResponse for VentError {
    fn into_response(self) -> axum::response::Response {
        let code = match &self {
            VentError::Sqlx {
                source: _,
                action: trying_to_do,
            } if !matches!(trying_to_do, SqlxAction::AcquiringConnection) => StatusCode::NOT_FOUND,
            VentError::ParseTime { .. }
            | VentError::Headers { .. }
            | VentError::Multipart { .. }
            | VentError::Image { .. }
            | VentError::NoImageExtension { .. }
            | VentError::MissingCFIP => StatusCode::BAD_REQUEST,
            VentError::PageNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (code, Json(self.to_string())).into_response()
    }
}
