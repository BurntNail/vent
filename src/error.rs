use crate::{auth::{backend::VentAuthBackend, cloudflare_turnstile::CommonHeaders}, image_format::ImageFormat};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use chrono::NaiveDateTime;
use http::Uri;
use snafu::Snafu;
use std::{
    ffi::OsString,
    fmt::{Debug, Display, Formatter},
    path::PathBuf,
};
use tower_sessions::session::Id;

pub type ALError = axum_login::Error<VentAuthBackend>;

#[derive(Debug)]
pub enum HttpAction {
    BuildingResponse,
}

#[derive(Debug)]
pub enum LoginFailureReason {
    PasswordIsNotSet,
    IncorrectPassword,
}

#[derive(Debug)]
pub enum ChannelReason {
    SendUpdateCalMessage,
}

#[derive(Debug)]
pub enum LettreAction {
    BuildMessage,
}

#[derive(Debug)]
pub enum WhatToParse {
    PartOfAPerson(PersonField),
    IdForRecord,
}

impl From<PersonField> for WhatToParse {
    fn from(value: PersonField) -> Self {
        Self::PartOfAPerson(value)
    }
}

#[derive(Debug)]
pub enum PersonField {
    FirstName,
    Surname,
    Form,
    IsPrefect,
    Username,
    WasFirstEntry,
}

#[derive(Debug)]
pub enum EventField {
    Location,
    Teacher,
    Date,
    Name,
    Time,
}

#[derive(Debug)]
pub enum TryingToGetFromCSV {
    Person(PersonField),
    Event(EventField),
}

impl From<PersonField> for TryingToGetFromCSV {
    fn from(value: PersonField) -> Self {
        Self::Person(value)
    }
}
impl From<EventField> for TryingToGetFromCSV {
    fn from(value: EventField) -> Self {
        Self::Event(value)
    }
}

#[derive(Debug)]
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
    FlushingFile,
}

#[derive(Debug)]
pub enum ReqwestAction {
    CloudflareTurntile,
    RErrorForStatus(Option<reqwest::StatusCode>),
    // HErrorForStatus(Option<http::StatusCode>),
    ConvertToJson(SerdeJsonAction),
}

#[derive(Debug)]
pub enum ConvertingWhatToString {
    FileName(OsString),
    PathBuffer(PathBuf),
    // Header(CommonHeaders),
}

#[derive(Debug)]
pub enum SerdeJsonAction {
    CloudflareTurnstileResponse,
    ParsingLogFile,
    SessionSerde,
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
    FindingExistingFilesWithWalkDir,
    BuildSpreadsheet,
}

#[derive(Debug)]
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

    RemovingPrefectOrPrefectFromEventByRI {
        relation_id: i32,
    },
    FindingParticipantOrPrefectByRI {
        relation_id: i32,
    },

    FindingPhotos(DatabaseIDMethod),
    RemovingPhoto(i32),
    AddingPhotos,

    AddingBonusPoint,
    UpdatingBonusPoint(i32),
    DeletingBonusPoint(i32),
    FindingPotentialParticipantsForBonusPoint(i32),
    FindingParticipantsForBonusPoint {
        person: DatabaseIDMethod,
        bonus_point_id: i32,
    },
    AddingParticipantToBonusPoint {
        person: DatabaseIDMethod,
        bonus_point_id: i32,
    },
    RemovingParticipantFromBonusPointByRI {
        relation_id: i32,
    },
    GettingBonusPoints,

    DeletingOldSessions,
    RemovingSession(Id),
    AddingSession,
    FindingSession(Id),

    AcquiringConnection,

    GettingRewards,
    GettingRewardsReceived(Option<DatabaseIDMethod>),
    AddingReward,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EncodeStep {
    Encode,
    Decode,
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
    #[snafu(display("Error Parsing Integer: {source:?} trying to get a {what_to_convert_to:?}"))]
    ParseInt {
        source: std::num::ParseIntError,
        what_to_convert_to: WhatToParse,
        how_got_in: EncodeStep,
    },
    #[snafu(display("Error Parsing Bool: {source:?} trying to convert for {trying_to_parse:?}"))]
    ParseBool {
        source: std::str::ParseBoolError,
        trying_to_parse: WhatToParse,
        how_got_in: EncodeStep,
    },
    #[snafu(display("Error Parsing {original:?} - {source:?}"))]
    ParseTime {
        source: chrono::ParseError,
        original: String,
        how_got_in: EncodeStep,
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
    #[snafu(display("Invalid Image"))]
    Image {
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
    #[snafu(display("Error reqwest-ing: {source:?} whilst trying to {action:?}"))]
    Reqwest {
        source: reqwest::Error,
        action: ReqwestAction,
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
    #[snafu(display("Error with time components out of range"))]
    ComponentRange {
        source: time::error::ComponentRange,
        naive: NaiveDateTime,
    },

    // internal errors
    #[snafu(display("Missing Extension on: {was_looking_for:?}"))]
    MissingExtension { was_looking_for: PathBuf },
    #[snafu(display("Unknown MIME Type for File: {path:?}"))]
    UnknownMIME { path: PathBuf },
    #[snafu(display("CSV incorrect format - trying to get {was_trying_to_get:?}"))]
    MalformedCSV {
        was_trying_to_get: TryingToGetFromCSV,
    },
    #[snafu(display("Missing Cloudflare IP in headers"))]
    MissingCFIP,
    #[snafu(display("Failure to login due to {reason:?}"))]
    LoginFailure { reason: LoginFailureReason },
}

impl From<ALError> for VentError {
    fn from(src: ALError) -> Self {
        match src {
            ALError::Session(source) => VentError::TowerSessions { source },
            ALError::Backend(be) => be,
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn get_error_page(error_code: StatusCode, content: VentError) -> (StatusCode, Html<String>) {
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

#[axum::debug_handler]
pub async fn not_found_fallback(uri: Uri) -> (StatusCode, Html<String>) {
    get_error_page(
        StatusCode::NOT_FOUND,
        VentError::PageNotFound {
            was_looking_for: uri,
        },
    )
}

impl IntoResponse for VentError {
    fn into_response(self) -> axum::response::Response {
        let code = match &self {
            VentError::Sqlx {
                source: _,
                action: trying_to_do,
            } if !matches!(trying_to_do, SqlxAction::AcquiringConnection) => StatusCode::NOT_FOUND,
            VentError::ParseInt { .. }
            | VentError::ParseBool { .. }
            | VentError::ParseTime { .. }
            | VentError::Headers { .. }
            | VentError::Multipart { .. }
            | VentError::Image { .. }
            | VentError::NoImageExtension { .. }
            | VentError::MalformedCSV { .. }
            | VentError::MissingCFIP
            | VentError::LoginFailure { .. } => StatusCode::BAD_REQUEST,
            VentError::PageNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        get_error_page(code, self).into_response()
    }
}

impl From<VentError> for tower_sessions::session_store::Error {
    fn from(value: VentError) -> Self {
        match &value {
            VentError::ParseInt { how_got_in, .. }
            | VentError::ParseBool { how_got_in, .. }
            | VentError::ParseTime { how_got_in, .. } => match *how_got_in {
                EncodeStep::Encode => {
                    tower_sessions::session_store::Error::Encode(value.to_string())
                }
                EncodeStep::Decode => {
                    tower_sessions::session_store::Error::Decode(value.to_string())
                }
            },
            _ => tower_sessions::session_store::Error::Backend(value.to_string()),
        }
    }
}
