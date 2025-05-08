#![allow(clippy::unused_async)]

use crate::{auth::DenimSession, error::DenimResult, maud_conveniences::title, state::DenimState};
use axum::{
    body::Body,
    extract::State,
    http::Response,
    response::{IntoResponse, Redirect},
};
use maud::{Markup, Render, html};
use snafu::OptionExt;
use crate::data::DataType;
use crate::data::event::Event;
use crate::data::user::UserKind;
use crate::error::{DenimError, UnableToFindUserInfoSnafu};
use crate::maud_conveniences::render_table;

pub async fn get_profile(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Response<Body>> {
    let Some(user) = session.user.clone() else {
        return Ok(Redirect::to("/").into_response());
    };
    let username = user.render();

    let display_first_name = internal_get_profile_first_name_display(session.clone()).await?;
    let display_pref_name = internal_get_profile_pref_name_display(session.clone()).await?;
    let display_surname = internal_get_profile_surname_display(session.clone()).await?;

    let display_email = internal_get_profile_email_display(session.clone()).await?;
    let display_password = internal_get_profile_password_display();

    let user_kind_specific = if matches!(user.kind, UserKind::Student {..}) {
        Some(internal_get_profile_student_display(State(state.clone()), session.clone()).await?)
    } else {
        None
    };

    Ok(state
        .render(
            session,
            html! {
                (title(username))
                div class="gap-x-4 flex flex-row w-xl" {
                    (display_first_name)
                    (display_pref_name)
                    (display_surname)
                }
                div class="gap-x-4 flex flex-row w-xl" {
                    (display_email)
                    (display_password)
                }
                @if let Some(user_kind_specific) = user_kind_specific {
                    div class="border-b border-gray-200 dark:border-gray-700 w-xl" {}
                    div class="w-xl my-4" {
                        (user_kind_specific)
                    }
                }
            },
        )
        .into_response())
}

pub async fn internal_get_profile_first_name_display(session: DenimSession) -> DenimResult<Markup> {
    let first_name = session.user.map(|x| x.first_name).context(UnableToFindUserInfoSnafu)?;

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4" {
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"First Name"}
            p class="text-gray-200" {(first_name)}
        }
    })
}

pub async fn internal_get_profile_pref_name_display(session: DenimSession) -> DenimResult<Markup> {
    let pref_name = session
        .user
        .context(UnableToFindUserInfoSnafu)?
        .pref_name;

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between container mx-auto bg-gray-800 rounded-md p-4" {
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Preferred Name"}
            @if let Some(pref_name) = pref_name {
                p class="text-gray-200" {(pref_name)}
            } @else {
                p class="text-gray-200 italic" {"N/A"}
            }
        }
    })
}

pub async fn internal_get_profile_surname_display(session: DenimSession) -> DenimResult<Markup> {
    let surname = session.user.map(|x| x.surname).context(UnableToFindUserInfoSnafu)?;

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between container mx-auto bg-gray-800 rounded-md p-4" {
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Surname"}
            br;
            p class="text-gray-200" {(surname)}
        }
    })
}

pub async fn internal_get_profile_email_display(session: DenimSession) -> DenimResult<Markup> {
    let email = session.user.map(|x| x.email).context(UnableToFindUserInfoSnafu)?;

    Ok(html! {
        div class="mb-4 flex flex-col items-center justify-between container mx-auto bg-gray-800 rounded-md p-4" {
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Email"}
            p class="text-gray-200" {(email)}
        }
    })
}

pub fn internal_get_profile_password_display () -> Markup {
    html! {
        div class="mb-4 flex flex-col items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4" {
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit Password?"}
        }
    }
}

pub async fn internal_get_profile_student_display (State(state): State<DenimState>, session: DenimSession) -> DenimResult<Markup> {
    let UserKind::Student {
        form: _,
        house: _,
        events_participated
    } = session.clone()
        .user
        .context(UnableToFindUserInfoSnafu)?
        .kind else {
        return Err(DenimError::UnableToFindUserInfo);
    };

    let mut event_details = Vec::with_capacity(events_participated.len());
    for event in events_participated {
        if let Some(event) = Event::get_from_db_by_id(event, state.get_connection().await?).await? {
            event_details.push([
                //TODO: link to event
                html!{
                    (event.name)
                },
                html!{
                    (event.date.format("%d/%m/%y"))
                }
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

pub async fn internal_get_profile_student_form_house_display (session: DenimSession) -> DenimResult<Markup> {
    let UserKind::Student {
        form,
        house,
        events_participated: _
    } = session
        .user
        .context(UnableToFindUserInfoSnafu)?
        .kind else {
        return Err(DenimError::UnableToFindUserInfo);
    };

    //TODO: link to form/house pages
    Ok(html!{
        div class="flex flex-col gap-4" {
            div class="flex flex-row gap-2" {
                p class="text-gray-200" {"Form: " (form.name)}
                p class="text-gray-200" {"House: " (house.name)}
            }
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit Form/House"}
        }
    })
}