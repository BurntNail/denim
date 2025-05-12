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
    #[snafu(display("Error opening database: {}", source))]
    OpenDatabase { source: sqlx::Error },
    #[snafu(display("Error getting db connection: {}", source))]
    GetDatabaseConnection { source: sqlx::Error },
    #[snafu(display("Error making query: {}", source))]
    MakeQuery { source: sqlx::Error },
    #[snafu(display("Error migrating DB schema: {}", source))]
    MigrateError { source: sqlx::migrate::MigrateError },
    #[snafu(display("Error converting {} to `chrono::NaiveDateTime`", odt))]
    InvalidDateTime { odt: OffsetDateTime },
    #[snafu(display(
        "Error converting {} to `time::OffsetDateTime` because {}",
        utc_dt,
        source
    ))]
    InvalidChronoDateTime {
        source: ComponentRange,
        utc_dt: DateTime<Utc>,
    },
    #[snafu(display("Error serialising with rmp_serde: {}", source))]
    RmpSerdeEncode { source: rmp_serde::encode::Error },
    #[snafu(display("Unable to retrieve env var {} because of {}", name, source))]
    BadEnvVar {
        source: dotenvy::Error,
        name: &'static str,
    },
    #[snafu(display("Unable to parse port because {}", source))]
    ParsePort { source: ParseIntError },
    #[snafu(display("Unable to parse date {:?} because of {}", original, source))]
    ParseTime {
        source: chrono::ParseError,
        original: String,
    },
    #[snafu(display("Unable to parse uuid {:?} because of {}", original, source))]
    ParseUuid {
        source: uuid::Error,
        original: String,
    },
    #[snafu(display("Unable to find event with UUID: {}", id))]
    MissingEvent { id: Uuid },
    #[snafu(display("Unable to find user with UUID: {}", id))]
    MissingUser { id: Uuid },
    #[snafu(display("Unable to find house with ID: {}", id))]
    MissingHouseGroup {id: i32},
    #[snafu(display("Unable to find tutor group with UUID: {}", id))]
    MissingTutorGroup {id: Uuid},
    #[snafu(display("Error with bcrypt: {}", source))]
    Bcrypt { source: bcrypt::BcryptError },
    #[snafu(display("Error with sessions: {}", source))]
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

        let (error_code, error_ty_desc) = match &self {
            Self::OpenDatabase { .. } => (ISE, basic_error("Opening Database")),
            Self::GetDatabaseConnection { .. } => (ISE, basic_error("Making Database Connection")),
            Self::MakeQuery { source } => match source {
                sqlx::Error::RowNotFound => (NF, basic_error("Database Item Not Found")),
                _ => (ISE, basic_error("Querying Database")),
            },
            Self::MigrateError { .. } => (ISE, basic_error("Running Database Migrations")),
            Self::InvalidDateTime { odt } => (
                BI,
                basic_error(&format!("Converting Date-Time Format, starting with {odt}")),
            ),
            Self::InvalidChronoDateTime { utc_dt, .. } => (
                ISE,
                basic_error(&format!(
                    "Converting Date-Time Format, starting with {utc_dt}"
                )),
            ),
            Self::RmpSerdeEncode { .. } => (ISE, basic_error("Serialising Session")),
            Self::BadEnvVar { name, source } => match source {
                dotenvy::Error::LineParse(_, _) => (
                    ISE,
                    basic_error(&format!("Parsing Environment Variable {name:?}")),
                ),
                dotenvy::Error::Io(_) => (ISE, basic_error("IO Error with `.env` file")),
                dotenvy::Error::EnvVar(ev) => match ev {
                    std::env::VarError::NotPresent => (
                        ISE,
                        basic_error(&format!("Environment Variable {ev:?} was not present")),
                    ),
                    std::env::VarError::NotUnicode(_) => (
                        ISE,
                        basic_error(&format!("Environment Variable {ev} was not unicode")),
                    ),
                },
                _ => (
                    ISE,
                    basic_error(&format!("Error with Environment Variable {name:?}")),
                ),
            },
            Self::ParsePort { .. } => (
                ISE,
                basic_error("Parsing port environment variable contents"),
            ),
            Self::ParseTime { .. } => (BI, basic_error("Parsing date-time from Form Data")),
            Self::ParseUuid { .. } => (BI, basic_error("Parsing UUID from Form Data")),
            Self::MissingEvent { id } => (
                NF,
                basic_error(&format!("Finding an event ({id}) in the DB")),
            ),
            Self::MissingUser { id } => {
                (NF, basic_error(&format!("Finding a user ({id}) in the DB")))
            }
            Self::MissingHouseGroup {id} => {
                (NF, basic_error(&format!("Finding a house ({id}) in the DB")))
            }
            Self::MissingTutorGroup {id} => {
                (NF, basic_error(&format!("Finding a tutor group ({id}) in the DB")))
            }
            Self::Bcrypt { .. } => (ISE, basic_error("Hashing")),
            Self::TowerSession { .. } => (ISE, basic_error("Dealing with Session Management")),
            Self::GeneratePassword => (ISE, basic_error("Generating a random password")),
            Self::UnableToFindUserInfo => (
                NF,
                basic_error("Finding the details of a user on a sign-in mandatory page"),
            ),
            Self::IncorrectPermissions { .. } => (
                NA,
                basic_error(
                    "Attempting to access/complete operations with insufficient permissions",
                ),
            ),
            Self::NoHousesOrNoTutorGroups => (
                ISE,
                basic_error(
                    "Trying to create a new student with either no houses and or tutor groups to add them to",
                ),
            ),
        };

        error!(?self, "Error!");
        (error_code, Html(error_ty_desc)).into_response()
    }
}
