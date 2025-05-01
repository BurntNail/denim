use crate::{
    data::{DataType, user::User},
    error::{BcryptSnafu, DenimError, MakeQuerySnafu},
    state::DenimState,
};
use async_trait::async_trait;
use axum_login::{AuthnBackend, UserId};
use secrecy::{ExposeSecret, SecretString};
use snafu::ResultExt;

//TODO: oauth2 related bits
//https://github.com/maxcountryman/axum-login/blob/main/examples/multi-auth/src/users.rs
#[derive(Clone)]
pub struct DenimAuthBackend {
    state: DenimState,
}

impl DenimAuthBackend {
    pub const fn new(state: DenimState) -> Self {
        Self { state }
    }
}

pub enum DenimAuthCredentials {
    EmailPassword {
        email: String,
        password: SecretString,
    },
}

#[async_trait]
impl AuthnBackend for DenimAuthBackend {
    type User = User;
    type Credentials = DenimAuthCredentials;
    type Error = DenimError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let mut conn = self.state.get_connection().await?;

        match creds {
            DenimAuthCredentials::EmailPassword { email, password } => {
                let Some(id) = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
                    .fetch_optional(&mut *conn)
                    .await
                    .context(MakeQuerySnafu)?
                else {
                    return Ok(None);
                };
                let Some(user) =
                    User::get_from_db_by_id(id.id, self.state.get_connection().await?).await?
                else {
                    unreachable!(
                        "we got the ID from the database, so not clear how we now don't have a user any more"
                    );
                };
                let Some(hash) = user.bcrypt_hashed_password.clone() else {
                    //TODO: work out way for people to add passwords
                    return Ok(None);
                };

                let password_verification_result = tokio::task::spawn_blocking(move || {
                    bcrypt::verify(password.expose_secret(), hash.expose_secret())
                })
                .await
                .expect("unable to join tokio task")
                .context(BcryptSnafu)?;

                Ok(if password_verification_result {
                    Some(user)
                } else {
                    None
                })
            }
        }
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        User::get_from_db_by_id(*user_id, self.state.get_connection().await?).await
    }
}
