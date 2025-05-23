#![allow(clippy::unused_async)]

use crate::{
    auth::{AuthUtilities, DenimSession, PasswordUserId, PermissionsTarget, add_password},
    data::{
        DataType,
        event::Event,
        user::{FullUserNameDisplay, User, UserKind, UsernameDisplay},
    },
    error::{BcryptSnafu, DenimError, DenimResult, MakeQuerySnafu, UnableToFindUserInfoSnafu},
    maud_conveniences::{
        Email, errors_list, form_submit_button, simple_form_element, subtitle, supertitle, table,
    },
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
                } @else {
                    div class="flex flex-col items-center justify-center p-4" {
                        div class="space-y-4" {
                            div {
                                p class="text-gray-300 text-sm" {"First Name"}
                                p class="text-gray-100 text-lg font-medium" {(user.first_name)}
                            }
                            div {
                                p class="text-gray-300 text-sm" {"Preferred Name"}
                                p class="text-gray-100 text-lg font-medium" {(user.pref_name.unwrap_or_default())}
                            }
                            div {
                                p class="text-gray-300 text-sm" {"Surname"}
                                p class="text-gray-100 text-lg font-medium" {(user.surname)}
                            }
                            div {
                                p class="text-gray-300 text-sm" {"First Name"}
                                p class="text-gray-100 text-lg font-medium" {(Email(&user.email))}
                            }

                        }
                    }
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
        tutor_group: _,
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

    let dlc = state.config().date_locale_config().get()?;

    for event in events_participated {
        if let Some(event) =
            Event::get_from_db_by_id(event, &mut *state.get_connection().await?).await?
        {
            event_details.push([
                html! {
                    a href={"/event/" (event.id)} class="underline hover:text-blue-300" {(event.name)}
                },
                html! {
                    (dlc.short_ymd(&event.datetime)?)
                },
            ]);
        }
    }

    let form_house_display = internal_get_profile_student_form_house_display(session).await?;
    let events_table = table(subtitle("Events"), ["Event", "Date"], event_details);

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4 rounded-lg" {
            (subtitle("Student Information"))
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
        tutor_group,
        house,
        events_participated: _,
    } = session.user.context(UnableToFindUserInfoSnafu)?.kind
    else {
        return Err(DenimError::UnableToFindUserInfo);
    };

    //TODO: link to tg/house pages
    Ok(html! {
        div class="flex flex-col gap-4" {
            div class="flex flex-row gap-2" {
                p class="text-gray-200" {"Tutor Group: " (tutor_group.staff_member)}
                p class="text-gray-200" {"House: " (house.name)}
            }
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit Form/House"}
        }
    })
}

fn get_edit_password_form(errors: ValidationError) -> Markup {
    html! {
        (supertitle("Change Password"))

        @if !errors.is_empty() {
            (errors_list(None, errors.as_nice_list()))
        }

        form hx-post="/internal/profile/edit_password" hx-trigger="submit" class="p-4" hx-target="#form_contents" {
            (simple_form_element("current", "Current Password", true, Some("password"), None))
            (simple_form_element("new", "New Password", true, Some("password"), None))
            (simple_form_element("confirmed", "Confirm New Password", true, Some("password"), None))

            (form_submit_button(Some("Change Password")))
        }
    }
}

pub fn internal_get_profile_edit_password() -> Markup {
    get_edit_password_form(ValidationError::empty())
}

fn get_one_item_form(
    action: &'static str,
    form_title: &'static str,
    current: &str,
    label: impl Render,
    input_type: Option<&'static str>,
    required: bool,
    errors: ValidationError,
) -> Markup {
    html! {
        (subtitle(form_title))

        @if !errors.is_empty() {
            (errors_list(None, errors.as_nice_list()))
        }

        form hx-post=(action) hx-trigger="submit" hx-target="#form_contents" class="p-4" {
            (simple_form_element("item", label, required, input_type, Some(current)))
            (form_submit_button(Some("Change")))
        }
    }
}

pub async fn internal_get_profile_edit_first_name(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_one_item_form(
        "/internal/profile/edit_first_name",
        "Change First Name",
        &user.first_name,
        "First Name",
        None,
        true,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_pref_name(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_one_item_form(
        "/internal/profile/edit_pref_name",
        "Change Preferred Name",
        user.pref_name.as_deref().unwrap_or(""),
        "Preferred Name",
        None,
        false,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_surname(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_one_item_form(
        "/internal/profile/edit_surname",
        "Change Surname",
        &user.surname,
        "Surname",
        None,
        true,
        ValidationError::empty(),
    ))
}
pub async fn internal_get_profile_edit_email(session: DenimSession) -> DenimResult<Markup> {
    let user = session.user.context(UnableToFindUserInfoSnafu)?;

    Ok(get_one_item_form(
        "/internal/profile/edit_email",
        "Change Email",
        user.email.as_str(),
        "Email",
        Some("email"),
        true,
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
        current_user: User,
    ) -> Result<User, ValidationResult> {
        let mut errors = ValidationError::empty();
        if new.expose_secret() == current.expose_secret() {
            errors |= ValidationError::SAME_AS_BEFORE;
        }

        let password_is_valid = {
            if let Some(bcrypt_hashed_password) = current_user.bcrypt_hashed_password.clone() {
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

        let PasswordUserId::FullUser(user) = add_password(
            current_user.into(),
            new,
            &mut *state.get_connection().await?,
            false,
        )
        .await?
        else {
            unreachable!("passed in user");
        };
        Ok(user)
    }

    let user = session
        .user
        .clone()
        .expect("cannot change password w/o signing in");

    handle_change_result(
        change_password(password_form, state, user).await,
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
        state: DenimState,
        mut current_user: User,
    ) -> Result<User, ValidationResult> {
        if first_name.is_empty() {
            return Err(ValidationResult::Invalid(ValidationError::EMPTY));
        }
        if first_name == current_user.first_name {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET first_name = $1 WHERE id = $2",
            first_name,
            current_user.id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        current_user.first_name = first_name;
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(current_user)
    }

    session.ensure_can(PermissionsTarget::CRUD_USERS)?;
    let user = session
        .user
        .clone()
        .expect("cannot CRUD_USERS w/o logging in");
    let backup_first_name = user.first_name.clone();

    handle_change_result(
        change_first_name(item, state, user).await,
        move |e| {
            get_one_item_form(
                "/internal/profile/edit_first_name",
                "Change First Name",
                &backup_first_name,
                "First Name",
                None,
                true,
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
        state: DenimState,
        mut current_user: User,
    ) -> Result<User, ValidationResult> {
        let pref_name = if pref_name.is_empty() {
            None
        } else {
            Some(pref_name)
        };

        if pref_name.as_deref() == current_user.pref_name.as_deref() {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET pref_name = $1 WHERE id = $2",
            pref_name,
            current_user.id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        current_user.pref_name = pref_name;
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(current_user)
    }

    session.ensure_can(PermissionsTarget::CRUD_USERS)?;
    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    let backup_pref_name = user.pref_name.clone();

    handle_change_result(
        change_pref_name(item, state, user).await,
        |e| {
            get_one_item_form(
                "/internal/profile/edit_pref_name",
                "Change Preferred Name",
                backup_pref_name.as_deref().unwrap_or(""),
                "Preferred Name",
                None,
                false,
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
        state: DenimState,
        mut current_user: User,
    ) -> Result<User, ValidationResult> {
        if surname.is_empty() {
            return Err(ValidationResult::Invalid(ValidationError::EMPTY));
        }
        if surname == current_user.surname {
            return Err(ValidationResult::Invalid(ValidationError::SAME_AS_BEFORE));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET surname = $1 WHERE id = $2",
            surname,
            current_user.id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        current_user.surname = surname;
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(current_user)
    }

    session.ensure_can(PermissionsTarget::CRUD_USERS)?;
    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    let backup_surname = user.surname.clone();

    handle_change_result(
        change_surname(item, state, user).await,
        move |e| {
            get_one_item_form(
                "/internal/profile/edit_surname",
                "Change Surname",
                &backup_surname,
                "Surname",
                None,
                true,
                e,
            )
        },
        session,
    )
    .await
}

#[derive(Deserialize)]
pub struct EmailForm {
    email: EmailAddress,
}

pub async fn internal_post_profile_edit_email(
    session: DenimSession,
    State(state): State<DenimState>,
    Form(EmailForm { email }): Form<EmailForm>,
) -> DenimResult<Markup> {
    async fn change_email(
        email: EmailAddress,
        state: DenimState,
        mut current_user: User,
    ) -> Result<User, ValidationResult> {
        let mut errors = ValidationError::empty();

        if email == current_user.email {
            errors |= ValidationError::SAME_AS_BEFORE;
            //theoretically we can't get both lol tho
        }

        if !errors.is_empty() {
            return Err(ValidationResult::Invalid(errors));
        }

        let mut conn = state.get_connection().await?;

        sqlx::query!(
            "UPDATE users SET email = $1 WHERE id = $2",
            email.as_str(),
            current_user.id
        )
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;

        current_user.email = email;
        state.send_sse_event(SseEvent::CrudPerson);

        Ok(current_user)
    }

    session.ensure_can(PermissionsTarget::CRUD_USERS)?;
    let user = session.user.clone().context(UnableToFindUserInfoSnafu)?;
    let backup_email = user.email.clone();

    handle_change_result(
        change_email(email, state, user).await,
        |e| {
            get_one_item_form(
                "/internal/profile/edit_email",
                "Change Email",
                backup_email.as_str(),
                "Email",
                None,
                true,
                e,
            )
        },
        session,
    )
    .await
}
