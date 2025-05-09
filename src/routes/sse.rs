use std::convert::Infallible;
use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::KeepAlive;
use futures::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use crate::state::DenimState;
use axum::response::sse::Event as AxumSseEvent;

#[derive(Copy, Clone, Debug)]
pub enum SseEvent {
    CrudEvent,
    CrudPerson,
}

impl From<SseEvent> for AxumSseEvent {
    fn from(value: SseEvent) -> Self {
        match value {
            SseEvent::CrudEvent => AxumSseEvent::default().event("crud_event").data(""),
            SseEvent::CrudPerson => AxumSseEvent::default().event("crud_person").data(""),
        }
    }
}

pub async fn sse_feed (State(state): State<DenimState>) -> Sse<impl Stream<Item = Result<AxumSseEvent, Infallible>>> {
    let stream = BroadcastStream::new(
        state.subscribe_to_sse_feed()
    ).filter_map(Result::ok).map(|sse_event| Ok(sse_event.into()));

    Sse::new(
        stream
    ).keep_alive(KeepAlive::default())
}