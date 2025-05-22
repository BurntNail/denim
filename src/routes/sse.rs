use crate::state::DenimState;
use axum::{
    extract::State,
    response::{
        Sse,
        sse::{Event as AxumSseEvent, KeepAlive},
    },
};
use futures::Stream;
use std::convert::Infallible;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub enum SseEvent {
    CrudEvent,
    CrudPerson,
    ChangeSignUp { event_id: Uuid },
}

impl From<SseEvent> for AxumSseEvent {
    fn from(value: SseEvent) -> Self {
        match value {
            SseEvent::CrudEvent => Self::default().event("crud_event").data(""),
            SseEvent::CrudPerson => Self::default().event("crud_person").data(""),
            SseEvent::ChangeSignUp { event_id } => Self::default()
                .event(format!("change_sign_up_{event_id}"))
                .data(""), //TODO: get the UUID into the data?
        }
    }
}

pub async fn sse_feed(
    State(state): State<DenimState>,
) -> Sse<impl Stream<Item = Result<AxumSseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.subscribe_to_sse_feed())
        .filter_map(Result::ok)
        .map(|sse_event| Ok(sse_event.into()));

    Sse::new(stream).keep_alive(KeepAlive::default())
}
