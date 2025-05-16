use crate::{
    auth::backend::DenimAuthBackend,
    data::user::User,
    error::{BcryptSnafu, DenimError, DenimResult, MakeQuerySnafu},
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

pub trait AuthUtilities {
    fn can(&self, needed: PermissionsTarget) -> bool;
    fn ensure_can(&self, needed: PermissionsTarget) -> DenimResult<()>;
}

impl AuthUtilities for DenimSession {
    fn can(&self, needed: PermissionsTarget) -> bool {
        let Some(user) = self.user.as_ref() else {
            return false;
        };
        user.get_permissions().contains(needed)
    }

    fn ensure_can(&self, needed: PermissionsTarget) -> DenimResult<()> {
        let found = self
            .user
            .as_ref()
            .map_or_else(PermissionsTarget::empty, User::get_permissions);

        if found.contains(needed) {
            Ok(())
        } else {
            Err(DenimError::IncorrectPermissions { needed, found })
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct PermissionsTarget: u16 {
        const SIGN_SELF_UP =             0b0000_0000_0000_0001;
        const SIGN_OTHERS_UP =           0b0000_0000_0000_0010;

        const VERIFY_ATTENDANCE =        0b0000_0000_0000_0100;
        const CRUD_EVENTS =              0b0000_0000_0000_1000;
        const CRUD_USERS =               0b0000_0000_0001_0000;

        const VIEW_PHOTOS =              0b0000_0000_0010_0000;
        const IMPORT_CSVS =              0b0000_0000_0100_0000;
        const EXPORT_CSVS =              0b0000_0000_1000_0000;

        const CRUD_ADMINS =              0b0000_0001_0000_0000;
        const VIEW_SENSITIVE_DETAILS =   0b0000_0010_0000_0000;

        const RUN_ONBOARDING =           0b0000_0100_0000_0000;
    }
}

#[allow(clippy::large_enum_variant)]
pub enum PasswordUserId {
    FullUser(User),
    JustId(Uuid),
}

impl PasswordUserId {
    pub const fn id(&self) -> Uuid {
        match self {
            Self::FullUser(u) => u.id,
            Self::JustId(i) => *i,
        }
    }
}

impl From<User> for PasswordUserId {
    fn from(value: User) -> Self {
        Self::FullUser(value)
    }
}
impl From<Uuid> for PasswordUserId {
    fn from(value: Uuid) -> Self {
        Self::JustId(value)
    }
}

pub async fn add_password(
    mut current_user: PasswordUserId,
    password: SecretString,
    conn: &mut PgConnection,
    is_default: bool,
) -> DenimResult<PasswordUserId> {
    let hashed = hash(password.expose_secret(), bcrypt::DEFAULT_COST).context(BcryptSnafu)?;

    sqlx::query!("UPDATE users SET bcrypt_hashed_password = $2, current_password_is_default = $3 WHERE id = $1", current_user.id(), hashed, is_default).execute(&mut *conn).await.context(MakeQuerySnafu)?;

    if let PasswordUserId::FullUser(user) = &mut current_user {
        user.current_password_is_default = is_default;
        user.bcrypt_hashed_password = Some(hashed.into());
    }

    Ok(current_user)
}
