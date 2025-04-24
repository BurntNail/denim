use std::env;
use axum::Router;
use axum::routing::get;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use crate::config::RuntimeConfiguration;
use crate::routes::events::{delete_event, get_events, internal_get_add_events_form, internal_get_event_in_detail, internal_get_events, put_new_event};
use crate::routes::index::{get_index_route};
use crate::routes::people::{delete_person, get_people, internal_get_add_people_form, internal_get_people, internal_get_person_in_detail, put_new_person};
use crate::state::DenimState;

mod state;
mod data;
mod error;
mod config;
mod routes;
mod maud_conveniences;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("unable to load env vars");
    
    let options = PgPoolOptions::new().max_connections(15);
    let config = RuntimeConfiguration::new().expect("unable to create config");
    let state = DenimState::new(options, config.clone()).await.expect("unable to create state");

    let app = Router::new()
        .route("/", get(get_index_route))
        .route("/events", get(get_events).put(put_new_event).delete(delete_event))
        .route("/people", get(get_people).put(put_new_person).delete(delete_person))
        .route("/internal/get_people", get(internal_get_people))
        .route("/internal/get_events", get(internal_get_events))
        .route("/internal/get_person", get(internal_get_person_in_detail))
        .route("/internal/get_event", get(internal_get_event_in_detail))
        .route("/internal/get_events_form", get(internal_get_add_events_form))
        .route("/internal/get_people_form", get(internal_get_add_people_form())) //static data, so just call it here to avoid re-calling it every time
        .with_state(state);

    let server_ip = env::var("DENIM_SERVER_IP").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&server_ip).await.expect("unable to listen on server ip");
    
    println!("Listening on {server_ip}");
    axum::serve(listener, app).await.expect("unable to serve app");
}
