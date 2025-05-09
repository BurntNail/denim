use crate::{
    auth::{DenimSession, add_password},
    config::RuntimeConfiguration,
    data::{
        DataType,
        user::{AddPersonForm, User},
    },
    error::{
        DenimResult, GeneratePasswordSnafu, GetDatabaseConnectionSnafu, MakeQuerySnafu,
        MigrateSnafu, OpenDatabaseSnafu,
    },
    maud_conveniences::render_nav,
    routes::sse::SseEvent,
};
use maud::{DOCTYPE, Markup, html};
use snafu::{OptionExt, ResultExt};
use sqlx::{Pool, Postgres, Transaction, pool::PoolConnection, postgres::PgPoolOptions};
use std::ops::Deref;
use tokio::sync::broadcast::{Receiver, Sender, channel};

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

    pub async fn ensure_admin_exists(&self) -> DenimResult<()> {
        let mut connection = self.get_connection().await?;

        if sqlx::query!("SELECT exists(SELECT 1 FROM developers)")
            .fetch_one(&mut *connection)
            .await
            .context(MakeQuerySnafu)?
            .exists
            .unwrap_or(false)
        {
            return Ok(());
        }

        //generate user
        let id = User::insert_into_database(
            AddPersonForm {
                first_name: "Example".to_string(),
                pref_name: String::new(),
                surname: "Admin".to_string(),
                email: "example.admin@den.im".to_string(),
            },
            &mut connection,
        )
        .await?;

        //add to devs
        sqlx::query!("INSERT INTO developers (user_id) VALUES ($1)", id)
            .execute(&mut *connection)
            .await
            .context(MakeQuerySnafu)?;

        //generate password
        let password = self
            .config
            .auth_config()
            .generate()
            .context(GeneratePasswordSnafu)?;

        println!("Adding {password:?} for admin user \"example.admin@den.im\"");

        //add password
        add_password(id, password.into(), &mut connection, true).await?;

        Ok(())
    }

    #[allow(clippy::unused_self, clippy::needless_pass_by_value)] //in case self is ever needed :), and to allow direct html! usage
    pub fn render(&self, auth_session: DenimSession, markup: Markup) -> Markup {
        let nav = render_nav(auth_session.user);

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
