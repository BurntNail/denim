use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    data::{
        DataType, IdForm,
        event::{AddEvent, Event},
        user::User,
    },
    error::{DenimError, DenimResult, ParseTimeSnafu, ParseUuidSnafu},
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
use chrono::NaiveDateTime;
use maud::{Markup, html};
use serde::Deserialize;
use snafu::ResultExt;
use uuid::Uuid;

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

#[derive(Deserialize)]
pub struct NewEventForm {
    name: String,
    date: String,
    location: String,
    extra_info: String,
    associated_staff_member: String,
}

pub async fn put_new_event(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(NewEventForm {
        name,
        date,
        location,
        extra_info,
        associated_staff_member,
    }): Form<NewEventForm>,
) -> DenimResult<Markup> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")
        .context(ParseTimeSnafu { original: date })?;

    let location = if location.is_empty() {
        None
    } else {
        Some(location)
    };
    let extra_info = if extra_info.is_empty() {
        None
    } else {
        Some(extra_info)
    };
    let associated_staff_member = if associated_staff_member.is_empty() {
        None
    } else {
        Some(
            Uuid::try_parse(&associated_staff_member).context(ParseUuidSnafu {
                original: associated_staff_member,
            })?,
        )
    };

    let id = Event::insert_into_database(
        AddEvent {
            name,
            date,
            location,
            extra_info,
            associated_staff_member,
        },
        &mut *state.get_connection().await?,
    )
    .await?;
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
    let form = internal_get_add_events_form(State(state.clone()), session).await?;

    //technically, there's a fun thing where in some cases the website will process the crud change BEFORE the new html
    //and that's insanely annoying
    //but there's kinda no nice way to fix it...
    state.send_sse_event(SseEvent::CrudEvent);

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
            (title(html!{
                a class="hover:text-blue-300 underline" target="_blank" href={"/event/" (id)} {(event.name)}
            }))
            div class="p-4" {
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

#[derive(Deserialize)]
pub struct FuturePastFilterQuery {
    pub future: Option<String>,
    pub past: Option<String>,
}

pub async fn internal_get_events(
    State(state): State<DenimState>,
    Query(FuturePastFilterQuery { future, past }): Query<FuturePastFilterQuery>,
) -> DenimResult<Markup> {
    let event_to_row = |evt: Event| {
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
    };
    
    let future_events: Vec<_> = Event::get_future_events(&state)
        .await?
        .into_iter()
        .filter(|event| future.as_ref().is_none_or(|filter| event.name.contains(filter)))
        .map(event_to_row)
        .collect();
    let past_events: Vec<_> = Event::get_past_events(&state)
        .await?
        .into_iter()
        .filter(|event| past.as_ref().is_none_or(|filter| event.name.contains(filter)))
        .map(event_to_row)
        .collect();
    

    Ok(html!{
        div class="flex flex-col" {
            (table(
                html! {
                    (title("Future Events"))
                    div class="flex rounded p-4 m-4" {
                        input value=[future] type="search" name="future" placeholder="Begin Typing To Search Events..." hx-get="/internal/get_events" hx-trigger="input changed delay:500ms, keyup[key=='Enter']" hx-target="#all_events" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
                    }
                },
                ["Name", "Date", "Location"],
                future_events,
            ))
            div class="h-4 bg-transparent" {""}
            (table(
                html! {
                    (title("Past Events"))
                    div class="flex rounded p-4 m-4" {
                        input value=[past] type="search" name="past" placeholder="Begin Typing To Search Events..." hx-get="/internal/get_events" hx-trigger="input changed delay:500ms, keyup[key=='Enter']" hx-target="#all_events" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
                    }
                },
                ["Name", "Date", "Location"],
                past_events,
            ))
        }
    })
}
