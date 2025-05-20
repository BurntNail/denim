use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    config::date_locale::DateFormat,
    data::{
        DataType, IdForm,
        event::{Event, EventSignUpState},
        user::User,
    },
    error::{DenimResult, MakeQuerySnafu, MissingEventSnafu},
    maud_conveniences::supertitle,
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Path, State},
};
use maud::{Markup, html};
use snafu::{OptionExt, ResultExt};
use uuid::Uuid;

#[allow(clippy::too_many_lines)]
pub async fn get_event(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(id): Path<Uuid>,
) -> DenimResult<Markup> {
    let mut conn = state.get_connection().await?;
    let event = Event::get_from_db_by_id(id, &mut conn)
        .await?
        .context(MissingEventSnafu { id })?;

    let signed_up_and_verified = if session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS) {
        Some(internal_get_signed_up(State(state.clone()), session.clone(), Path(id)).await?)
    } else {
        None
    };

    let extra_info = event.extra_info.map(|extra_info| {
        html! {
            div {
                @for line in extra_info.lines() {
                    (line)
                    br;
                }
            }
        }
    });

    let sign_up_button =
        internal_get_signup_button(State(state.clone()), session.clone(), Path(event.id)).await?;

    let dlc = state.config().date_locale_config().get()?;

    Ok(state.render(session, html!{
        div class="container mx-auto px-4 py-8" {
            div class="bg-gray-800 p-6 md:p-8 rounded-lg shadow-xl" hx-ext="sse" sse-connect="/sse_feed" {
                (supertitle(event.name))

                div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8" {
                    div {
                        p class="text-gray-300 text-sm" {"Date:"}
                        p class="text-gray-100 text-lg" {(dlc.long_ymdet(&event.datetime)?)}
                        @if let Some((event_tz, global_tz)) = event.datetime.time_zone().iana_name().zip(dlc.timezone.iana_name()) {
                            @if event_tz != global_tz {
                                p class="text-gray-100 text-md" {
                                    "Local Time (" 
                                    span class="italic" {(event_tz)}
                                    "): "(dlc.format(&event.datetime, DateFormat::ShortYMDET, false)?)
                                }
                            }
                        }
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
                    (sign_up_button)
                }

                div class="mb-8" {
                    p class="text-gray-300 text-sm mb-2" {"Extra Information:"}
                    @if let Some(extra_info) = extra_info {
                        p class="text-gray-100 leading-relaxed" {(extra_info)}
                    } @else {
                        p class="text-gray-500 italic" {"No extra information provided."}
                    }
                }

                @if let Some(signed_up_and_verified) = signed_up_and_verified {
                    (signed_up_and_verified)
                }
            }
        }
    }))
}

pub async fn internal_post_toggle_self_sign_up(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(event_id): Path<Uuid>,
) -> DenimResult<()> {
    session.ensure_can(PermissionsTarget::SIGN_SELF_UP)?;
    let user = session.user.expect("can't sign self up if not logged in");
    let mut conn = state.get_connection().await?;

    if let Some(sign_up_state) =
        Event::user_is_signed_up_to_event(event_id, user.id, &mut conn).await?
    {
        match sign_up_state {
            EventSignUpState::Nothing => {
                sqlx::query!("INSERT INTO public.participation (event_id, student_id, is_verified) VALUES ($1, $2, $3)", event_id, user.id, false)
                    .execute(&mut *conn)
                    .await
                    .context(MakeQuerySnafu)?;
                state.send_sse_event(SseEvent::ChangeSignUp(event_id));
            }
            EventSignUpState::SignedUp => {
                sqlx::query!(
                    "DELETE FROM public.participation WHERE event_id = $1 AND student_id = $2",
                    event_id,
                    user.id
                )
                .execute(&mut *conn)
                .await
                .context(MakeQuerySnafu)?;
                state.send_sse_event(SseEvent::ChangeSignUp(event_id));
            }
            EventSignUpState::Verified => {
                //can't get out that easily ;)
            }
        }
    }

    Ok(())
}

pub async fn internal_post_verify(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(event_id): Path<Uuid>,
    Form(IdForm { id: student_id }): Form<IdForm>,
) -> DenimResult<()> {
    session.ensure_can(PermissionsTarget::VERIFY_ATTENDANCE)?;
    let mut conn = state.get_connection().await?;

    match Event::user_is_signed_up_to_event(event_id, student_id, &mut conn).await? {
        Some(EventSignUpState::SignedUp) => {
            sqlx::query!("UPDATE public.participation SET is_verified = TRUE WHERE event_id = $1 AND student_id = $2", event_id, student_id)
                .execute(&mut *conn)
                .await
                .context(MakeQuerySnafu)?;
            state.send_sse_event(SseEvent::ChangeSignUp(event_id));
        }
        Some(EventSignUpState::Nothing) => {
            info!(
                ?student_id,
                ?event_id,
                "Tried to verify non-signed up student"
            );
        }
        Some(EventSignUpState::Verified) => {
            info!(
                ?student_id,
                ?event_id,
                "Tried to verify already verified student"
            );
        }
        None => {
            info!(?student_id, ?event_id, "Tried to verify non-student");
        }
    }

    Ok(())
}

pub async fn internal_get_signup_button(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(event_id): Path<Uuid>,
) -> DenimResult<Markup> {
    let sign_up_state = match session.user.as_ref() {
        Some(user) => {
            Event::user_is_signed_up_to_event(
                event_id,
                user.id,
                &mut *state.get_connection().await?,
            )
            .await?
        }
        None => None,
    };

    Ok(html! {
        @if let Some(sign_up_state) = sign_up_state {
            div hx-get={"/internal/event/" (event_id) "/signup_button"} hx-trigger={"sse:change_sign_up_" (event_id)} hx-swap="outerHTML" {
                @match sign_up_state {
                    EventSignUpState::Nothing => {
                        button class="bg-green-600 hover:bg-green-800 font-bold py-2 px-4 rounded" hx-post={"/internal/event/" (event_id) "/post_toggle_self_signup"} hx-swap="none" {
                            "Sign Up"
                        }
                    },
                    EventSignUpState::SignedUp => {
                        button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-post={"/internal/event/" (event_id) "/post_toggle_self_signup"} hx-swap="none" {
                            "Un-Sign Up"
                        }
                    },
                    EventSignUpState::Verified => {
                        p class="text-gray-800 font-bold py-2 px-4 rounded" {"Verified!"}
                    }
                }
            }
        }
    })
}

pub async fn internal_get_signed_up(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(id): Path<Uuid>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::VIEW_SENSITIVE_DETAILS)?;

    let mut conn = state.get_connection().await?;

    let mut signed_up = vec![];
    let mut verified = vec![];

    for rec in sqlx::query!(
        "SELECT student_id, is_verified FROM participation WHERE event_id = $1",
        id
    )
    .fetch_all(&mut *conn)
    .await
    .context(MakeQuerySnafu)?
    {
        if rec.is_verified {
            verified.push(rec.student_id);
        } else {
            signed_up.push(rec.student_id);
        }
    }
    let signed_up_students = User::get_from_iter_of_ids(signed_up, &mut conn).await?;
    let verified_students = User::get_from_iter_of_ids(verified, &mut conn).await?;
    let can_verify = session.can(PermissionsTarget::VERIFY_ATTENDANCE);

    Ok(html! {
        div id="signed_up_and_verified" class="grid grid-cols-1 md:grid-cols-2 gap-6" hx-get={"/internal/event/" (id) "/signed_up_and_verified"} hx-trigger={"sse:change_sign_up_" (id)} {
            div {
                h3 class="text-xl font-semibold text-white mb-4" {"Signed Up Students (currently " (signed_up_students.len()) "): " }
                ul class="space-y-2 text-gray-100" {
                    @for student in signed_up_students {
                        li class="bg-gray-700 p-3 rounded" {
                            (student)
                            @if can_verify {
                                " - "
                                a class="text-green-300 hover:text-green-800 cursor-pointer underline" hx-post={"/internal/event/" (id) "/post_verify"} hx-swap="none" hx-vals={"{\"id\": \"" (student.id) "\"}" } {"Verify Attendance"}
                            }
                        }
                    }
                }
            }
            @if !verified_students.is_empty() {
                div {
                    h3 class="text-xl font-semibold text-white mb-4" {"Verified Students (currently " (verified_students.len()) "): " }
                    ul class="space-y-2 text-gray-100" {
                        @for student in verified_students {
                            li class="bg-gray-700 p-3 rounded" {(student)}
                        }
                    }
                }
            }
        }
    })
}
