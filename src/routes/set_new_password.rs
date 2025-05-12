use crate::{
    auth::{DenimSession, PasswordUserId, add_password},
    error::{BcryptSnafu, DenimResult},
    maud_conveniences::{errors_list, title},
    state::DenimState,
};
use axum::{
    Form,
    body::Body,
    extract::{Query, State},
    http::Response,
    response::{IntoResponse, Redirect},
};
use bcrypt::verify;
use bitflags::bitflags;
use maud::html;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::ResultExt;

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct ReplaceDefaultPasswordValidationError: u8 {
        const SAME_AS_BEFORE = 0b0000_0001;
        const DIDNT_MATCH =    0b0000_0010;
        const EMPTY =          0b0000_0100;
    }
}

impl ReplaceDefaultPasswordValidationError {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|e| match e {
            Self::SAME_AS_BEFORE => Some("Provided password was same as default"),
            Self::DIDNT_MATCH => Some("Provided passwords didn't match"),
            Self::EMPTY => Some("Provided password was empty"),
            _ => None,
        })
    }
}

#[derive(Deserialize)]
pub struct SetPasswordQuery {
    next: String,
    validation_errors: Option<u8>,
}

pub async fn get_replace_default_password(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(SetPasswordQuery {
        next,
        validation_errors,
    }): Query<SetPasswordQuery>,
) -> Response<Body> {
    if session
        .user
        .as_ref()
        .is_none_or(|user| !user.current_password_is_default)
    {
        return Redirect::to("/").into_response();
    }

    let validation_errors = validation_errors.map_or_else(
        ReplaceDefaultPasswordValidationError::empty,
        ReplaceDefaultPasswordValidationError::from_bits_truncate,
    );

    state.render(session, html!{
        div class="bg-gray-800 shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-md" {
            (title("Replace Default Password"))
            @if !validation_errors.is_empty() {
                (errors_list(validation_errors.as_nice_list()))
            }
            form method="post" {
                input type="hidden" id="next" name="next" value={(next)};
                div class="mb-4" {
                    label for="new_password" class="block text-sm font-bold mb-2 text-gray-300" {"New Password"}
                    input required id="new_password" name="new_password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
                }
                div class="mb-4" {
                    label for="confirmed_password" class="block text-sm font-bold mb-2 text-gray-300" {"Confirm Password"}
                    input required id="confirmed_password" name="confirmed_password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
                }
                div class="flex items-center justify-between" {
                    button type="submit" class="bg-green-500 hover:bg-green-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Set New Password"}
                }
            }
        }
    }).into_response()
}

#[derive(Deserialize)]
pub struct SetPasswordForm {
    next: String,
    new_password: SecretString,
    confirmed_password: SecretString,
}

pub async fn post_replace_default_password(
    State(state): State<DenimState>,
    mut session: DenimSession,
    Form(SetPasswordForm {
        next,
        new_password,
        confirmed_password,
    }): Form<SetPasswordForm>,
) -> DenimResult<Redirect> {
    let Some(user) = session.user.clone() else {
        return Ok(Redirect::to("/"));
    };
    if !user.current_password_is_default {
        return Ok(Redirect::to("/"));
    }

    let mut errors = ReplaceDefaultPasswordValidationError::empty();
    if new_password.expose_secret() != confirmed_password.expose_secret() {
        errors |= ReplaceDefaultPasswordValidationError::DIDNT_MATCH;
    }
    if new_password.expose_secret().trim().is_empty() {
        errors |= ReplaceDefaultPasswordValidationError::EMPTY;
    }
    let password_is_same_as_before = {
        if let Some(bcrypt_hashed_password) = user.bcrypt_hashed_password.clone() {
            let new_password = new_password.clone();
            tokio::task::spawn_blocking(move || {
                let exposed_hash = bcrypt_hashed_password.expose_secret();
                let exposed_new_try = new_password.expose_secret();

                verify(exposed_new_try, exposed_hash).context(BcryptSnafu)
            })
            .await
            .expect("unable to join tokio task")?
        } else {
            false
        }
    };
    if password_is_same_as_before {
        errors |= ReplaceDefaultPasswordValidationError::SAME_AS_BEFORE;
    }

    if !errors.is_empty() {
        return Ok(Redirect::to(&format!(
            "/replace_default_password?next={next}&validation_errors={}",
            errors.bits()
        )));
    }

    let mut conn = state.get_connection().await?;

    let PasswordUserId::FullUser(user) =
        add_password(user.into(), new_password, &mut conn, false).await?
    else {
        unreachable!("passed in a user")
    };

    session.login(&user).await?;

    Ok(Redirect::to(&next))
}
