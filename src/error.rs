use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use axum_login::tower_sessions::cookie::time::{OffsetDateTime, error::ComponentRange};
use chrono::{DateTime, Utc};
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
    #[snafu(display("Error with bcrypt: {}", source))]
    Bcrypt { source: bcrypt::BcryptError },
    #[snafu(display("Error with sessions: {}", source))]
    TowerSession {
        source: axum_login::tower_sessions::session::Error,
    },
    #[snafu(display("Unable to generate password"))]
    GeneratePassword,
    #[snafu(display("Tried to get user information, found either no user or the incorrect kind of user"))]
    UnableToFindUserInfo
}

impl IntoResponse for DenimError {
    fn into_response(self) -> Response {
        eprintln!("ERROR! {self:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, Html("whoopsies sorry")).into_response()
    }
}
