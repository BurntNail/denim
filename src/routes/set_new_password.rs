use crate::{
    auth::{DenimSession, add_password},
    data::{DataType, user::User},
    error::{BcryptSnafu, DenimResult},
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
        .map_or(true, |user| !user.current_password_is_default)
    {
        return Redirect::to("/").into_response();
    }

    let validation_errors = match validation_errors {
        None => ReplaceDefaultPasswordValidationError::empty(),
        Some(n) => ReplaceDefaultPasswordValidationError::from_bits_truncate(n),
    };

    state.render(session, html!{
        div class="bg-gray-800 shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-md" {
            h2 class="text-2xl font-semibold mb-6 text-gray-300 text-center" {"Replace Default Password"}
            @if !validation_errors.is_empty() {
                div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative mb-4" role="alert" {
                    strong class="font-bold" {"Errors:"}
                    ul class="list-disc pl-5" {
                        @for error in validation_errors.as_nice_list() {
                            li {(error)}
                        }
                    }
                }
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
        if let Some(bcrypt_hashed_password) = user.bcrypt_hashed_password {
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

    add_password(
        user.id,
        new_password,
        &mut *state.get_connection().await?,
        false,
    )
    .await?;
    let Some(user) = User::get_from_db_by_id(user.id, &mut *state.get_connection().await?).await?
    else {
        unreachable!("already been having fun with this user")
    }; //ensure we get correct new user, in case add_password makes any changes that are important

    session.login(&user).await?;

    Ok(Redirect::to(&next))
}
