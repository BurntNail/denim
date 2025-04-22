use axum::extract::State;
use axum::response::IntoResponse;
use maud::html;
use crate::error::{DenimResult};
use crate::routes::events::internal_get_events;
use crate::state::DenimState;

pub async fn get_index_route(State(state): State<DenimState>) -> DenimResult<impl IntoResponse> {
    let default_info = internal_get_events(State(state.clone())).await?;
    
    Ok(state.render(html! {
        div class="bg-gray-800 p-8 rounded shadow-md max-w-md w-full" {
            h1 class="text-2xl font-semibold mb-6 text-center" {
                "Denim!"
            }
    
            div class="flex flex-row space-x-4 justify-center" {
                button class="bg-blue-600 hover:bg-blue-800 text-white font-bold py-2 px-4 rounded" hx-get="/internal/get_events" hx-target="#click_result" {
                    "View Events"
                }
                button class="bg-green-600 hover:bg-green-800 text-white font-bold py-2 px-4 rounded" hx-get="/internal/get_people" hx-target="#click_result" {
                    "View People"
                }
            }
        }
        
        div id="click_result" class="bg-gray-800 rounded shadow-md max-w-2xl w-full m-4 p-8" {
            (default_info)
        }
    }))
}