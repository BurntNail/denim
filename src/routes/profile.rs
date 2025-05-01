#![allow(clippy::unused_async)]

use crate::{auth::DenimSession, error::DenimResult, maud_conveniences::title, state::DenimState};
use axum::{
    body::Body,
    extract::State,
    http::Response,
    response::{IntoResponse, Redirect},
};
use maud::{Markup, Render, html};

pub async fn get_profile(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Response<Body>> {
    let Some(user) = session.user.clone() else {
        return Ok(Redirect::to("/").into_response());
    };
    let username = user.render();

    let display_first_name = internal_get_profile_first_name_display(session.clone()).await;
    let display_pref_name = internal_get_profile_pref_name_display(session.clone()).await;
    let display_surname = internal_get_profile_surname_display(session.clone()).await;

    Ok(state
        .render(
            session,
            html! {
                (title(username))

                div class="space-y-4 flex flex-col" {
                    (display_first_name)
                    (display_pref_name)
                    (display_surname)
                }
            },
        )
        .into_response())
}

pub async fn internal_get_profile_first_name_display(session: DenimSession) -> Markup {
    let first_name = session.user.map(|x| x.first_name).unwrap_or_default();

    html! {
        div class="mb-4 flex items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4" {
            label class="block text-sm font-bold mb-2 text-gray-300" {"First Name: "}
            p class="text-gray-200" {(first_name)}
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit?"}
        }
    }
}

pub async fn internal_get_profile_pref_name_display(session: DenimSession) -> Markup {
    let pref_name = session
        .user
        .and_then(|x| x.pref_name)
        .unwrap_or_default();

    html! {
        div class="mb-4 flex items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4" {
            label class="block text-sm font-bold mb-2 text-gray-300" {"Preferred Name: "}
            p class="text-gray-200" {(pref_name)}
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit?"}
        }
    }
}

pub async fn internal_get_profile_surname_display(session: DenimSession) -> Markup {
    let surname = session.user.map(|x| x.surname).unwrap_or_default();

    html! {
        div class="mb-4 flex items-center justify-between space-x-4 container mx-auto bg-gray-800 rounded-md p-4" {
            label class="block text-sm font-bold mb-2 text-gray-300" {"Surname: "}
            p class="text-gray-200" {(surname)}
            button class="bg-gray-700 hover:bg-gray-600 text-gray-300 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {"Edit?"}
        }
    }
}
