use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    config::RuntimeConfiguration,
    error::{DenimResult, GetDatabaseConnectionSnafu, MigrateSnafu, OpenDatabaseSnafu},
    routes::sse::SseEvent,
};
use maud::{DOCTYPE, Markup, html};
use snafu::ResultExt;
use sqlx::{Pool, Postgres, Transaction, pool::PoolConnection, postgres::PgPoolOptions};
use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{
        Mutex,
        broadcast::{Receiver, Sender, channel},
    },
    task::JoinHandle,
};

type LongJobResult = JoinHandle<DenimResult<Markup>>;

#[derive(Clone, Debug)]
pub struct DenimState {
    pool: Pool<Postgres>,
    config: RuntimeConfiguration,
    sse_events_sender: Sender<SseEvent>,
    import_students_job: Arc<Mutex<Option<LongJobResult>>>,
    submit_students_job_token: Arc<AtomicBool>,
}

pub struct SubmitStudentsJobToken {
    has_submitted: bool,
    state: DenimState,
}

impl SubmitStudentsJobToken {
    pub async fn submit_job(mut self, job: LongJobResult) {
        *self.state.import_students_job.lock().await = Some(job);
        self.has_submitted = true;
    }
}

impl Drop for SubmitStudentsJobToken {
    fn drop(&mut self) {
        if !self.has_submitted {
            self.state
                .submit_students_job_token
                .store(false, Ordering::SeqCst);
        }
    }
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
            import_students_job: Arc::new(Mutex::new(None)),
            submit_students_job_token: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn take_import_students_job_result(&self) -> Option<DenimResult<Markup>> {
        let mut lock = self.import_students_job.lock().await;

        let mut is_finished = false;
        if let Some(job) = lock.as_mut() {
            is_finished = job.is_finished();
        }

        if is_finished {
            //looks like there's no other way of doing this, because we only want to take it if it's finished
            let job = lock.take().expect("just checked for it");
            drop(lock);

            self.submit_students_job_token
                .store(false, Ordering::SeqCst);

            job.await.ok() //await should be basically instant because it's finished
        } else {
            None
        }
    }

    pub fn get_submit_students_job_token(&self) -> Option<SubmitStudentsJobToken> {
        if self.submit_students_job_token.swap(true, Ordering::SeqCst) {
            None
        } else {
            Some(SubmitStudentsJobToken {
                state: self.clone(),
                has_submitted: false,
            })
        }
    }

    pub fn student_job_exists(&self) -> bool {
        self.submit_students_job_token.load(Ordering::SeqCst)
    }

    #[allow(clippy::unused_self, clippy::needless_pass_by_value)] //in case self is ever needed :), and to allow direct html! usage
    pub fn render(&self, auth_session: DenimSession, markup: Markup) -> Markup {
        let (height, nav) = render_nav(&auth_session);

        let top_padding = format!("h-{}", height + 4);

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
                body hx-ext="sse" class="bg-gray-900 flex flex-col items-center text-white" {
                    (nav)
                    div class={(top_padding) " bg-transparent"} {""}
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

fn render_nav(session: &DenimSession) -> (u32, Markup) {
    let can_view_people = session.can(PermissionsTarget::VIEW_SENSITIVE_DETAILS);
    let can_import_export = session.can(PermissionsTarget::IMPORT_CSVS);

    let logged_in_user = session.user.as_ref();

    let height = if logged_in_user.is_some() { 24 } else { 16 };

    (
        height,
        html! {
            nav class="bg-gray-800 shadow fixed top-0 z-10 rounded-lg" id="nav" {
                div class="container mx-auto px-4" {
                    @let height_class = format!("h-{height}");
                    div class={"flex items-center justify-center space-x-4 " (height_class)} {
                        a href="/events" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"Events"}
                        @if can_view_people {
                            a href="/people" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"People"}
                        }
                        @if can_import_export {
                            a href="/import_export" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"Import/Export CSVs"}
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
        },
    )
}
