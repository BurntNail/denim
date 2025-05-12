use axum::extract::{Path, State};
use maud::{html, Markup};
use snafu::OptionExt;
use uuid::Uuid;
use crate::auth::DenimSession;
use crate::data::DataType;
use crate::data::event::Event;
use crate::error::{DenimResult, MissingEventSnafu};
use crate::maud_conveniences::title;
use crate::state::DenimState;

pub async fn get_event (State(state): State<DenimState>, session: DenimSession, Path(id): Path<Uuid>) -> DenimResult<Markup> {
    let event = Event::get_from_db_by_id(id, &mut *state.get_connection().await?).await?.context(MissingEventSnafu {id})?;
    
    info!(?event, "Retrieved");
    
    Ok(state.render(session, html!{(title(event.name))}))
}