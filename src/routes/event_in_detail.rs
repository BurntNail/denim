use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    config::date_locale::DateFormat,
    data::{
        DataType, FilterQuery, IdForm,
        event::{Event, EventSignUpState},
        user::User,
        photo::Photo,
    },
    error::{DenimResult, MakeQuerySnafu, MissingEventSnafu},
    maud_conveniences::supertitle,
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Path, Query, State},
};
use futures::TryStreamExt;
use maud::{Markup, html};
use snafu::{ensure, OptionExt, ResultExt};
use sqlx::PgConnection;
use std::collections::HashSet;
use axum::extract::Multipart;
use infer::MatcherType;
use uuid::Uuid;
use crate::data::photo::NewPhotoForm;
use crate::error::{DenimError, InvalidImageSnafu, MultipartSnafu};
use crate::maud_conveniences::{form_submit_button, subtitle};

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
        Some(
            internal_get_signed_up_with_list(
                &mut *state.get_connection().await?,
                &event.signed_up,
                &event.verified,
                session.can(PermissionsTarget::VERIFY_ATTENDANCE),
                id,
            )
            .await?,
        )
    } else {
        None
    };
    let sign_others_up = if session.can(PermissionsTarget::SIGN_OTHERS_UP) {
        Some(
            internal_get_sign_others_up(
                State(state.clone()),
                session.clone(),
                Path(id),
                Query(FilterQuery { filter: None }),
            )
            .await?,
        )
    } else {
        None
    };

    let sign_up_button = if session.can(PermissionsTarget::SIGN_SELF_UP) {
        Some(internal_get_signup_button(State(state.clone()), session.clone(), Path(event.id)).await?)
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

    
    let photos = if session.can(PermissionsTarget::VIEW_PHOTOS) || session.can(PermissionsTarget::UPLOAD_PHOTOS) {
        Some(internal_get_photos(State(state.clone()), session.clone(), Path(id)).await?)
    } else {
        None
    };

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
                    @if let Some(photos) = photos {
                        (photos)
                    }
                    @if let Some(sign_up_button) = sign_up_button {
                        (sign_up_button)
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

                @if let Some(sign_others_up) = sign_others_up {
                    (sign_others_up)
                }

                @if let Some(signed_up_and_verified) = signed_up_and_verified {
                    (signed_up_and_verified)
                }
            }
        }
    }))
}

pub async fn internal_get_photos(State(state): State<DenimState>, session: DenimSession, Path(event_id): Path<Uuid>) -> DenimResult<Markup> {
    let (can_view_photos, can_upload_photos) = (session.can(PermissionsTarget::VIEW_PHOTOS), session.can(PermissionsTarget::UPLOAD_PHOTOS));

    if !(can_view_photos || can_upload_photos) {
        return Err(DenimError::IncorrectPermissions {
            needed: PermissionsTarget::VIEW_PHOTOS,
            found: session.get_permissions(),
        });
    }

    let links = if can_view_photos {
        let mut links = vec![];
        let bucket = state.config().s3_bucket().get()?;
        for photo in Photo::get_by_event_id(event_id, &mut *state.get_connection().await?).await? {
            links.push(photo.get_s3_url(&bucket).await?);
        }

        Some(html!{
            div class="flex flex-col space-y-2" {
                p class="text-gray-300 text-sm" {"Photos:"}
                ul class="list-disc pl-5 overflow-y-clip overflow-y-scroll max-h-64 p-2 m-4" {
                    @if links.is_empty() {
                        p class="text-gray-100 italic text-sm" {"(no photos uploaded yet)"}
                        br;
                    } @else {
                        @for (index, link) in links.into_iter().enumerate() {
                            li {
                                a href={(link)} target="_blank" class="text-gray-100 hover:text-blue-300 underline" {
                                    "Photo " (index + 1)
                                }
                            }
                        }
                    }
                }
            }
        })
    } else {
        None
    };

    Ok(html!{
        div id="photos" hx-trigger={"sse:change_photos_" (event_id)} hx-swap="outerHTML" {
            @if let Some(links) = links {
                (links)
            }
            @if can_upload_photos {
                p class="text-gray-300 text-sm" {"Upload more Photos:"}
                div class="flex flex-col space-y-2 p-2" {
                    form hx-post={"/internal/event/" (event_id) "/photos"} hx-swap="outerHTML" hx-target="#photos" hx-encoding="multipart/form-data" {
                        label for="photos" class="block text-sm font-medium text-gray-400 mb-2" {"Photos to Upload"}
                        input multiple type="file" name="photos" id="photos" accept="image/*" class="block w-full text-sm text-gray-300 file:mr-4 file:py-2 file:px-4 file:rounded file:border-0 file:text-sm file:font-semibold file:bg-violet-50 file:text-violet-700 hover:file:bg-violet-100 mb-4";
                        (form_submit_button(Some("Upload Photos")))
                    }
                }
            }
        }
    })
}

pub async fn internal_post_photos(State(state): State<DenimState>, session: DenimSession, Path(event_id): Path<Uuid>, mut multipart: Multipart) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::VIEW_PHOTOS)?;
    let bucket = state.config().s3_bucket().get()?;
    
    loop {
        let Some(field) = multipart.next_field().await.context(MultipartSnafu)? else {
            break;
        };
        
        let bytes = field.bytes().await.context(MultipartSnafu)?;
        let inferred_type = infer::get(&bytes).context(InvalidImageSnafu {found_mime: None})?;
        
        let content_type = inferred_type.mime_type();
        ensure!(inferred_type.matcher_type() == MatcherType::Image, InvalidImageSnafu {found_mime: Some(content_type)});
        
        let transaction = state.get_transaction().await?;
        
        Photo::insert_into_database_transaction(
            NewPhotoForm {
                bytes: bytes.to_vec(),
                content_type,
                extension: inferred_type.extension(),
                s3_bucket_to_add_to: bucket.clone(),
                event_id,
            },
            transaction
        ).await?;
    }
    
    
    internal_get_photos(State(state), session, Path(event_id)).await
}

pub async fn internal_get_sign_others_up(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(event_id): Path<Uuid>,
    Query(FilterQuery { filter }): Query<FilterQuery>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::SIGN_OTHERS_UP)?;
    let filter = filter.map(|filter| filter.to_lowercase());

    let students = if let Some(filter) = &filter {
        let mut all_students = User::get_all_students_with_filter(&state, filter).await?;

        let so_far_here = sqlx::query!(
            "SELECT student_id FROM participation WHERE event_id = $1",
            event_id
        )
        .fetch_all(&mut *state.get_connection().await?)
        .await
        .context(MakeQuerySnafu)?
        .into_iter()
        .map(|rec| rec.student_id)
        .collect::<HashSet<_>>();

        all_students.retain(|user| !so_far_here.contains(&user.id));

        all_students
    } else {
        vec![]
    };

    Ok(html! {
        div id="sign_others_up" hx-get={"/internal/event/" (event_id) "/sign_others_up"} hx-trigger="sse:crud_person" hx-swap="outerHTML" class="container mx-auto flex flex-col space-y-8 background-gray-800 rounded-lg shadow p-4 m-4" {
            (subtitle("Student Participation"))
            div class="flex rounded p-4 m-4" {
                input value=[filter] type="search" name="filter" placeholder="Search here to sign up students..." hx-get={"/internal/event/" (event_id) "/sign_others_up"} hx-trigger="input changed delay:500ms, keyup[key=='Enter']" hx-target="#sign_others_up" hx-swap="outerHTML" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600";
            }
            ul class="space-y-2" {
                @for student in students {
                    li class="bg-gray-700 p-3 rounded" {
                        (student)
                        " - "
                        a class="text-green-300 hover:text-green-800 cursor-pointer underline" hx-post={"/internal/event/" (event_id) "/sign_others_up"} hx-swap="outerHTML" hx-target="#sign_others_up" hx-vals={"{\"id\": \"" (student.id) "\"}" } hx-swap="none" {"Sign Up"}
                    }
                }
            }
        }
    })
}

pub async fn internal_post_sign_others_up(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(event_id): Path<Uuid>,
    Form(IdForm { id: user_id }): Form<IdForm>,
) -> DenimResult<Markup> {
    if session.user.as_ref().is_some_and(|user| user.id == user_id) {
        session.ensure_can(PermissionsTarget::SIGN_SELF_UP)?;
    } else {
        session.ensure_can(PermissionsTarget::SIGN_OTHERS_UP)?;
    }

    sqlx::query!("INSERT INTO public.participation (event_id, student_id, is_verified) VALUES ($1, $2, false)", event_id, user_id)
        .execute(&mut *state.get_connection().await?)
        .await
        .context(MakeQuerySnafu)?;

    state.send_sse_event(SseEvent::ChangeSignUp { event_id });

    internal_get_sign_others_up(
        State(state),
        session,
        Path(event_id),
        Query(FilterQuery { filter: None }),
    )
    .await
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
                sqlx::query!("INSERT INTO public.participation (event_id, student_id, is_verified) VALUES ($1, $2, FALSE)", event_id, user.id)
                    .execute(&mut *conn)
                    .await
                    .context(MakeQuerySnafu)?;
                state.send_sse_event(SseEvent::ChangeSignUp { event_id });
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
                state.send_sse_event(SseEvent::ChangeSignUp { event_id });
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
            state.send_sse_event(SseEvent::ChangeSignUp { event_id });
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

async fn internal_get_signed_up_with_list(
    conn: &mut PgConnection,
    signed_up: &[Uuid],
    verified: &[Uuid],
    can_verify: bool,
    id: Uuid,
) -> DenimResult<Markup> {
    let signed_up_students =
        User::get_from_iter_of_ids(signed_up.iter().copied(), &mut *conn).await?;
    let verified_students =
        User::get_from_iter_of_ids(verified.iter().copied(), &mut *conn).await?;

    Ok(html! {
        div id="signed_up_and_verified" class="grid grid-cols-1 md:grid-cols-2 gap-6" hx-get={"/internal/event/" (id) "/signed_up_and_verified"} hx-trigger={"sse:change_sign_up_" (id)} hx-swap="outerHTML" {
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

pub async fn internal_get_signed_up(
    State(state): State<DenimState>,
    session: DenimSession,
    Path(id): Path<Uuid>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::VIEW_SENSITIVE_DETAILS)?;

    let mut conn = state.get_connection().await?;

    let mut signed_up = vec![];
    let mut verified = vec![];

    let mut participation_stream = sqlx::query!(
        "SELECT student_id, is_verified FROM participation WHERE event_id = $1",
        id
    )
    .fetch(&mut *conn);
    while let Some(record) = participation_stream
        .try_next()
        .await
        .context(MakeQuerySnafu)?
    {
        if record.is_verified {
            verified.push(record.student_id);
        } else {
            signed_up.push(record.student_id);
        }
    }
    drop(participation_stream);

    internal_get_signed_up_with_list(
        &mut conn,
        &signed_up,
        &verified,
        session.can(PermissionsTarget::VERIFY_ATTENDANCE),
        id,
    )
    .await
}
