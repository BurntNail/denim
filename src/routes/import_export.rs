use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    data::{
        DataType,
        student_groups::{HouseGroup, NewHouse, NewTutorGroup, TutorGroup},
        user::{AddPersonForm, AddUserKind, User},
    },
    error::{
        DenimResult, GeneratePasswordSnafu, MakeQuerySnafu, MultipartSnafu, S3Snafu, ZipSnafu,
    },
    maud_conveniences::{Email, errors_list, form_element, form_submit_button, table},
    state::DenimState,
};
use axum::extract::{Multipart, Query, State};
use email_address::EmailAddress;
use maud::{Markup, Render, html};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    io::{Cursor, Write},
    time::Duration,
};
use uuid::Uuid;
use zip::{AesMode, ZipWriter, write::SimpleFileOptions};
use crate::maud_conveniences::{subsubtitle, title};
use crate::routes::sse::SseEvent;

#[derive(Deserialize)]
pub struct NewCSVStudent {
    first_name: String,
    pref_name: String,
    surname: String,
    email: EmailAddress,
    house: String,
    tutor_email: EmailAddress,
}

pub async fn get_import_export_page(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::EXPORT_CSVS)?;
    let can_import = session.can(PermissionsTarget::IMPORT_CSVS);

    let staff = User::get_all_staff(&state).await?;

    Ok(state.render(session, html!{
        div class="mx-auto flex flex-row justify-center p-2 m-2 rounded gap-x-8" {
            div class="rounded shadow-xl flex flex-col p-4 m-2 bg-gray-800" {
                (title(html!{p class="text-pink-400" {"Events"}}))

                /* div class="mb-8" {
                    h3 class="text-xl font-semibold mb-4" {"Export Events"}
                    button class="bg-pink-600 hover:bg-pink-700 font-bold py-2 px-4 rounded" {
                        "Download as CSV"
                    }
                } */

                @if can_import {
                    div class="overflow-scroll overflow-clip" {
                        h3 class="text-xl font-semibold mb-4" {"Import Events"}

                        div id="import_events_form" {
                            (table(
                                subsubtitle("CSV Format"),
                                ["Column", "Example", "Required"],
                                vec![
                                    ["name", "House Football", "✅"],
                                    ["datetime", "14/5/2025 10:20", "✅"],
                                    ["location", "Common", "❌"],
                                    ["extra_info", "Bring Cleats!", "❌"]
                                ]
                            ))

                            br;

                            form hx-put="/import_export/import_events" hx-swap="innerHTML" hx-target="import_events_form" {
                                label for="events_csv" class="block text-sm font-medium text-gray-400 mb-2" {"Upload Events CSV"}
                                input multiple type="file" name="events_csv" id="events_csv" accept=".csv" class="block w-full text-sm text-gray-300 file:mr-4 file:py-2 file:px-4 file:rounded file:border-0 file:text-sm file:font-semibold file:bg-violet-50 file:text-violet-700 hover:file:bg-violet-100 mb-4";

                                (form_element("associated_staff_member", "Associated Staff Member", html!{
                                    select id="associated_staff_member" name="associated_staff_member" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                                        option value="" {"Select a Staff Member (optional)"}
                                        @for staff_member in staff {
                                            option value={(staff_member.id)} {(staff_member)}
                                        }
                                    }
                                }))

                                (form_submit_button(Some("Import Events")))
                            }
                        }
                    }
                }
            }

            div class="rounded shadow-xl flex flex-col p-4 m-2 bg-gray-800" {
                (title(html!{p class="text-pink-400" {"People"}}))

                /* div class="mb-8" {
                    h3 class="text-xl font-semibold mb-4" {"Export People"}
                    button class="bg-pink-600 hover:bg-pink-700 font-bold py-2 px-4 rounded" {
                        "Download as CSV"
                    }
                } */

                @if can_import {
                    div class="overflow-scroll overflow-clip" {
                        h3 class="text-xl font-semibold mb-4" {"Import People"}

                        div id="import_people_forms" {
                            (table(
                                subsubtitle("CSV Format"),
                                ["Column", "Example", "Required"],
                                vec![
                                    ["first_name", "Jackson", "✅"],
                                    ["pref_name", "Jack", "❌"],
                                    ["surname", "Programmerson", "✅"],
                                    ["email", "jack@example.org", "✅"],
                                    ["house", "Lion", "✅"],
                                    ["tutor_email", "tutor@example.org", "✅"]
                                ]
                            ))
                            
                            p class="italic" {"NB: Missing houses, tutors and tutor groups are auto-magically added."}
                            br;

                            form hx-put="/import_export/import_people" hx-swap="innerHTML" hx-target="#import_people_forms" hx-encoding="multipart/form-data" {
                                label for="people_csv" class="block text-sm font-medium text-gray-400 mb-2" {"Upload People CSV"}
                                input multiple type="file" name="people_csv" id="people_csv" accept=".csv" class="block w-full text-sm text-gray-300 file:mr-4 file:py-2 file:px-4 file:rounded file:border-0 file:text-sm file:font-semibold file:bg-violet-50 file:text-violet-700 hover:file:bg-violet-100 mb-4";

                                (form_submit_button(Some("Import People")))
                            }
                        }
                    }
                }
            }
        }
    }))
}

#[allow(clippy::too_many_lines)]
pub async fn put_add_new_draft_students(
    State(state): State<DenimState>,
    session: DenimSession,
    mut multipart: Multipart,
) -> DenimResult<Markup> {
    struct DraftIndividualStudent {
        first_name: String,
        pref_name: String,
        surname: String,
        email: EmailAddress,
        house: i32,
        tutor_group: Uuid,
    }

    session.ensure_can(PermissionsTarget::IMPORT_CSVS)?;

    let Some(job_submitter_token) = state.get_submit_students_job_token() else {
        return get_students_import_checker(
            State(state),
            session,
            Query(ImportCheckerQuery {
                n: None,
                dots: String::new(),
            }),
        )
        .await;
    };

    let mut teachers_to_add = HashSet::new();
    let mut students_to_add = Vec::new();

    let mut houses_lookup: HashMap<_, _> = HouseGroup::get_all(&state)
        .await?
        .into_iter()
        .map(|house| (house.name, house.id))
        .collect();
    let mut tutor_group_lookup: HashMap<_, _> = TutorGroup::get_all(&state)
        .await?
        .into_iter()
        .map(|tutor_group| (tutor_group.staff_member.email, tutor_group.id))
        .collect();

    let existing_teachers: HashMap<_, _> = User::get_all_staff(&state)
        .await?
        .into_iter()
        .map(|teacher| (teacher.email, teacher.id))
        .collect();

    let mut pg_connection = state.get_transaction().await?;
    loop {
        let Some(field) = multipart.next_field().await.context(MultipartSnafu)? else {
            break;
        };

        let bytes = field.bytes().await.context(MultipartSnafu)?;
        let mut rdr = csv::Reader::from_reader(bytes.as_ref());

        for record in rdr.deserialize::<NewCSVStudent>() {
            let NewCSVStudent {
                first_name,
                pref_name,
                surname,
                email,
                house,
                tutor_email,
            } = match record {
                Ok(rec) => rec,
                Err(e) => {
                    error!(?e, "Error parsing CSV record");
                    continue;
                }
            };

            let house = if let Some(id) = houses_lookup.get(&house) {
                *id
            } else {
                let new_index = HouseGroup::insert_into_database(
                    NewHouse {
                        name: house.clone(),
                    },
                    &mut pg_connection,
                )
                .await?;

                houses_lookup.insert(house, new_index);
                new_index
            };

            let tutor_group = if let Some(id) = tutor_group_lookup.get(&tutor_email) {
                *id
            } else if let Some(teacher_id) = existing_teachers.get(&tutor_email) {
                let new_index = TutorGroup::insert_into_database(
                    NewTutorGroup {
                        staff_id: *teacher_id,
                        house_id: house,
                    },
                    &mut pg_connection,
                )
                .await?;

                tutor_group_lookup.insert(tutor_email, new_index);
                new_index
            } else {
                teachers_to_add.insert(tutor_email);
                continue;
            };

            students_to_add.push(DraftIndividualStudent {
                first_name,
                pref_name,
                surname,
                email,
                house,
                tutor_group,
            });
        }
    }

    if !teachers_to_add.is_empty() {
        return Ok(errors_list(
            Some("The following teachers need to be added:"),
            teachers_to_add
                .into_iter()
                .map(|email| Email(&email).render()),
        ));
    }

    pg_connection.commit().await.context(MakeQuerySnafu)?; //commit the new houses

    let (passwords, csv_password) = {
        #[allow(clippy::significant_drop_tightening)]
        let auth_config = state.config().auth_config().await;

        let mut passwords = (0..=students_to_add.len())
            .map(|_| auth_config.generate().context(GeneratePasswordSnafu))
            .collect::<Result<Vec<_>, _>>()?;
        let csv_password = passwords.pop().expect("adding 1 to a min 0, must have an element");

        (passwords, csv_password)
    };

    let num_students = students_to_add.len();

    let task_state = state.clone();
    let task = tokio::task::spawn(async move {
        let mut output_csv = String::from("email,default_password");
        let mut pg_connection = task_state.get_transaction().await?;

        for (
            DraftIndividualStudent {
                first_name,
                pref_name,
                surname,
                email,
                house,
                tutor_group,
            },
            password,
        ) in students_to_add.into_iter().zip(passwords)
        {
            User::insert_into_database(
                AddPersonForm {
                    first_name,
                    pref_name,
                    surname,
                    email: email.clone(),
                    password: Some(password.clone().into()),
                    current_password_is_default: true,
                    user_kind: AddUserKind::Student { tutor_group, house },
                },
                &mut pg_connection,
            )
            .await?;
            
            write!(&mut output_csv, "\n{email},{password}")
                .expect("unable to add passwords to zip file");
        }

        let mut mock_file_contents = vec![];
        let mut zip = ZipWriter::new(Cursor::new(&mut mock_file_contents));

        zip.start_file(
            "passwords.csv",
            SimpleFileOptions::default().with_aes_encryption(AesMode::Aes256, &csv_password),
        )
        .context(ZipSnafu)?;
        zip.write_all(output_csv.as_bytes())
            .expect("unable to write passwords to mock zip file");
        zip.finish().context(ZipSnafu)?;

        let bucket = task_state.config().s3_bucket();
        bucket
            .put_object_with_content_type(
                "latest_passwords.zip",
                mock_file_contents.as_slice(),
                "text/csv",
            )
            .await
            .context(S3Snafu)?;

        let presigned_get_url = {
            let mut custom_queries = HashMap::new();
            custom_queries.insert(
                "response-content-disposition".into(),
                "attachment; filename=\"latest_passwords.zip\"".into(),
            );

            bucket
                .presign_get(
                    "latest_passwords.zip",
                    2 * 24 * 60 * 60,
                    Some(custom_queries),
                )
                .await
                .context(S3Snafu)?
        };

        pg_connection.commit().await.context(MakeQuerySnafu)?;
        task_state.send_sse_event(SseEvent::CrudPerson);
        

        Ok(html! {
            div class="flex flex-col m-4 p-4 space-y-4 rounded shadow items-center justify-center text-center" {
                p {"Student accounts created - ZIP password is \"" (csv_password) "\""}

                a href=(presigned_get_url) target="_blank" class="text-gray-300 bg-green-900 hover:bg-green-700 px-3 py-2 rounded-md text-sm font-medium" {"Get Passwords for Students"}
            }
        })
    });

    job_submitter_token.submit_job(task).await;
    get_students_import_checker(
        State(state),
        session,
        Query(ImportCheckerQuery {
            n: Some(num_students),
            dots: String::new(),
        }),
    )
    .await
}

#[derive(Deserialize)]
pub struct ImportCheckerQuery {
    n: Option<usize>,
    dots: String,
}

pub async fn get_students_import_checker(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(ImportCheckerQuery {
        n: num_students,
        dots,
    }): Query<ImportCheckerQuery>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::IMPORT_CSVS)?;

    if !state.student_job_exists() {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        return Ok(errors_list(
            Some("No Import Job Exists"),
            std::iter::empty::<String>(),
        ));
    }

    if let Some(finished_job) = state.take_import_students_job_result().await {
        return finished_job;
    }

    let dots = match dots.as_str() {
        "." => "..",
        ".." => "...",
        "..." => "",
        _ => ".",
    };
    let fmt_num_students = num_students
        .map_or_else(
            || "an unknown number of".to_string(),
            |n| n.to_string());
    
    let hx_vals = html! {
        "{"
        @if let Some(n) = num_students {
            "\"n\": " (n) ", "
        }
        "\"dots\": \"" (dots) "\""
        "}"
    };

    Ok(html! {
        div hx-get="/import_export/import_people_fetch" hx-vals=(hx_vals) hx-trigger="every 1s" hx-target="this" hx-swap="outerHTML" {
            div class="flex items-center justify-center p-4 m-4 shadow rounded" {
                p {
                    "Currently adding " (fmt_num_students) " student(s) to the database" (dots);
                }
            }
        }
    })
}
