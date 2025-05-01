use crate::{
    auth::DenimSession,
    data::{DataType, IdForm, event::Event, user::User},
    error::{DenimError, DenimResult},
    maud_conveniences::{escape, render_table, title},
    state::DenimState,
};
use axum::{
    Form,
    extract::{Query, State},
};
use maud::{Markup, html};

pub async fn get_events(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    let internal_events = internal_get_events(State(state.clone())).await?;
    let internal_form = internal_get_add_events_form(State(state.clone())).await?;

    Ok(state.render(session, html!{
        div class="mx-auto bg-gray-800 p-8 rounded shadow-md max-w-4xl w-full flex flex-col space-y-4" {
            div class="container flex flex-row justify-center space-x-4" {
                div id="all_events" {
                    (internal_events)
                }
                div id="in_focus" {
                    (internal_form)
                }
            }
            button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/get_events_form" hx-target="#in_focus" {
                "Add new Event"
            }
        }
    }))
}

pub async fn internal_get_add_events_form(State(state): State<DenimState>) -> DenimResult<Markup> {
    let staff = User::get_all_staff(state.clone()).await?;

    Ok(html! {
        (title("Add New Event Form"))
        form hx-put="/events" hx-trigger="submit" hx-target="#in_focus" class="p-4" {
            div class="mb-4" {
                label for="name" class="block text-sm font-bold mb-2 text-gray-300" {"Name"}
                input required type="text" id="name" name="name" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="date" class="block text-sm font-bold mb-2 text-gray-300" {"Date/Time"}
                input required type="datetime-local" id="date" name="date" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="location" class="block text-sm font-bold mb-2 text-gray-300" {"Location (optional)"}
                input type="text" id="location" name="location" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="extra_info" class="block text-sm font-bold mb-2 text-gray-300" {"Extra Information (optional)"}
                input type="text" id="extra_info" name="extra_info" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            }
            div class="mb-4" {
                label for="associated_staff_member" class="block text-sm font-bold mb-2 text-gray-300" {"Associated Staff Member"}
                select id="associated_staff_member" name="associated_staff_member" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                    option value="" {"Select a Staff Member (optional)"}
                    @for staff_member in staff {
                        option value={(staff_member.id)} {(staff_member)}
                    }
                }
            }

            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {
                    "Add Event"
                }
            }
        }
    })
}

pub async fn put_new_event(
    State(state): State<DenimState>,
    Form(add_event_form): Form<<Event as DataType>::FormForAdding>,
) -> DenimResult<Markup> {
    let id = Event::insert_into_database(add_event_form, state.get_connection().await?).await?;

    let all_events = internal_get_events(State(state.clone())).await?;
    let this_event =
        internal_get_event_in_detail(State(state.clone()), Query(IdForm { id })).await?;
    Ok(html! {
        (this_event)
        div hx-swap-oob="outerHTML:#all_events" id="all_events" {
            (all_events)
        }
    })
}

pub async fn delete_event(
    State(state): State<DenimState>,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    Event::remove_from_database(id, state.get_connection().await?).await?;

    let all_events = internal_get_events(State(state.clone())).await?;
    let form = internal_get_add_events_form(State(state.clone())).await?;
    Ok(html! {
        (form)
        div hx-swap-oob="outerHTML:#all_events" id="all_events" {
            (all_events)
        }
    })
}

pub async fn internal_get_event_in_detail(
    State(state): State<DenimState>,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    let Some(event) = Event::get_from_db_by_id(id, state.get_connection().await?).await? else {
        return Err(DenimError::MissingEvent { id });
    };

    Ok(html! {
        (title(event.name))
        div class="p-6 mb-4" {
            h2 class="text-lg font-semibold mb-2 text-gray-300 underline" {"Event Information"}
            @if let Some(location) = event.location {
                p class="text-gray-200 font-semibold" {
                    "Location: "
                    span class="font-medium" {(location)}
                }
            }
            p class="text-gray-200 font-semibold" {
                "Time: "
                span class="font-medium" {(event.date.format("%a %d/%m/%y @ %H:%M"))}
            }
            @if let Some(staff) = event.associated_staff_member {
                p class="text-gray-200 font-semibold" {
                    "Staff Member: "
                    span class="font-medium" {(staff)}
                }
            }
            @if let Some(extra) = event.extra_info {
                p class="text-gray-200 font-semibold" {
                    "Extra Information: "
                    span class="font-medium" {(extra)}
                }
            }
            br;
            button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-delete="/events" hx-vals={"{\"id\": \"" (id) "\"}" } hx-target="#in_focus" {
                "Delete event"
            }
        }
    })
}

pub async fn internal_get_events(State(state): State<DenimState>) -> DenimResult<Markup> {
    let events = Event::get_all(state.clone()).await?;

    Ok(render_table(
        "Events",
        ["Name", "Date", "Location",],
        events.into_iter()
            .map(|evt| {
                [
                    html!{
                        a class="hover:text-blue-300 underline" hx-get="/internal/get_event" hx-target="#in_focus" hx-vals={"{\"id\": \"" (evt.id) "\"}" } {
                            (evt.name)
                        }
                    },
                    escape(evt.date.format("%a %d/%m/%y @ %H:%M").to_string()),
                    html!{
                        @if let Some(location) = evt.location {
                            p {(location)}
                        } @else {
                            p class="italic" {"-"}
                        }
                    }
                ]
            })
            .collect()
    ))
}
