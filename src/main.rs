#![warn(clippy::pedantic, clippy::all, clippy::nursery)]

use crate::{
    auth::{backend::DenimAuthBackend, postgres_store::PostgresSessionStore},
    config::RuntimeConfiguration,
    routes::{
        events::{
            delete_event, get_events, internal_get_add_events_form, internal_get_event_in_detail,
            internal_get_events, put_new_event,
        },
        index::get_index_route,
        login::{get_login, post_login, post_logout},
        people::{
            delete_person, get_people, internal_get_add_people_form, internal_get_people,
            internal_get_person_in_detail, put_new_person,
        },
        profile::{
            get_profile, internal_get_profile_edit_email, internal_get_profile_edit_first_name,
            internal_get_profile_edit_password, internal_get_profile_edit_pref_name,
            internal_get_profile_edit_surname, internal_get_profile_student_display,
            internal_get_profile_student_form_house_display, internal_post_profile_edit_email,
            internal_post_profile_edit_first_name, internal_post_profile_edit_password,
            internal_post_profile_edit_pref_name, internal_post_profile_edit_surname,
        },
    },
    state::DenimState,
};
use axum::{
    Router,
    routing::{get, post},
};
use axum_login::{
    AuthManagerLayerBuilder,
    tower_sessions::{Expiry, SessionManagerLayer, cookie::time::Duration},
};
use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::net::TcpListener;

mod auth;
mod config;
mod data;
mod error;
mod maud_conveniences;
mod routes;
mod state;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("unable to load env vars");

    let options = PgPoolOptions::new().max_connections(15);
    let config = RuntimeConfiguration::new().expect("unable to create config");
    let state = DenimState::new(options, config.clone())
        .await
        .expect("unable to create state");

    state
        .ensure_admin_exists()
        .await
        .expect("unable to ensure admin exists");

    let session_store = PostgresSessionStore::new(state.clone());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::days(5)));

    let auth_backend = DenimAuthBackend::new(state.clone());
    let auth_layer = AuthManagerLayerBuilder::new(auth_backend, session_layer).build();

    let app = Router::new()
        .route("/", get(get_index_route))
        .route(
            "/events",
            get(get_events).put(put_new_event).delete(delete_event),
        )
        .route(
            "/people",
            get(get_people).put(put_new_person).delete(delete_person),
        )
        .route("/profile", get(get_profile))
        .route("/login", get(get_login).post(post_login))
        .route("/logout", post(post_logout))
        .route("/internal/get_people", get(internal_get_people))
        .route("/internal/get_events", get(internal_get_events))
        .route("/internal/get_person", get(internal_get_person_in_detail))
        .route("/internal/get_event", get(internal_get_event_in_detail))
        .route(
            "/internal/get_events_form",
            get(internal_get_add_events_form),
        )
        .route(
            "/internal/get_people_form",
            get(internal_get_add_people_form()),
        )
        .route(
            "/internal/profile/get_user_specific",
            get(internal_get_profile_student_display),
        )
        .route(
            "/internal/profile/get_student_form_house_display",
            get(internal_get_profile_student_form_house_display),
        )
        .route(
            "/internal/profile/edit_first_name",
            get(internal_get_profile_edit_first_name).post(internal_post_profile_edit_first_name),
        )
        .route(
            "/internal/profile/edit_pref_name",
            get(internal_get_profile_edit_pref_name).post(internal_post_profile_edit_pref_name),
        )
        .route(
            "/internal/profile/edit_surname",
            get(internal_get_profile_edit_surname).post(internal_post_profile_edit_surname),
        )
        .route(
            "/internal/profile/edit_email",
            get(internal_get_profile_edit_email).post(internal_post_profile_edit_email),
        )
        .route(
            "/internal/profile/edit_password",
            get(internal_get_profile_edit_password()).post(internal_post_profile_edit_password),
        )
        .layer(auth_layer)
        .with_state(state);

    let server_ip = env::var("DENIM_SERVER_IP").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&server_ip)
        .await
        .expect("unable to listen on server ip");

    println!("Listening on {server_ip}");
    axum::serve(listener, app)
        .await
        .expect("unable to serve app");
}
