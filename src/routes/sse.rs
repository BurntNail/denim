use std::convert::Infallible;
use axum::extract::State;
use axum::response::Sse;
use futures::Stream;
use serde::Serialize;
use crate::state::DenimState;

#[derive(Serialize)]
pub enum SseEvent {
    Event,
    Person,
}

pub async fn sse_feed (State(state): State<DenimState>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.subscribe_to_sse_feed();
    
    Sse()
}