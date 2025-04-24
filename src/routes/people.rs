use axum::extract::{Query, State};
use axum::Form;
use maud::{html, Markup};
use serde::Deserialize;
use snafu::ResultExt;
use crate::error::{DenimResult, MakeQuerySnafu};
use crate::state::DenimState;
use crate::data::{IdForm, User};
use crate::maud_conveniences::title;

pub async fn get_people (State(state): State<DenimState>) -> DenimResult<Markup> {
    let internal_people = internal_get_people(State(state.clone())).await?;
    let internal_form = internal_get_add_people_form();
    
    Ok(state.render(html!{
        div class="mx-auto bg-gray-800 p-8 rounded shadow-md max-w-4xl w-full flex flex-col space-y-4" {
            div class="container flex flex-row justify-center space-x-4" {
                div id="all_people" {
                    (internal_people)
                }
                div id="in_focus" {
                    (internal_form)
                }
            }
            button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/get_people_form" hx-target="#in_focus" {
                "Add new Person"
            }
        }
    }))
}

pub fn internal_get_add_people_form () -> Markup {
    html!{
        (title("Add New Person Form"))
        form hx-put="/people" hx-trigger="submit" hx-target="#in_focus" class="p-4" {
            div class="mb-4" {
                label for="first_name" class="block text-sm font-bold mb-2 text-gray-300" {"First Name"}
                input required type="text" id="first_name" name="first_name" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="pref_name" class="block text-sm font-bold mb-2 text-gray-300" {"Preferred Name (optional)"}
                input type="text" id="pref_name" name="pref_name" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="surname" class="block text-sm font-bold mb-2 text-gray-300" {"Surname"}
                input required type="text" id="surname" name="surname" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="email" class="block text-sm font-bold mb-2 text-gray-300" {"Email Address"}
                input required type="email" id="email" name="email" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }

            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {
                    "Add Person"
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct AddPersonForm {
    pub first_name: String,
    pub pref_name: String,
    pub surname: String,
    pub email: String,
}

pub async fn put_new_person(State(state): State<DenimState>, Form(AddPersonForm { first_name, pref_name, surname, email }): Form<AddPersonForm>) -> DenimResult<Markup> {
    let pref_name = if pref_name.is_empty() {None} else {Some(pref_name)};

    let id = sqlx::query!("INSERT INTO users (first_name, pref_name, surname, email) VALUES ($1, $2, $3, $4) RETURNING id", first_name, pref_name, surname, email).fetch_one(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?.id;

    let all_people = internal_get_people(State(state.clone())).await?;
    let this_person = internal_get_person_in_detail(State(state.clone()), Query(IdForm{id})).await?;
    Ok(html!{
        (this_person)
        div hx-swap-oob="outerHTML:#all_events" id="all_events" {
            (all_people)
        }
    })
}

pub async fn delete_person (State(state): State<DenimState>, Query(IdForm{id}): Query<IdForm>) -> DenimResult<Markup> {
    sqlx::query!("DELETE FROM users WHERE id = $1", id).execute(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

    let all_people = internal_get_people(State(state.clone())).await?;
    let form = internal_get_add_people_form();
    Ok(html!{
        (form)
        div hx-swap-oob="outerHTML:#all_events" id="all_people" {
            (all_people)
        }
    })
}

pub async fn internal_get_people (State(state): State<DenimState>) -> DenimResult<Markup> {
    let people = sqlx::query_as!(User, "SELECT * FROM users").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;
    
    Ok(html!{
        div class="container mx-auto" {
            h1 class="text-2xl font-semibold mb-4" {"People"}
            div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                @for person in people {
                    a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                        (person)
                    }
                }
            }
        }
    })
}

pub async fn internal_get_person_in_detail(State(state): State<DenimState>, Query(IdForm {id}): Query<IdForm>) -> DenimResult<Markup> {
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