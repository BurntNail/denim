use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Write;
use axum::extract::State;
use maud::{html, Escaper, Markup, PreEscaped, Render};
use snafu::ResultExt;
use uuid::Uuid;
use crate::error::{DenimResult, MakeQuerySnafu};
use crate::state::DenimState;
use crate::data::{Event, User};
use crate::maud_conveniences::render_table;

pub async fn internal_get_events(State(state): State<DenimState>) -> DenimResult<Markup> {
    let mut connection = state.get_connection().await?;
    let events = sqlx::query_as!(Event, "SELECT * FROM events").fetch_all(&mut *connection).await.context(MakeQuerySnafu)?;

    let mut staff_member_names: HashMap<Uuid, User> = HashMap::new();
    for staff_member in events.iter().flat_map(|evt| evt.associated_staff_member) {
        if let Entry::Vacant(vac) = staff_member_names.entry(staff_member) {
            let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", staff_member).fetch_one(&mut *connection).await.context(MakeQuerySnafu)?;
            vac.insert(user);
        }
    }
    
    let escape_to_preescaped = |txt: String| {
        let mut output = String::new();
        Escaper::new(&mut output).write_str(&txt).unwrap(); //this method always succeeds - strange api!
        PreEscaped(output)
    };
    
    Ok(render_table(
        "Events",
        ["Name", "Date", "Location", "Extra Info", "Staff"],
        events.into_iter()
            .map(|evt| {
                [
                    escape_to_preescaped(evt.name),
                    escape_to_preescaped(evt.date.format("%a %d/%m/%y @ %H:%M").to_string()),
                    escape_to_preescaped(evt.location.unwrap_or_else(|| "N/A".to_string())),
                    escape_to_preescaped(evt.extra_info.unwrap_or_default()),
                    html! {
                        @if let Some(staff_member) = evt.associated_staff_member {
                            @let staff_member = staff_member_names.get(&staff_member).unwrap();
                            a href={"/people/" (staff_member.id)} {
                                (staff_member)
                            }
                        } @ else {
                            p class="italic" {"Nobody"}
                        }
                    }.render()
                ]
            })
            .collect()
    ))
}