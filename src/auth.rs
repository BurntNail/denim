use crate::{
    auth::backend::DenimAuthBackend,
    error::{BcryptSnafu, DenimResult, MakeQuerySnafu},
};
use axum_login::AuthSession;
use bcrypt::hash;
use bitflags::bitflags;
use secrecy::{ExposeSecret, SecretString};
use snafu::ResultExt;
use sqlx::PgConnection;
use uuid::Uuid;

pub mod backend;
pub mod postgres_store;

pub type DenimSession = AuthSession<DenimAuthBackend>;

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct PermissionsTarget: u8 {
        const SIGN_SELF_UP =      0b0000_0001;
        const SIGN_OTHERS_UP =    0b0000_0010;

        const VERIFY_ATTENDANCE = 0b0000_0100;
        const CRUD_EVENTS =       0b0000_1000;
        const CRUD_USERS =        0b0001_0000;

        const SEE_PHOTOS =        0b0010_0000;
        const IMPORT_CSVS =       0b0100_0000;
        const EXPORT_CSVS =       0b1000_0000;
    }
}

pub async fn add_password(
    id: Uuid,
    password: SecretString,
    conn: &mut PgConnection,
    is_default: bool,
) -> DenimResult<()> {
    let hashed = hash(password.expose_secret(), bcrypt::DEFAULT_COST).context(BcryptSnafu)?;

    sqlx::query!("UPDATE users SET bcrypt_hashed_password = $2, current_password_is_default = $3 WHERE id = $1", id, hashed, is_default).execute(&mut *conn).await.context(MakeQuerySnafu)?;
    Ok(())
}
