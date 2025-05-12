use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    data::{DataType, IdForm, event::Event, user::User},
    error::{DenimError, DenimResult},
    maud_conveniences::{
        escape, form_element, form_submit_button, simple_form_element, table, title,
    },
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Query, State},
};
use maud::{Markup, html};

#[axum::debug_handler]
pub async fn get_events(State(state): State<DenimState>, session: DenimSession) -> Markup {
    let can_add_events = session.can(PermissionsTarget::CRUD_EVENTS);

    state.render(session, html!{
        div class="mx-auto bg-gray-800 p-8 rounded shadow-md max-w-4xl w-full flex flex-col space-y-4" {
            div hx-ext="sse" sse-connect="/sse_feed" class="container flex flex-row justify-center space-x-4" {
                div hx-get="/internal/get_events" hx-trigger="sse:crud_event,load" id="all_events" {}
                @if can_add_events {
                    div id="in_focus" hx-get="/internal/events/get_events_form" hx-trigger="load" {}
                } @else {
                   div id="in_focus" {} 
                }
            }
            @if can_add_events {
                button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/events/get_events_form" hx-target="#in_focus" {
                    "Add new Event"
                }
            }
        }
    })
}

pub async fn internal_get_add_events_form(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_EVENTS)?;

    let staff = User::get_all_staff(&state).await?;

    Ok(html! {
        (title("Add New Event Form"))
        form hx-put="/events" hx-trigger="submit" hx-target="#in_focus" class="p-4" {
            (simple_form_element("name", "Name", true, None, None))
            (simple_form_element("date", "Date/Time", true, Some("datetime-local"), None))
            (simple_form_element("location", "Location (optional)", false, None, None))
            (form_element("extra_info", "Extra Information (optional)", html!{
                textarea id="extra_info" name="extra_info" rows="2" class="w-full bg-gray-700 text-gray-100 rounded px-4 py-2 border border-gray-600 focus:outline-none focus:ring focus:ring-blue-500 placeholder-gray-400 resize-y" {}
            }))
            (form_element("associated_staff_member", "Associated Staff Member", html!{
                select id="associated_staff_member" name="associated_staff_member" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                    option value="" {"Select a Staff Member (optional)"}
                    @for staff_member in staff {
                        option value={(staff_member.id)} {(staff_member)}
                    }
                }
            }))

            (form_submit_button(Some("Add Event")))
        }
    })
}

pub async fn put_new_event(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(add_event_form): Form<<Event as DataType>::FormForAdding>,
) -> DenimResult<Markup> {
    let id =
        Event::insert_into_database(add_event_form, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudEvent);

    let this_event =
        internal_get_event_in_detail(State(state.clone()), session, Query(IdForm { id })).await?;

    Ok(html! {
        (this_event)
    })
}

pub async fn delete_event(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_EVENTS)?;

    Event::remove_from_database(id, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudEvent);

    let form = internal_get_add_events_form(State(state.clone()), session).await?;
    Ok(html! {
        (form)
    })
}

pub async fn internal_get_event_in_detail(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    let Some(event) = Event::get_from_db_by_id(id, &mut *state.get_connection().await?).await?
    else {
        return Err(DenimError::MissingEvent { id });
    };

    let can_view_sensitives = session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS);
    let can_delete = session.can(PermissionsTarget::CRUD_EVENTS);

    Ok(html! {
        div hx-get="/internal/get_event" hx-target="#in_focus" hx-vals={"{\"id\": \"" (id) "\"}" } hx-trigger="sse:crud_event" {
            (title(event.name))
            div class="p-6 mb-4" {
                (title("Event Information"))
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
                @if can_view_sensitives {
                    @if let Some(staff) = event.associated_staff_member {
                        p class="text-gray-200 font-semibold" {
                            "Staff Member: "
                            span class="font-medium" {(staff)}
                        }
                    }
                }
                @if let Some(extra) = event.extra_info {
                    p class="text-gray-200 font-semibold" {
                        "Extra Information: "
                        span class="font-medium" {(extra)}
                    }
                }
                @if can_delete {
                    br;
                    button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-delete="/events" hx-vals={"{\"id\": \"" (id) "\"}" } hx-target="#in_focus" {
                        "Delete event"
                    }
                }
            }
        }
    })
}

pub async fn internal_get_events(State(state): State<DenimState>) -> DenimResult<Markup> {
    let events = Event::get_all(&state).await?;

    Ok(table(
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
