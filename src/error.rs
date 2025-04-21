use std::num::ParseIntError;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use snafu::Snafu;

pub type DenimResult<T> = Result<T, DenimError>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum DenimError {
    #[snafu(display("Error opening database: {}", source))]
    OpenDatabase {
        source: sqlx::Error
    },
    #[snafu(display("Error getting db connection: {}", source))]
    GetDatabaseConnection {
        source: sqlx::Error
    },
    #[snafu(display("Error making query: {}", source))]
    MakeQuery {
        source: sqlx::Error
    },
    #[snafu(display("Unable to retrieve env var {} because of {}", name, source))]
    BadEnvVar {
        source: dotenvy::Error,
        name: &'static str
    },
    #[snafu(display("Unable to parse port because {}", source))]
    ParsePort {
        source: ParseIntError
    }
}

impl IntoResponse for DenimError {
    fn into_response(self) -> Response {
        eprintln!("ERROR! {self:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, Html("whoopsies sorry")).into_response()
    }
}