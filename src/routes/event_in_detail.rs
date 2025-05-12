use axum::extract::{Path, State};
use maud::{html, Markup};
use snafu::{OptionExt, ResultExt};
use uuid::Uuid;
use crate::auth::{AuthUtilities, DenimSession, PermissionsTarget};
use crate::data::DataType;
use crate::data::event::Event;
use crate::data::user::User;
use crate::error::{DenimResult, MakeQuerySnafu, MissingEventSnafu};
use crate::maud_conveniences::title;
use crate::state::DenimState;

pub async fn get_event (State(state): State<DenimState>, session: DenimSession, Path(id): Path<Uuid>) -> DenimResult<Markup> {
    let mut conn = state.get_connection().await?;
    let event = Event::get_from_db_by_id(id, &mut conn).await?.context(MissingEventSnafu {id})?;

    let can_view_sensitives = session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS);
    
    let mut signed_up = vec![];
    let mut verified = vec![];
    
    for rec in sqlx::query!("SELECT student_id, is_verified FROM participation WHERE event_id = $1", id)
        .fetch_all(&mut *conn)
        .await
        .context(MakeQuerySnafu)? {
        if rec.is_verified {
            verified.push(rec.student_id);
        } else {
            signed_up.push(rec.student_id);
        }
    }
    let signed_up_students = User::get_from_iter_of_ids(signed_up, &mut conn).await?;
    let verified_students = User::get_from_iter_of_ids(verified, &mut conn).await?;

    let extra_info = event.extra_info
        .map(|extra_info| 
            html!{
                div {
                    @for line in extra_info.lines() {
                        (line)
                        br;
                    }
                }
            }
        );

    Ok(state.render(session, html!{
        div class="container mx-auto px-4 py-8" {
            div class="bg-gray-800 p-6 md:p-8 rounded-lg shadow-xl" {
                (title(event.name))

                div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8" {
                    div {
                        p class="text-gray-300 text-sm" {"Date:"}
                        p class="text-gray-100 text-lg" {(event.date.format("%A %d %B, %Y at %H:%M"))}
                    }
                    div {
                        p class="text-gray-300 text-sm" {"Location:"}
                        @if let Some(location) = event.location {
                            p class="text-gray-100 text-lg" {(location)}
                        } @else {
                            p class="text-gray-500 text-lg" {"Not Specified"}
                        }
                    }
                    div {
                        p class="text-gray-300 text-sm" {"Staff Member:"}
                        @if let Some(staff) = event.associated_staff_member {
                            p class="text-gray-100 text-lg" {(staff)}
                        } @else {
                            p class="text-gray-500 text-lg" {"None Assigned"}
                        }
                    }
                }

                div class="mb-8" {
                    p class="text-gray-300 text-sm mb-2" {"Extra Information:"}
                    @if let Some(extra_info) = extra_info {
                        p class="text-gray-100 leading-relaxed" {(extra_info)}
                    } @else {
                        p class="text-gray-500 italic" {"No extra information provided."}
                    }
                }

                @if can_view_sensitives {
                    div class="grid grid-cols-1 md:grid-cols-2 gap-6" {
                        div {
                            h3 class="text-xl font-semibold text-white mb-4" {"Signed Up Students (currently " (signed_up_students.len()) ")" }
                            ul class="space-y-2 text-gray-100" {
                                @for student in signed_up_students {
                                    li class="bg-gray-700 p-3 rounded" {(student)}
                                }
                            }
                        }
                        @if !verified_students.is_empty() {
                            div {
                                h3 class="text-xl font-semibold text-white mb-4" {"Verified Students (currently " (verified_students.len()) ")" }
                                ul class="space-y-2 text-gray-100" {
                                    @for student in verified_students {
                                        li class="bg-gray-700 p-3 rounded" {(student)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}