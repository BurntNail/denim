use crate::{
    auth::{DenimSession, add_password},
    data::{
        DataType,
        user::{AddPersonForm, User},
    },
    error::{DenimResult, MakeQuerySnafu},
    maud_conveniences::{errors_list, title},
    state::DenimState,
};
use axum::{
    Form,
    body::Body,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use bitflags::bitflags;
use email_address::EmailAddress;
use maud::html;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::ResultExt;
use crate::maud_conveniences::{form_submit_button, simple_form_element};
//flow:
// 1. create account
// done :)

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    struct NewAdminDetailsError: u16 {
        const EMPTY_FIRST_NAME =  0b0000_0000_0000_0001;
        const EMPTY_SURNAME =     0b0000_0000_0000_0010;
        const EMPTY_PASSWORD =    0b0000_0000_0000_0100;
        const EMPTY_EMAIL =       0b0000_0000_0000_1000;

        const INVALID_EMAIL =     0b0000_0000_0001_0000;
        const MISMATCH_PASSWORD = 0b0000_0000_0010_0000;
    }
}

impl NewAdminDetailsError {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|x| match x {
            Self::EMPTY_FIRST_NAME => Some("Provided First Name was empty"),
            Self::EMPTY_SURNAME => Some("Provided surname was empty"),
            Self::EMPTY_PASSWORD => Some("Provided password was empty"),
            Self::EMPTY_EMAIL => Some("Provided email was empty"),
            Self::INVALID_EMAIL => Some("Provided email wasn't valid"),
            Self::MISMATCH_PASSWORD => Some("Passwords didn't match"),
            _ => None,
        })
    }
}

#[derive(Deserialize)]
pub struct NewAdminCreationQuery {
    errors: Option<u16>,
}

pub async fn get_create_new_admin(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(NewAdminCreationQuery { errors }): Query<NewAdminCreationQuery>,
) -> DenimResult<Response<Body>> {
    //double check that no users exist
    if sqlx::query!("SELECT exists(SELECT 1 FROM public.users)")
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false)
    {
        return Ok(Redirect::to("/").into_response());
    }

    let errors = errors.map_or_else(
        NewAdminDetailsError::empty,
        NewAdminDetailsError::from_bits_truncate,
    );

    Ok(state.render(session, html! {
        div class="flex items-center justify-center" {
            div class="bg-gray-800 p-8 rounded-lg shadow-xl w-full max-w-md" {
                (title("Create new Admin Account"))

                @if !errors.is_empty() {
                    (errors_list(errors.as_nice_list()))
                }

                form method="post" {
                    (simple_form_element("first_name", "First Name", true, None, None))
                    (simple_form_element("pref_name", "Preferred Name", false, None, None))
                    (simple_form_element("surname", "Surname", true, None, None))
                    (simple_form_element("email", "Email", true, Some("email"), None))
                    
                    (simple_form_element("password", "Password", true, Some("password"), None))
                    (simple_form_element("confirm_password", "Confirm Password", true, Some("password"), None))
                    
                    (form_submit_button(Some("Create Admin User")))
                }
            }
        }
    }).into_response())
}

#[derive(Deserialize)]
pub struct CreateAdminAccountForm {
    first_name: String,
    pref_name: String,
    surname: String,
    email: String,
    password: SecretString,
    confirm_password: SecretString,
}

pub async fn post_add_new_admin(
    State(state): State<DenimState>,
    mut session: DenimSession,
    Form(CreateAdminAccountForm {
        first_name,
        pref_name,
        surname,
        email,
        password,
        confirm_password,
    }): Form<CreateAdminAccountForm>,
) -> DenimResult<Redirect> {
    let mut conn = state.get_transaction().await?;

    //double check that no users exist
    if sqlx::query!("SELECT exists(SELECT 1 FROM public.users)")
        .fetch_one(&mut *conn)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false)
    {
        return Ok(Redirect::to("/"));
    }

    let mut errors = NewAdminDetailsError::empty();
    if first_name.is_empty() {
        errors |= NewAdminDetailsError::EMPTY_FIRST_NAME;
    }
    if surname.is_empty() {
        errors |= NewAdminDetailsError::EMPTY_SURNAME;
    }
    if email.is_empty() {
        errors |= NewAdminDetailsError::EMPTY_EMAIL;
    }
    if !EmailAddress::is_valid(&email) {
        errors |= NewAdminDetailsError::INVALID_EMAIL;
    }
    if password.expose_secret().trim().is_empty() {
        errors |= NewAdminDetailsError::EMPTY_PASSWORD;
    }
    if password.expose_secret() != confirm_password.expose_secret() {
        errors |= NewAdminDetailsError::MISMATCH_PASSWORD;
    }

    if !errors.is_empty() {
        return Ok(Redirect::to(&format!(
            "/onboarding/create_admin_acc?errors={}",
            errors.bits()
        )));
    }

    let id = User::insert_into_database(
        AddPersonForm {
            first_name,
            pref_name,
            surname,
            email,
        },
        &mut conn,
    )
    .await?;
    sqlx::query!("INSERT INTO public.developers VALUES ($1)", id)
        .execute(&mut *conn)
        .await
        .context(MakeQuerySnafu)?;
    add_password(id, password, &mut conn, false).await?;

    let Some(user) = User::get_from_db_by_id(id, &mut conn).await? else {
        unreachable!("literally just added ;)")
    };
    conn.commit().await.context(MakeQuerySnafu)?;

    session.login(&user).await?;

    Ok(Redirect::to("/"))
}
