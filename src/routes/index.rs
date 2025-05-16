use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    error::DenimResult,
    state::DenimState,
};
use axum::{
    body::Body,
    extract::State,
    http::Response,
    response::{IntoResponse, Redirect},
};
use maud::html;

pub async fn get_index_route(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Response<Body>> {
    if session.can(PermissionsTarget::RUN_ONBOARDING) && !state.config().s3_bucket_exists() {
        return Ok(Redirect::to("/onboarding").into_response());
    }

    let can_view_people = session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS);

    Ok(state.render(session, html! {
        div class="bg-gray-800 p-8 rounded shadow-md max-w-md w-full" {
            h1 class="text-2xl font-semibold mb-6 text-center" {
                "Denim!"
            }
            div class="flex flex-row space-x-4 justify-center" {
                a href="/events" class="bg-slate-600 hover:bg-slate-800 font-bold py-2 px-4 rounded"  {
                    "View Events"
                }
                @if can_view_people {
                    a href="/people" class="bg-slate-600 hover:bg-slate-800 font-bold py-2 px-4 rounded"  {
                        "View People"
                    }
                }
            }
        }
    }).into_response())
}
