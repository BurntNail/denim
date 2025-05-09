use crate::{
    auth::DenimSession,
    data::{
        DataType, IdForm,
        user::{FormGroup, FullUserNameDisplay, HouseGroup, User, UserKind, UsernameDisplay},
    },
    error::{DenimError, DenimResult},
    maud_conveniences::title,
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Query, State},
};
use maud::{Markup, html};

#[axum::debug_handler]
pub async fn get_people(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    Ok(state.render(session, html!{
        div class="mx-auto bg-gray-800 p-8 rounded shadow-md max-w-4xl w-full flex flex-col space-y-4" {
            div hx-ext="sse" sse-connect="/sse_feed" class="container flex flex-row justify-center space-x-4" {
                div id="all_people" hx-get="/internal/get_people" hx-trigger="load" {}
                div id="in_focus" hx-get="/internal/get_people_form" hx-trigger="load" {}
            }
            button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/get_people_form" hx-target="#in_focus" {
                "Add new Person"
            }
        }
    }))
}

pub fn internal_get_add_people_form() -> Markup {
    html! {
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

pub async fn put_new_person(
    State(state): State<DenimState>,
    Form(add_person_form): Form<<User as DataType>::FormForAdding>,
) -> DenimResult<Markup> {
    let id =
        User::insert_into_database(add_person_form, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudPerson);

    internal_get_person_in_detail(State(state.clone()), Query(IdForm { id })).await
}

pub async fn delete_person(
    State(state): State<DenimState>,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    User::remove_from_database(id, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudPerson);

    Ok(internal_get_add_people_form())
}

pub async fn internal_get_people(State(state): State<DenimState>) -> DenimResult<Markup> {
    let staff = User::get_all_staff(&state).await?;
    let developers = User::get_all_developers(&state).await?;
    let students = User::get_all_students(&state).await?;

    Ok(html! {
        div hx-get="/internal/get_people" hx-trigger="sse:crud_person" class="container mx-auto flex flex-col space-y-8" {
            div {
                h1 class="text-2xl font-semibold mb-4" {"Staff"}
                div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                    @for person in staff {
                        a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                            (person)
                        }
                    }
                }
            }
            div {
                h1 class="text-2xl font-semibold mb-4" {"Developers"}
                div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                    @for person in developers {
                        a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                            (person)
                        }
                    }
                }
            }
            div {
                h1 class="text-2xl font-semibold mb-4" {"Students"}
                div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                    @for person in students {
                        a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                            (person)
                        }
                    }
                }
            }
        }
    })
}

pub async fn internal_get_person_in_detail(
    State(state): State<DenimState>,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    let Some(person) = User::get_from_db_by_id(id, &mut *state.get_connection().await?).await?
    else {
        return Err(DenimError::MissingUser { id });
    };

    Ok(html! {
        div hx-get="/internal/get_person" hx-trigger="sse:crud_person" hx-vals={"{\"id\": \"" (id) "\"}" } class="container mx-auto" {
            div class="rounded-lg shadow-md overflow-hidden bg-gray-800 max-w-md mx-auto" {
                div class="p-4" {
                    h1 class="text-2xl font-semibold mb-2" {(person)}
                    (FullUserNameDisplay(&person, UsernameDisplay::empty()))
                    @match person.kind {
                        UserKind::Student {
                            form: FormGroup {id: _, name: form_name},
                            house: HouseGroup {id: _, name: house_name},
                            events_participated
                        } => {
                            p class="text-gray-200 font-semibold" {
                                "House: "
                                span class="font-medium" {(house_name)}
                            }
                            p class="text-gray-200 font-semibold" {
                                "Form: "
                                span class="font-medium" {(form_name)}
                            }
                            p class="text-gray-200 font-semibold" {
                                "House Events: "
                                span class="font-medium" {(events_participated.len())}
                            }
                        },
                        _ => {}
                    }
                    p {
                        a href={"mailto:" (person.email)} class="text-blue-500" {(person.email)}
                    }
                    br;
                    button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-delete="/people" hx-vals={"{\"id\": \"" (id) "\"}" } hx-target="#in_focus" {
                        "Delete person"
                    }
                }
            }
        }
    })
}
