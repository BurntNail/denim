use crate::data::student::{FormStudent, Student};
use axum::extract::State;
use axum::Form;
use axum::response::IntoResponse;
use maud::{html, Markup};
use snafu::ResultExt;
use crate::data::IdForm;
use crate::error::{DenimResult, MakeQuerySnafu};
use crate::state::DenimState;

pub async fn get_just_students_part(State(state): State<DenimState>) -> DenimResult<Markup> {
    let students: Vec<(i32, String)> = sqlx::query_as!(Student, "SELECT * FROM students").fetch_all(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?
        .into_iter()
        .map(|student: Student| {
            println!("{student:?}");
            let first_name = student.preferred_name.unwrap_or(student.first_name);
            (student.id, format!("{first_name} {}", student.last_name))
        }).collect();

    Ok(html!{
        ul {
            @for (id, name) in &students {
                li id={"student_" (id)} {
                    p { (name) }
                    a hx-post="/delete_student" hx-vals={ "{\"id\": " (id) "}" } hx-confirm="Are you sure you wish to delete this student?" hx-target={"#student_" (id)} hx-swap="delete" {"Delete Student"}
                }
            }
        }
    })
}

pub async fn put_add_student(State(state): State<DenimState>, Form(mut new_student): Form<FormStudent>) -> DenimResult<impl IntoResponse> {
    if new_student.preferred_name.as_ref().map_or(false, |pn| pn.is_empty()) {
        new_student.preferred_name = None;
    }
    sqlx::query!("INSERT INTO students (first_name, preferred_name, last_name) VALUES ($1, $2, $3)", new_student.first_name, new_student.preferred_name, new_student.last_name).execute(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;

    get_just_students_part(State(state)).await
}

pub async fn post_delete_student (State(state): State<DenimState>, Form(IdForm {id}): Form<IdForm>) -> DenimResult<()> {
    sqlx::query!("DELETE FROM students WHERE id = $1", id).execute(&mut *state.get_connection().await?).await.context(MakeQuerySnafu)?;
    Ok(())
}

pub async fn get_index_route(State(state): State<DenimState>) -> DenimResult<impl IntoResponse> {
    let students_part = get_just_students_part(State(state.clone())).await?;

    Ok(state.render(html!{
        h1 {"Denim Front Page"}
        h2 {"Current Students: "}
        div id="students_part" {
            (students_part)
        }
        h2 {"Add New Student: "}
        form hx-put="/add_student" hx-target="#students_part" {
            input name="first_name" placeholder="First Name" required {}
            br;
            input name="preferred_name" placeholder="Preferred Name" {}
            br;
            input name="last_name" placeholder="Last Name" required {}
            br;
            button type="submit" { "Add new Student" }
        }
    }))
}
