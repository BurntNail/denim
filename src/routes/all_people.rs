use crate::{
    auth::{DenimSession, PermissionsTarget},
    data::{
        DataType, IdForm,
        user::{
            AddPersonForm, AddUserKind, FullUserNameDisplay, User, UserKind,
            UsernameDisplay,
        },
        student_groups::{FormGroup, HouseGroup},
    },
    error::{DenimError, DenimResult, NoHousesOrNoFormsSnafu},
    maud_conveniences::{form_element, simple_form_element, title},
    routes::sse::SseEvent,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Query, State},
};
use axum::body::Body;
use axum::http::Response;
use axum::response::{IntoResponse, Redirect};
use maud::{Markup, html};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use uuid::Uuid;
use crate::auth::AuthUtilities;
use crate::maud_conveniences::Email;

#[axum::debug_handler]
pub async fn get_people(
    State(state): State<DenimState>,
    session: DenimSession,
) -> Response<Body> {
    if !session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS) {
        return Redirect::to("/login?next=people").into_response();
    }
    
    state.render(session, html!{
        div class="mx-auto bg-gray-800 p-8 rounded shadow-md max-w-4xl w-full flex flex-col space-y-4" {
            div hx-ext="sse" sse-connect="/sse_feed" class="container flex flex-row justify-center space-x-4" {
                div id="all_people" hx-get="/internal/get_people" hx-trigger="load" {}
                div id="in_focus" {}
            }
        }
    }).into_response()
}

#[derive(Deserialize)]
pub struct IsStaffQuery {
    is_staff: bool,
}

pub async fn internal_get_add_dev_or_staff_form(
    session: DenimSession,
    Query(IsStaffQuery { mut is_staff }): Query<IsStaffQuery>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_USERS)?;

    if !session.can(PermissionsTarget::CRUD_ADMINS) {
        is_staff = true;
    }

    Ok(html! {
        @if is_staff {
            (title("Add New Staff Member"))
        } @else {
            (title("Add New Admin"))
        }

        form hx-put="/internal/people/new_staff_or_dev_form" hx-trigger="submit" hx-target="#in_focus" class="p-4" {
            (simple_form_element("first_name", "First Name", true, None,  None))
            (simple_form_element("pref_name", "Preferred Name", false, None,  None))
            (simple_form_element("surname", "Surname", true, None,  None))
            (simple_form_element("email", "Email", true, Some("email"),  None))

            div class="mb-4 flex items-center" {
                input type="checkbox" name="generate_password" id="generate_password" class="mr-2 leading-tight";
                label for="generate_password" class="text-gray-300 cursor-pointer" {"Auto-Generate Password?"}
            }

            input type="hidden" value=(is_staff) name="is_staff" id="is_staff" {}

            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {
                    "Add Person"
                }
            }
        }
    })
}

pub async fn internal_get_add_student_form(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_USERS)?;

    let forms = FormGroup::get_all(&state).await?;
    let houses = HouseGroup::get_all(&state).await?;

    snafu::ensure!(!forms.is_empty(), NoHousesOrNoFormsSnafu);
    snafu::ensure!(!houses.is_empty(), NoHousesOrNoFormsSnafu);

    Ok(html! {
        (title("Add New Student"))

        form hx-put="/internal/people/new_student_form" hx-trigger="submit" hx-target="#in_focus" class="p-4" {
            (simple_form_element("first_name", "First Name", true, None,  None))
            (simple_form_element("pref_name", "Preferred Name", false, None,  None))
            (simple_form_element("surname", "Surname", true, None,  None))
            (simple_form_element("email", "Email", true, Some("email"),  None))

            div class="mb-4 flex items-center" {
                input type="checkbox" name="generate_password" id="generate_password" class="mr-2 leading-tight";
                label for="generate_password" class="text-gray-300 cursor-pointer" {"Auto-Generate Password?"}
            }

            (form_element("form", "Form Group", html!{
                select id="form" name="form" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                    @for form in forms {
                        option value={(form.id)} {(form.name)}
                    }
                }
            }))
            (form_element("house", "House", html!{
                select id="house" name="house" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                    @for house in houses {
                        option value={(house.id)} {(house.name)}
                    }
                }
            }))

            div class="flex items-center justify-between" {
                button type="submit" class="bg-blue-500 hover:bg-blue-700 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {
                    "Add Person"
                }
            }
        }
    })
}

#[derive(Deserialize)]
pub struct NewStaffOrDevForm {
    first_name: String,
    pref_name: String,
    surname: String,
    email: String,
    generate_password: String,
    is_staff: bool,
}

pub async fn internal_put_new_staff_or_dev(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(form): Form<NewStaffOrDevForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_USERS)?;
    let user_kind = if form.is_staff && session.can(PermissionsTarget::CRUD_ADMINS) {
        AddUserKind::Staff
    } else {
        AddUserKind::Dev
    };

    let password = if &form.generate_password == "on" {
        state
            .config()
            .auth_config()
            .await
            .generate()
            .map(Into::into)
    } else {
        None
    };
    
    let add_person_form = AddPersonForm {
        first_name: form.first_name,
        pref_name: form.pref_name,
        surname: form.surname,
        email: form.email,
        password: password.clone(),
        current_password_is_default: true,
        user_kind,
    };

    let id =
        User::insert_into_database(add_person_form, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudPerson);

    internal_get_person_in_detail(
        State(state.clone()),
        session,
        Query(InDetailForm {
            id,
            new_password: password,
        }),
    )
    .await
}

#[derive(Deserialize)]
pub struct NewStudentForm {
    first_name: String,
    pref_name: String,
    surname: String,
    email: String,
    generate_password: String,
    form: i32,
    house: i32,
}
pub async fn internal_put_new_student(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(form): Form<NewStudentForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_USERS)?;

    let password = if &form.generate_password == "on" {
        state
            .config()
            .auth_config()
            .await
            .generate()
            .map(Into::into)
    } else {
        None
    };

    let add_person_form = AddPersonForm {
        first_name: form.first_name,
        pref_name: form.pref_name,
        surname: form.surname,
        email: form.email,
        password: password.clone(),
        current_password_is_default: true,
        user_kind: AddUserKind::Student {
            form: form.form,
            house: form.house,
        },
    };

    let id =
        User::insert_into_database(add_person_form, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudPerson);

    internal_get_person_in_detail(
        State(state.clone()),
        session,
        Query(InDetailForm {
            id,
            new_password: password,
        }),
    )
    .await
}

pub async fn delete_person(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(IdForm { id }): Query<IdForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::CRUD_USERS)?;

    User::remove_from_database(id, &mut *state.get_connection().await?).await?;
    state.send_sse_event(SseEvent::CrudPerson);

    Ok(html! {})
}

pub async fn internal_get_people(State(state): State<DenimState>, session: DenimSession) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::VIEW_SENSITIVE_DETAILS)?;
    
    let staff = User::get_all_staff(&state).await?;
    let developers = User::get_all_developers(&state).await?;
    let students = User::get_all_students(&state).await?;
    
    let can_change_users = session.can(PermissionsTarget::CRUD_USERS);
    let can_change_admins = session.can(PermissionsTarget::CRUD_ADMINS);

    Ok(html! {
        div hx-get="/internal/get_people" hx-trigger="sse:crud_person" class="container mx-auto flex flex-col space-y-8" {
            div {
                div class="flex flex-row items-center justify-between" {
                    (title("Staff"))
                    @if can_change_users {
                        button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/people/new_staff_or_dev_form?is_staff=true" hx-target="#in_focus" {
                            "Add new Staff Member"
                        }
                    }
                }
                div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                    @for person in staff {
                        a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                            (person)
                        }
                    }
                }
            }
            div {
                div class="flex flex-row items-center justify-between" {
                    (title("Admins"))
                    @if can_change_admins {
                        button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/people/new_staff_or_dev_form?is_staff=false" hx-target="#in_focus" {
                            "Add new Admin"
                        }
                    }
                }
                div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                    @for person in developers {
                        a hx-get="/internal/get_person" hx-target="#in_focus" hx-vals={"{\"id\": \"" (person.id) "\"}" } class="block rounded-lg shadow-md p-4 text-center bg-gray-700 hover:bg-gray-600" {
                            (person)
                        }
                    }
                }
            }
            div {
                div class="flex flex-row items-center justify-between" {
                    (title("Students"))
                    @if can_change_users {
                        button class="bg-blue-600 hover:bg-blue-800 font-bold py-2 px-4 rounded" hx-get="/internal/people/new_student_form" hx-target="#in_focus" {
                            "Add new Student"
                        }
                    }
                }
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

#[derive(Deserialize)]
pub struct InDetailForm {
    pub id: Uuid,
    pub new_password: Option<SecretString>,
}

pub async fn internal_get_person_in_detail(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(InDetailForm { id, new_password }): Query<InDetailForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::VIEW_SENSITIVE_DETAILS)?;
    
    let Some(person) = User::get_from_db_by_id(id, &mut *state.get_connection().await?).await?
    else {
        return Err(DenimError::MissingUser { id });
    };
    
    let hx_vals = new_password
        .as_ref()
        .map_or_else(
            || html! { "{\"id\": \"" (id) "\"}" },
            |np| html!{ "{\"id\": \"" (id) "\", \"new_password\": \"" (np.expose_secret()) "\"}" }
        );
    
    let can_delete = session.can(
        match person.kind {
            UserKind::Developer => PermissionsTarget::CRUD_ADMINS,
            _ => PermissionsTarget::CRUD_USERS,
        }
    );

    Ok(html! {
        div hx-get="/internal/get_person" hx-trigger="sse:crud_person" hx-vals=(hx_vals) class="container mx-auto" {
            (title(person.clone()))

            div class="rounded-lg shadow-md overflow-hidden bg-gray-800 max-w-md mx-auto" {
                div class="p-4" {
                    (FullUserNameDisplay(&person, UsernameDisplay::empty()))

                    @if let Some(new_password) = new_password {
                        br;
                        div class="py-4" {
                            p class="text-gray-200 font-semibold" {
                                "Default Password (not shown again): "
                                span class="font-medium" {(new_password.expose_secret())}
                            }
                        }
                    }
                    
                    br;
                    (Email(&person.email))

                    @match person.kind {
                        UserKind::Student {
                            form: FormGroup {id: _, name: form_name},
                            house: HouseGroup {id: _, name: house_name},
                            events_participated
                        } => {
                            div class="py-4" {
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
                            }
                        },
                        _ => {}
                    }
                    
                    @if can_delete {
                        br;
                        button class="bg-red-600 hover:bg-red-800 font-bold py-2 px-4 rounded" hx-delete="/people" hx-vals={"{\"id\": \"" (id) "\"}" } hx-target="#in_focus" {
                            "Delete person"
                        }
                    }
                }
            }
        }
    })
}
