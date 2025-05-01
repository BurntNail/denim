use crate::{
    auth::backend::DenimAuthBackend,
    error::{BcryptSnafu, DenimResult, MakeQuerySnafu},
};
use axum_login::AuthSession;
use bcrypt::hash;
use secrecy::{ExposeSecret, SecretString};
use snafu::ResultExt;
use sqlx::{Postgres, pool::PoolConnection};
use uuid::Uuid;

pub mod backend;
pub mod postgres_store;

pub type DenimSession = AuthSession<DenimAuthBackend>;

pub async fn add_password(
    id: Uuid,
    password: SecretString,
    mut conn: PoolConnection<Postgres>,
    is_default: bool,
) -> DenimResult<()> {
    let hashed = hash(password.expose_secret(), bcrypt::DEFAULT_COST).context(BcryptSnafu)?;

    sqlx::query!("UPDATE users SET bcrypt_hashed_password = $2, current_password_is_default = $3 WHERE id = $1", id, hashed, is_default).execute(&mut *conn).await.context(MakeQuerySnafu)?;
    Ok(())
}
