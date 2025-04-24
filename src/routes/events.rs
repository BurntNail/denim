use axum::extract::{Query, State};
use axum::Form;
use chrono::NaiveDateTime;
use maud::{html, Markup};
use serde::Deserialize;
use snafu::ResultExt;
use uuid::Uuid;
use crate::error::{DenimResult, MakeQuerySnafu, ParseTimeSnafu, ParseUuidSnafu};
use crate::state::DenimState;
use crate::data::{Event, IdForm, User};
use crate::maud_conveniences::{escape, render_table, title};

pub async fn get_events (State(state): State<DenimState>) -> DenimResult<Markup> {
    let internal_events = internal_get_events(State(state.clone())).await?;
    let internal_form = internal_get_add_events_form(State(state.clone())).await?;

    Ok(state.render(html!{
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

pub async fn internal_get_add_events_form (State(state): State<DenimState>) -> DenimResult<Markup> {
    let staff = sqlx::query_as!(User, "SELECT u.id, u.first_name, u.pref_name, u.surname, u.email, u.bcrypt_hashed_password, u.magic_first_login_characters FROM staff s INNER JOIN users u ON s.user_id = u.id").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

    Ok(html!{
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

#[derive(Deserialize)]
pub struct AddEventForm {
    pub name: String,
    pub date: String,
    pub location: String,
    pub extra_info: String,
    pub associated_staff_member: String
}

pub async fn put_new_event (State(state): State<DenimState>, Form(AddEventForm { name, date, location, extra_info, associated_staff_member }): Form<AddEventForm>) -> DenimResult<Markup> {
    let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M").context(ParseTimeSnafu {
        original: date
    })?;

    let location = if location.is_empty() {None} else {Some(location)};
    let extra_info = if extra_info.is_empty() {None} else {Some(extra_info)};
    let associated_staff_member = if associated_staff_member.is_empty() {None} else {
        Some(Uuid::try_parse(&associated_staff_member).context(ParseUuidSnafu {
            original: associated_staff_member
        })?)
    };

    let id = sqlx::query!("INSERT INTO events (name, date, location, extra_info, associated_staff_member) VALUES ($1, $2, $3, $4, $5) RETURNING id", name, date, location, extra_info, associated_staff_member).fetch_one(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?.id;
    //gets weird when i try to use query_as, idk

    let all_events = internal_get_events(State(state.clone())).await?;
    let this_event = internal_get_event_in_detail(State(state.clone()), Query(IdForm{id})).await?;
    Ok(html!{
        (this_event)
        div hx-swap-oob="outerHTML:#all_events" id="all_events" {
            (all_events)
        }
    })
}

pub async fn delete_event (State(state): State<DenimState>, Query(IdForm{id}): Query<IdForm>) -> DenimResult<Markup> {
    sqlx::query!("DELETE FROM events WHERE id = $1", id).execute(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

    let all_events = internal_get_events(State(state.clone())).await?;
    let form = internal_get_add_events_form(State(state.clone())).await?;
    Ok(html!{
        (form)
        div hx-swap-oob="outerHTML:#all_events" id="all_events" {
            (all_events)
        }
    })
}

pub async fn internal_get_event_in_detail (State(state): State<DenimState>, Query(IdForm {id}): Query<IdForm>) -> DenimResult<Markup> {
    let mut connection = state.get_connection().await?;
    let event = sqlx::query_as!(Event, "SELECT * FROM events WHERE id = $1", id).fetch_one(&mut *connection).await.context(MakeQuerySnafu)?;
    let associated_staff_member = if let Some(assoc_staff_id) = event.associated_staff_member {
        Some(sqlx::query_as!(User, "SELECT * FROM users u WHERE id = $1", assoc_staff_id).fetch_one(&mut *connection).await.context(MakeQuerySnafu)?)
    } else {
        None
    };
    drop(connection);

    Ok(html!{
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
            @if let Some(staff) = associated_staff_member {
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
            button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-delete="/events" hx-vals={"{\"id\": \"" (event.id) "\"}" } hx-target="#in_focus" {
                "Delete event"
            }
        }
    })
}

pub async fn internal_get_events(State(state): State<DenimState>) -> DenimResult<Markup> {
    let events = sqlx::query_as!(Event, "SELECT * FROM events").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

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