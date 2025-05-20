#![warn(clippy::pedantic, clippy::all, clippy::nursery)]
#![allow(clippy::single_match_else)]

use crate::{
    auth::{backend::DenimAuthBackend, postgres_store::PostgresSessionStore},
    config::RuntimeConfiguration,
    routes::{
        all_events::{
            delete_event, get_events, internal_get_add_events_form, internal_get_event_in_detail,
            internal_get_events, put_new_event,
        },
        all_people::{
            delete_person, get_people, internal_get_add_dev_or_staff_form,
            internal_get_add_student_form, internal_get_people, internal_get_person_in_detail,
            internal_put_new_staff_or_dev, internal_put_new_student,
        },
        event_in_detail::get_event,
        import_export::{
            get_import_export_page, get_students_import_checker, put_add_new_events,
            put_add_new_students, put_fully_import_events,
        },
        index::get_index_route,
        login::{get_login, post_login, post_logout},
        new_admin_flow::{
            get_start_onboarding, internal_post_add_new_admin, internal_post_setup_auth_config,
            internal_post_setup_s3,
        },
        profile::{
            get_profile, internal_get_profile_edit_email, internal_get_profile_edit_first_name,
            internal_get_profile_edit_password, internal_get_profile_edit_pref_name,
            internal_get_profile_edit_surname, internal_get_profile_student_display,
            internal_get_profile_student_form_house_display, internal_post_profile_edit_email,
            internal_post_profile_edit_first_name, internal_post_profile_edit_password,
            internal_post_profile_edit_pref_name, internal_post_profile_edit_surname,
        },
        set_new_password::{get_replace_default_password, post_replace_default_password},
        sse::sse_feed,
    },
    state::DenimState,
};
use axum::{Router, routing::{get, post, put}};
use axum_login::{
    AuthManagerLayerBuilder,
    tower_sessions::{Expiry, SessionManagerLayer, cookie::time::Duration},
};
use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use crate::routes::new_admin_flow::internal_post_setup_timezone;

#[macro_use]
extern crate tracing;

mod auth;
mod config;
mod data;
mod error;
mod maud_conveniences;
mod routes;
mod state;

async fn shutdown_signal(state: DenimState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    if let Err(e) = state.sensible_shutdown().await {
        error!(?e, "Error sensibly shutting down");
    }
    warn!("signal received, starting graceful shutdown");
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() {
    dotenvy::dotenv().expect("unable to load env vars");

    tracing::subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .finish(),
    )
    .expect("unable to set tracing subscriber");

    info!("`tracing` online");

    let options = PgPoolOptions::new().max_connections(15);
    let config = RuntimeConfiguration::new().expect("unable to create config");
    let state = DenimState::new(options, config.clone())
        .await
        .expect("unable to create state");

    let session_store = PostgresSessionStore::new(state.clone());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(Duration::days(5)));
    let auth_backend = DenimAuthBackend::new(state.clone());
    let auth_layer = AuthManagerLayerBuilder::new(auth_backend, session_layer).build();

    let trace_layer = TraceLayer::new_for_http();

    let app = Router::new()
        .route("/", get(get_index_route))
        .route(
            "/events",
            get(get_events).put(put_new_event).delete(delete_event),
        )
        .route("/event/{id}", get(get_event))
        .route("/people", get(get_people).delete(delete_person))
        .route("/profile", get(get_profile))
        .route("/login", get(get_login).post(post_login))
        .route("/logout", post(post_logout))
        .route(
            "/replace_default_password",
            get(get_replace_default_password).post(post_replace_default_password),
        )
        .route("/import_export", get(get_import_export_page))
        .route("/import_export/import_people", put(put_add_new_students))
        .route("/import_export/import_events", put(put_add_new_events))
        .route(
            "/import_export/fully_import_events",
            put(put_fully_import_events),
        )
        .route(
            "/import_export/import_people_fetch",
            get(get_students_import_checker),
        )
        .route("/onboarding", get(get_start_onboarding))
        .route("/internal/get_people", get(internal_get_people))
        .route("/internal/get_events", get(internal_get_events))
        .route("/internal/get_person", get(internal_get_person_in_detail))
        .route("/internal/get_event", get(internal_get_event_in_detail))
        .route(
            "/internal/events/get_events_form",
            get(internal_get_add_events_form),
        )
        .route(
            "/internal/people/new_staff_or_dev_form",
            get(internal_get_add_dev_or_staff_form).put(internal_put_new_staff_or_dev),
        )
        .route(
            "/internal/people/new_student_form",
            get(internal_get_add_student_form).put(internal_put_new_student),
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
        .route(
            "/internal/onboarding/create_admin",
            post(internal_post_add_new_admin),
        )
        .route(
            "/internal/onboarding/setup_s3",
            post(internal_post_setup_s3),
        )
        .route(
            "/internal/onboarding/setup_auth_config",
            post(internal_post_setup_auth_config),
        )
        .route(
            "/internal/onboarding/setup_timezone",
            post(internal_post_setup_timezone)
        )
        .route("/sse_feed", get(sse_feed))
        .layer(auth_layer)
        .layer(trace_layer)
        .with_state(state.clone());

    let server_ip = env::var("DENIM_SERVER_IP").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&server_ip)
        .await
        .expect("unable to listen on server ip");

    info!(?server_ip, "Listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .expect("unable to serve app");
}
