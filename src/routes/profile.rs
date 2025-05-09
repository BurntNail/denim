#![allow(clippy::unused_async)]

use crate::{
    auth::{DenimSession, PermissionsTarget, add_password},
    data::{
        DataType,
        event::Event,
        user::{FullUserNameDisplay, User, UserKind, UsernameDisplay},
    },
    error::{BcryptSnafu, DenimError, DenimResult, MakeQuerySnafu, UnableToFindUserInfoSnafu},
    maud_conveniences::{render_errors_list, render_table},
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    body::Body,
    extract::State,
    http::Response,
    response::{IntoResponse, Redirect},
};
use bcrypt::verify;
use bitflags::bitflags;
use email_address::EmailAddress;
use maud::{Markup, Render, html};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt};
use uuid::Uuid;

pub async fn get_profile(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Response<Body>> {
    let Some(user) = session.user.clone() else {
        return Ok(Redirect::to("/").into_response());
    };
    let username = FullUserNameDisplay(&user, UsernameDisplay::all()).render();

    let load_user_specific = matches!(user.kind, UserKind::Student { .. });

    let edit_button = |form_get_url: &str, name: &str| {
        html! {
            button hx-get={(form_get_url)} hx-target="#form_contents" class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit " (name)}
        }
    };

    let can_edit = user
        .get_permissions()
        .contains(PermissionsTarget::CRUD_USERS);

    Ok(state
        .render(
            session,
            html! {
                div class="p-4" id="user_display" {
                    (username)
                    br;
                }
                @if can_edit {
                    div class="gap-x-4 mb-4 flex flex-col items-center justify-between space-y-4 container mx-auto bg-gray-800 rounded-md p-4 w-xl" {
                        div class="flex flex-row items-center justify-between space-x-4" {
                            (edit_button("/internal/profile/edit_first_name", "First Name"))
                            (edit_button("/internal/profile/edit_pref_name", "Preferred Name"))
                            (edit_button("/internal/profile/edit_surname", "Surname"))
                        }
                        div class="flex flex-row items-center justify-between space-x-4" {
                            (edit_button("/internal/profile/edit_email", "Email"))
                            (edit_button("/internal/profile/edit_password", "Password"))
                        }
                    }
                    div id="form_contents" class="bg-gray-800 rounded-md w-xl p-4" {}
                }
                @if load_user_specific {
                    div class="border-b border-gray-200 dark:border-gray-700 w-xl" {}
                    div hx-trigger="load" hx-get="/internal/profile/get_user_specific" class="w-xl my-4" {}
                }
            },
        )
        .into_response())
}

pub async fn internal_get_profile_student_display(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    let UserKind::Student {
        form: _,
        house: _,
        events_participated,
    } = session
        .clone()
        .user
        .context(UnableToFindUserInfoSnafu)?
        .kind
    else {
        return Err(DenimError::UnableToFindUserInfo);
    };

    let mut event_details = Vec::with_capacity(events_participated.len());
    for event in events_participated {
        if let Some(event) =
            Event::get_from_db_by_id(event, &mut *state.get_connection().await?).await?
        {
            event_details.push([
                //TODO: link to event
                html! {
                    (event.name)
                },
                html! {
                    (event.date.format("%d/%m/%y"))
                },
            ]);
        }
    }

    let form_house_display = internal_get_profile_student_form_house_display(session).await?;
    let events_table = render_table("Events", ["Event", "Date"], event_details);

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4 rounded-lg" {
            h2 class="text-lg font-semibold mb-2 text-gray-300 underline" {"Student Information"}
            (form_house_display)
            br;
            (events_table)
        }
    })
}

pub async fn internal_get_profile_student_form_house_display(
    session: DenimSession,
) -> DenimResult<Markup> {
    let UserKind::Student {
        form,
        house,
        events_participated: _,
    } = session.user.context(UnableToFindUserInfoSnafu)?.kind
    else {
        return Err(DenimError::UnableToFindUserInfo);
    };

    //TODO: link to form/house pages
    Ok(html! {
        div class="flex flex-col gap-4" {
            div class="flex flex-row gap-2" {
                p class="text-gray-200" {"Form: " (form.name)}
                p class="text-gray-200" {"House: " (house.name)}
            }
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit Form/House"}
        }
    })
}

fn get_edit_password_form(errors: ValidationError) -> Markup {
    html! {
        h2 class="text-2xl font-semibold mb-6 text-gray-300 text-center" {
            "Change Password"
        }

        @if !errors.is_empty() {
            (render_errors_list(errors.as_nice_list()))
        }

        form hx-post="/internal/profile/edit_password" hx-trigger="submit" class="p-4" hx-target="#form_contents" {
            div class="mb-4" {
                label for="current_password" class="block text-sm font-bold mb-2 text-gray-300" {"Current Password"}
                input required id="current_password" name="current_password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
            }
            div class="mb-4" {
                label for="new_password" class="block text-sm font-bold mb-2 text-gray-300" {"New Password"}
                input required id="new_password" name="new_password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
            }
            div class="mb-4" {
                label for="confirm_password" class="block text-sm font-bold mb-2 text-gray-300" {"Confirm Password"}
                input required id="confirm_password" name="confirm_password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
            }
            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Submit"}
            }
        }
    }
}

pub fn internal_get_profile_edit_password() -> Markup {
    get_edit_password_form(ValidationError::empty())
}

fn get_form(
    action: &'static str,
    title: &'static str,
    current: impl Render,
    label: impl Render,
    input_type: Option<&'static str>,
    errors: ValidationError,
) -> Markup {
    let input_type = input_type.unwrap_or("text");

    html! {
        h2 class="text-2xl font-semibold mb-6 text-gray-300 text-center" {
            (title)
        }

        @if !errors.is_empty() {
            (render_errors_list(errors.as_nice_list()))
        }

        form hx-post=(action) hx-trigger="submit" hx-target="#form_contents" class="p-4" {
            div class="mb-4" {
                label for="item" class="block text-sm font-bold mb-2 text-gray-300" {(label)}
                input id="item" name="item" type=(input_type) value=(current) class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
            }

            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Submit"}
            }
        }
    }
}

pub async fn internal_get_profile_edit_first_name(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_form(
        "/internal/profile/edit_first_name",
        "Change First Name",
        user.first_name,
        "First Name",
        None,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_pref_name(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_form(
        "/internal/profile/edit_pref_name",
        "Change Preferred Name",
        user.pref_name.as_deref().unwrap_or(""),
        "Preferred Name",
        None,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_surname(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_form(
        "/internal/profile/edit_surname",
        "Change Surname",
        user.surname,
        "Surname",
        None,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_email(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_form(
        "/internal/profile/edit_email",
        "Change Email",
        user.email,
        "Email",
        Some("email"),
        ValidationError::empty(),
    ))
}

#[derive(Deserialize)]
pub struct SingleItemForm {
    item: String,
}

#[derive(Deserialize)]
pub struct PasswordForm {
    current: SecretString,
    new: SecretString,
    confirmed: SecretString,
}

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    struct ValidationError: u8 {
        const EMPTY =                      0b0000_0001;
        const PASSWORDS_NOT_MATCH =        0b0000_0010;
        const CURRENT_PASSWORD_INCORRECT = 0b0000_0100;
        const ALREADY_TAKEN_EMAIL =        0b0000_1000;
        const INVALID_EMAIL =              0b0001_0000;
        const SAME_AS_BEFORE =             0b0010_0000;
    }
}

impl ValidationError {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|e| match e {
            Self::EMPTY => Some("Field was empty"),
            Self::PASSWORDS_NOT_MATCH => Some("Provided passwords do not match"),
            Self::CURRENT_PASSWORD_INCORRECT => Some("Provided current password was incorrect"),
            Self::ALREADY_TAKEN_EMAIL => Some("Provided email is already in use"),
            Self::INVALID_EMAIL => Some("Provided email is invalid"),
            Self::SAME_AS_BEFORE => Some("Field was the same as before"),
            _ => None,
        })
    }
}

enum ValidationResult {
    Invalid(ValidationError),
    InternalError(DenimError),
}

impl From<DenimError> for ValidationResult {
    fn from(value: DenimError) -> Self {
        Self::InternalError(value)
    }
}
impl From<ValidationError> for ValidationResult {
    fn from(value: ValidationError) -> Self {
        Self::Invalid(value)
    }
}

async fn handle_change_result(
    res: Result<User, ValidationResult>,
    form: impl Fn(ValidationError) -> Markup,
    mut session: DenimSession,
) -> DenimResult<Markup> {
    match res {
        Ok(user) => {
            session.login(&user).await?;
            let username = FullUserNameDisplay(&user, UsernameDisplay::all());

            Ok(html! {
                div hx-swap-oob="innerHTML:#user_display" {
                    (username)
                    br;
                }
                div hx-swap-oob="innerHTML:#nav_username" {
                    (user)
                }
            })
        }
        Err(ValidationResult::InternalError(e)) => Err(e),
        Err(ValidationResult::Invalid(e)) => Ok(form(e)),
    }
}

pub async fn internal_post_profile_edit_password(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(password_form): Form<PasswordForm>,
) -> DenimResult<Markup> {
    async fn change_password(
        PasswordForm {
            current,
            new,
            confirmed,
        }: PasswordForm,
        state: DenimState,
        id: Uuid,
        hash: Option<SecretString>,
    ) -> Result<User, ValidationResult> {
        let mut errors = ValidationError::empty();
        if new.expose_secret() == current.expose_secret() {
            errors |= ValidationError::SAME_AS_BEFORE;
        }

        let password_is_valid = {
            if let Some(bcrypt_hashed_password) = hash {
                tokio::task::spawn_blocking(move || {
                    let exposed_hash = bcrypt_hashed_password.expose_secret();
                    let exposed_current_try = current.expose_secret();

                    verify(exposed_current_try, exposed_hash).context(BcryptSnafu)
                })
                .await
                .expect("unable to join tokio task")?
            } else {
                true
            }
        };
        if !password_is_valid {
            errors |= ValidationError::CURRENT_PASSWORD_INCORRECT;
        }

        if new.expose_secret().is_empty() {
            errors |= ValidationError::EMPTY;
        }
        if new.expose_secret() != confirmed.expose_secret() {
            errors |= ValidationError::PASSWORDS_NOT_MATCH;
        }

        if !errors.is_empty() {
            return Err(ValidationResult::Invalid(errors));
        }

        add_password(id, new, &mut *state.get_connection().await?, false).await?;
        let Some(user) = User::get_from_db_by_id(id, &mut *state.get_connection().await?).await?
        else {
            unreachable!("already been having fun with this user")
        }; //ensure we get correct new user, in case add_password makes any changes that are important

        Ok(user)
    }

    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    user.ensure_can(PermissionsTarget::CRUD_USERS)?;

    handle_change_result(
        change_password(password_form, state, user.id, user.bcrypt_hashed_password).await,
        get_edit_password_form,
        session,
    )
    .await
}

pub async fn internal_post_profile_edit_first_name(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(SingleItemForm { item }): Form<SingleItemForm>,
) -> DenimResult<Markup> {
    async fn change_first_name(
        first_name: String,
        current: &str,
        state: DenimState,
        id: Uuid,
    ) -> Result<User, ValidationResult> {
        if first_name.is_empty() {
            return Err(ValidationResult::Invalid(ValidationError::EMPTY));
        }
        if first_name == current {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET first_name = $1 WHERE id = $2",
            first_name,
            id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        let Some(user) = User::get_from_db_by_id(id, &mut conn).await? else {
            unreachable!("already been having fun with this user")
        }; //ensure we get correct new user, in case add_password makes any changes that are important
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(user)
    }

    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    user.ensure_can(PermissionsTarget::CRUD_USERS)?;

    handle_change_result(
        change_first_name(item, &user.first_name, state, user.id).await,
        move |e| {
            get_form(
                "/internal/profile/edit_first_name",
                "Change First Name",
                &user.first_name,
                "First Name",
                None,
                e,
            )
        },
        session,
    )
    .await
}
pub async fn internal_post_profile_edit_pref_name(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(SingleItemForm { item }): Form<SingleItemForm>,
) -> DenimResult<Markup> {
    async fn change_pref_name(
        pref_name: String,
        current: Option<&str>,
        state: DenimState,
        id: Uuid,
    ) -> Result<User, ValidationResult> {
        let pref_name = if pref_name.is_empty() {
            None
        } else {
            Some(pref_name)
        };

        if pref_name.as_deref() == current {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET pref_name = $1 WHERE id = $2",
            pref_name,
            id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        let Some(user) = User::get_from_db_by_id(id, &mut conn).await? else {
            unreachable!("already been having fun with this user")
        }; //ensure we get correct new user, in case add_password makes any changes that are important
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(user)
    }

    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    user.ensure_can(PermissionsTarget::CRUD_USERS)?;

    handle_change_result(
        change_pref_name(item, user.pref_name.as_deref(), state, user.id).await,
        |e| {
            get_form(
                "/internal/profile/edit_pref_name",
                "Change Preferred Name",
                user.pref_name.as_deref().unwrap_or(""),
                "Preferred Name",
                None,
                e,
            )
        },
        session,
    )
    .await
}
pub async fn internal_post_profile_edit_surname(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(SingleItemForm { item }): Form<SingleItemForm>,
) -> DenimResult<Markup> {
    async fn change_surname(
        surname: String,
        current: &str,
        state: DenimState,
        id: Uuid,
    ) -> Result<User, ValidationResult> {
        if surname.is_empty() {
            return Err(ValidationResult::Invalid(ValidationError::EMPTY));
        }
        if surname == current {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!("UPDATE users SET surname = $1 WHERE id = $2", surname, id)
            .execute(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;

        let Some(user) = User::get_from_db_by_id(id, &mut conn).await? else {
            unreachable!("already been having fun with this user")
        }; //ensure we get correct new user, in case add_password makes any changes that are important
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(user)
    }

    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    user.ensure_can(PermissionsTarget::CRUD_USERS)?;

    handle_change_result(
        change_surname(item, &user.surname, state, user.id).await,
        move |e| {
            get_form(
                "/internal/profile/edit_surname",
                "Change Surname",
                &user.surname,
                "Surname",
                None,
                e,
            )
        },
        session,
    )
    .await
}
pub async fn internal_post_profile_edit_email(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(SingleItemForm { item }): Form<SingleItemForm>,
) -> DenimResult<Markup> {
    async fn change_email(
        email: String,
        current: &str,
        state: DenimState,
        id: Uuid,
    ) -> Result<User, ValidationResult> {
        if email.is_empty() {
            return Err(ValidationResult::Invalid(ValidationError::EMPTY));
        }
        let mut errors = ValidationError::empty();

        if !EmailAddress::is_valid(&email) {
            errors |= ValidationError::INVALID_EMAIL;
        }
        if email == current {
            errors |= ValidationError::SAME_AS_BEFORE;
            //theoretically we can't get both lol tho
        }

        if !errors.is_empty() {
            return Err(ValidationResult::Invalid(errors));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!("UPDATE users SET email = $1 WHERE id = $2", email, id)
            .execute(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;

        let Some(user) = User::get_from_db_by_id(id, &mut conn).await? else {
            unreachable!("already been having fun with this user")
        }; //ensure we get correct new user, in case add_password makes any changes that are important
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(user)
    }

    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    user.ensure_can(PermissionsTarget::CRUD_USERS)?;

    handle_change_result(
        change_email(item, &user.email, state, user.id).await,
        |e| {
            get_form(
                "/internal/profile/edit_email",
                "Change Email",
                &user.email,
                "Email",
                None,
                e,
            )
        },
        session,
    )
    .await
}
