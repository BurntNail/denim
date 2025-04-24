use axum::extract::State;
use axum::response::IntoResponse;
use maud::html;
use crate::error::{DenimResult};
use crate::routes::events::internal_get_events;
use crate::state::DenimState;

pub async fn get_index_route(State(state): State<DenimState>) -> DenimResult<impl IntoResponse> {
    Ok(state.render(html! {
        div class="bg-gray-800 p-8 rounded shadow-md max-w-md w-full" {
            h1 class="text-2xl font-semibold mb-6 text-center" {
                "Denim!"
            }
    
            div class="flex flex-row space-x-4 justify-center" {
                a href="/events" class="bg-slate-600 hover:bg-slate-800 font-bold py-2 px-4 rounded"  {
                    "View Events"
                }
                a href="/people" class="bg-slate-600 hover:bg-slate-800 font-bold py-2 px-4 rounded"  {
                    "View People"
                }
            }
        }
    }))
}