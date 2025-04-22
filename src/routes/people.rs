use axum::extract::State;
use maud::{html, Markup, Render};
use snafu::ResultExt;
use crate::error::{DenimResult, MakeQuerySnafu};
use crate::state::DenimState;
use crate::data::User;

pub async fn internal_get_people (State(state): State<DenimState>) -> DenimResult<Markup> {
    let people = sqlx::query_as!(User, "SELECT * FROM users").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;
    
    Ok(html!{
        div class="container mx-auto" {
            h1 class="text-2xl font-semibold mb-4" {"People"}
            div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4" {
                @for person in people {
                    a href={"/people/" (person.id)} class="block rounded-lg shadow-md p-4 text-center bg-gray-800 hover:bg-gray-700" {
                        (person)
                    }
                }
            }
        }
    })
}