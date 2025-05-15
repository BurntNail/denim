use crate::auth::{PermissionsTarget, backend::DenimAuthBackend};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use axum_login::tower_sessions::cookie::time::{OffsetDateTime, error::ComponentRange};
use chrono::{DateTime, Utc};
use maud::html;
use snafu::Snafu;
use std::num::ParseIntError;
use uuid::Uuid;

pub type DenimResult<T> = Result<T, DenimError>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum DenimError {
    #[snafu(display("Error opening database"))]
    OpenDatabase { source: sqlx::Error },
    #[snafu(display("Error getting db connection"))]
    GetDatabaseConnection { source: sqlx::Error },
    #[snafu(display("Error making SQL query"))]
    MakeQuery { source: sqlx::Error },
    #[snafu(display("Error commiting SQL transaction"))]
    CommitTransaction { source: sqlx::Error },
    #[snafu(display("Error rolling back SQL transaction"))]
    RollbackTransaction { source: sqlx::Error },
    #[snafu(display("Error migrating DB schema"))]
    MigrateError { source: sqlx::migrate::MigrateError },
    #[snafu(display("Error converting {} to `chrono::NaiveDateTime`", odt))]
    InvalidDateTime { odt: OffsetDateTime },
    #[snafu(display("Error converting {} to `time::OffsetDateTime`", utc_dt,))]
    InvalidChronoDateTime {
        source: ComponentRange,
        utc_dt: DateTime<Utc>,
    },
    #[snafu(display("Error serialising with rmp_serde"))]
    RmpSerdeEncode { source: rmp_serde::encode::Error },
    #[snafu(display("Error deserialising with rmp_serde"))]
    RmpSerdeDecode { source: rmp_serde::decode::Error },
    #[snafu(display("Unable to retrieve env var `{}`", name))]
    BadEnvVar {
        source: dotenvy::Error,
        name: &'static str,
    },
    #[snafu(display("Unable to parse IP port"))]
    ParsePort { source: ParseIntError },
    #[snafu(display("Unable to parse date {:?}", original))]
    ParseTime {
        source: chrono::ParseError,
        original: String,
    },
    #[snafu(display("Unable to parse uuid {:?}", original))]
    ParseUuid {
        source: uuid::Error,
        original: String,
    },
    #[snafu(display("Unable to find event with UUID: {}", id))]
    MissingEvent { id: Uuid },
    #[snafu(display("Unable to find user with UUID: {}", id))]
    MissingUser { id: Uuid },
    #[snafu(display("Unable to find house with ID: {}", id))]
    MissingHouseGroup { id: i32 },
    #[snafu(display("Unable to find tutor group with UUID: {}", id))]
    MissingTutorGroup { id: Uuid },
    #[snafu(display("Error with hashing/password verification"))]
    Bcrypt { source: bcrypt::BcryptError },
    #[snafu(display("Error with sessions"))]
    TowerSession {
        source: axum_login::tower_sessions::session::Error,
    },
    #[snafu(display("Unable to generate password"))]
    GeneratePassword,
    #[snafu(display(
        "Tried to get user information, found either no user or the incorrect kind of user"
    ))]
    UnableToFindUserInfo,
    #[snafu(display("Tried to {:?}, only had {:?}", needed.iter_names().collect::<Vec<_>>(), found.iter_names().collect::<Vec<_>>()))]
    IncorrectPermissions {
        needed: PermissionsTarget,
        found: PermissionsTarget,
    },
    #[snafu(display(
        "Tried to get the new student form, but no houses and/or no tutor groups existed to add them into"
    ))]
    NoHousesOrNoTutorGroups,
    #[snafu(display("Error with multipart form input"))]
    Multipart {
        source: axum::extract::multipart::MultipartError,
    },
    #[snafu(display("Error parsing email address"))]
    Email { source: email_address::Error },
    #[snafu(display("Error with ZIPs"))]
    Zip { source: zip::result::ZipError },
    #[snafu(display("Error with CSVs"))]
    Csv { source: csv::Error },
    #[snafu(display("Error with S3 Credentials"))]
    S3Creds {
        source: s3::creds::error::CredentialsError,
    },
    #[snafu(display("Error with S3"))]
    S3 { source: s3::error::S3Error },
    #[snafu(display("Error decoding Base64"))]
    B64 { source: base64::DecodeError },
}

impl From<axum_login::Error<DenimAuthBackend>> for DenimError {
    fn from(value: axum_login::Error<DenimAuthBackend>) -> Self {
        match value {
            axum_login::Error::Session(source) => Self::TowerSession { source },
            axum_login::Error::Backend(backend) => backend,
        }
    }
}

impl IntoResponse for DenimError {
    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn into_response(self) -> Response {
        const ISE: StatusCode = StatusCode::INTERNAL_SERVER_ERROR; //internal server error
        const NF: StatusCode = StatusCode::NOT_FOUND; //not found
        const NA: StatusCode = StatusCode::FORBIDDEN; //not allowed
        const BI: StatusCode = StatusCode::BAD_REQUEST; //bad input

        let basic_error = |desc| {
            html! {
                div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative mb-4" role="alert" {
                    strong class="font-bold" {"Denim Error"}
                    span {(desc)}
                }
            }
        };

        let status_code = match &self {
            Self::OpenDatabase { .. } | Self::GetDatabaseConnection { .. } => ISE,
            Self::MigrateError { .. } => ISE,
            Self::MakeQuery { source } => match source {
                sqlx::Error::RowNotFound => NF,
                _ => ISE,
            },
            Self::CommitTransaction { .. } | Self::RollbackTransaction { .. } => ISE,
            Self::InvalidDateTime { .. } => BI,
            Self::InvalidChronoDateTime { .. } => ISE,
            Self::RmpSerdeEncode { .. } => ISE,
            Self::RmpSerdeDecode { .. } => BI,
            Self::BadEnvVar { .. } => ISE,
            Self::ParsePort { .. } => ISE,
            Self::ParseTime { .. } => BI,
            Self::ParseUuid { .. } => BI,
            Self::MissingEvent { .. } => NF,
            Self::MissingUser { .. } => NF,
            Self::MissingHouseGroup { .. } => NF,
            Self::MissingTutorGroup { .. } => NF,
            Self::Bcrypt { .. } => ISE,
            Self::TowerSession { .. } => ISE,
            Self::GeneratePassword => ISE,
            Self::UnableToFindUserInfo => NF,
            Self::IncorrectPermissions { .. } => NA,
            Self::NoHousesOrNoTutorGroups => ISE,
            Self::Multipart { source } => source.status(),
            Self::Email { .. } => ISE,
            Self::Zip { .. } => ISE,
            Self::Csv { .. } => ISE,
            Self::S3Creds { .. } | Self::S3 { .. } => ISE,
            Self::B64 { .. } => BI,
        };

        error!(?self, "Error!");
        (status_code, Html(basic_error(self.to_string()))).into_response()
    }
}
