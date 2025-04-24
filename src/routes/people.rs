use axum::extract::{Query, State};
use maud::{html, Markup, Render};
use snafu::ResultExt;
use crate::error::{DenimResult, MakeQuerySnafu};
use crate::state::DenimState;
use crate::data::{IdForm, User};

pub async fn internal_get_people (State(state): State<DenimState>) -> DenimResult<Markup> {
    let people = sqlx::query_as!(User, "SELECT * FROM users").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;
    
    Ok(html!{
        div class="container mx-auto" {
            h1 class="text-2xl font-semibold mb-4" {"People"}
            div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4" {
                @for person in people {
                    a hx-get="/internal/get_person" hx-target="#person_in_detail" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-800 hover:bg-gray-700" {
                        (person)
                    }
                }
            }
        }
    })
}

pub async fn internal_get_person (State(state): State<DenimState>, Query(IdForm {id}): Query<IdForm>) -> DenimResult<Markup> {
    let person: User = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id).fetch_one(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

    Ok(html!{
        div class="container mx-auto" {
            div class="rounded-lg shadow-md overflow-hidden bg-gray-800 max-w-md mx-auto" {
                div class="p-4" {
                    h1 class="text-2xl font-semibold mb-2" {(person)}
                    p class="text-sm italic" {
                        (person.first_name)
                        " "
                        @if let Some(pref_name) = person.pref_name {
                            "\""
                            (pref_name)
                            "\" "
                        }
                        (person.surname)
                    }
                    p {
                        a href={"mailto:" (person.email)} class="text-blue-500" {(person.email)}
                    }
                }
            }
        }
    })
}