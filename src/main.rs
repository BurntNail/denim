use std::env;
use axum::Router;
use axum::routing::get;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use crate::config::RuntimeConfiguration;
use crate::routes::events::internal_get_events;
use crate::routes::index::{get_index_route};
use crate::routes::people::{internal_get_people, internal_get_person};
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
        .route("/internal/get_people", get(internal_get_people))
        .route("/internal/get_events", get(internal_get_events))
        .route("/internal/get_person", get(internal_get_person))
        .with_state(state);

    let server_ip = env::var("DENIM_SERVER_IP").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&server_ip).await.expect("unable to listen on server ip");
    
    println!("Listening on {server_ip}");
    axum::serve(listener, app).await.expect("unable to serve app");
}
