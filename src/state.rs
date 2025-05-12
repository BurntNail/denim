use crate::{
    auth::DenimSession,
    config::RuntimeConfiguration,
    error::{DenimResult, GetDatabaseConnectionSnafu, MigrateSnafu, OpenDatabaseSnafu},
    routes::sse::SseEvent,
};
use maud::{DOCTYPE, Markup, html};
use snafu::ResultExt;
use sqlx::{Pool, Postgres, Transaction, pool::PoolConnection, postgres::PgPoolOptions};
use std::ops::Deref;
use tokio::sync::broadcast::{Receiver, Sender, channel};
use crate::auth::{AuthUtilities, PermissionsTarget};

#[derive(Clone, Debug)]
pub struct DenimState {
    pool: Pool<Postgres>,
    config: RuntimeConfiguration,
    sse_events_sender: Sender<SseEvent>,
}

impl DenimState {
    pub async fn new(options: PgPoolOptions, config: RuntimeConfiguration) -> DenimResult<Self> {
        let pool = options
            .connect(&config.db_config().get_db_path())
            .await
            .context(OpenDatabaseSnafu)?;

        sqlx::migrate!().run(&pool).await.context(MigrateSnafu)?;

        let (tx, _rx) = channel(1);

        Ok(Self {
            pool,
            config,
            sse_events_sender: tx,
        })
    }

    #[allow(clippy::unused_self, clippy::needless_pass_by_value)] //in case self is ever needed :), and to allow direct html! usage
    pub fn render(&self, auth_session: DenimSession, markup: Markup) -> Markup {
        let nav = render_nav(&auth_session);

        html! {
            (DOCTYPE)
            html {
                head {
                    meta charset="UTF-8" {}
                    meta name="viewport" content="width=device-width, initial-scale=1.0" {}
                    script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous" {}
                    script src="https://unpkg.com/htmx-ext-sse@2.2.3" integrity="sha384-Y4gc0CK6Kg+hmulDc6rZPJu0tqvk7EWlih0Oh+2OkAi1ZDlCbBDCQEE2uVk472Ky" crossorigin="anonymous" {}
                    script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" {}
                    title { "Denim?" }
                }
                body hx-ext="sse" class="bg-gray-900 h-screen flex flex-col items-center justify-center text-white" {
                    (nav)
                    (markup)
                }
            }
        }
    }

    pub async fn get_connection(&self) -> DenimResult<PoolConnection<Postgres>> {
        self.pool
            .acquire()
            .await
            .context(GetDatabaseConnectionSnafu)
    }

    #[allow(dead_code)]
    pub async fn get_transaction(&self) -> DenimResult<Transaction<Postgres>> {
        self.pool.begin().await.context(GetDatabaseConnectionSnafu)
    }

    pub const fn config(&self) -> &RuntimeConfiguration {
        &self.config
    }

    pub fn subscribe_to_sse_feed(&self) -> Receiver<SseEvent> {
        self.sse_events_sender.subscribe()
    }

    pub fn send_sse_event(&self, event: SseEvent) {
        let _ = self.sse_events_sender.send(event);
    }
}

impl Deref for DenimState {
    type Target = Pool<Postgres>;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

fn render_nav(session: &DenimSession) -> Markup {
    let can_view_people = session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS);
    let logged_in_user = session.user.as_ref();
    
    html! {
        nav class="bg-gray-800 shadow fixed top-0 z-10 rounded-lg" id="nav" {
            div class="container mx-auto px-4" {
                @let height_class = if logged_in_user.is_some() {"h-24"} else {"h-16"};
                div class={"flex items-center justify-center space-x-4 " (height_class)} {
                    a href="/events" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"Events"}
                    @if can_view_people {
                        a href="/people" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"People"}
                    }
                    a href="/" class="text-gray-300 bg-fuchsia-900 hover:bg-fuchsia-700 px-3 py-2 rounded-md text-md font-bold" {"Denim"}
                    @match logged_in_user {
                        Some(logged_in_user) => {
                            div class="flex flex-col space-y-2 text-center items-center justify-between" {
                                a href="/profile" id="nav_username" class="text-gray-300 bg-green-900 hover:bg-green-700 px-3 py-2 rounded-md text-sm font-medium" {(logged_in_user)}
                                form method="post" action="/logout" {
                                    input type="submit" value="Logout" class="text-gray-300 bg-red-900 hover:bg-red-700 px-3 py-2 rounded-md text-sm font-medium" {}
                                }
                            }
                        },
                        None => {
                            a href="/login" class="text-gray-300 bg-green-900 hover:bg-green-700 px-3 py-2 rounded-md text-sm font-medium" {"Login"}
                        }
                    }
                }
            }
        }
    }
}
