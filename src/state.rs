use maud::{html, Markup, DOCTYPE};
use snafu::ResultExt;
use sqlx::{Pool, Postgres};
use sqlx::pool::PoolConnection;
use sqlx::postgres::PgPoolOptions;
use crate::config::RuntimeConfiguration;
use crate::error::{DenimResult, GetDatabaseConnectionSnafu, OpenDatabaseSnafu};

#[derive(Clone)]
pub struct DenimState {
    pool: Pool<Postgres>,
    config: RuntimeConfiguration
}

impl DenimState {
    pub async fn new (options: PgPoolOptions, config: RuntimeConfiguration) -> DenimResult<Self> {
        let pool = options
            .connect(&config.db_config().get_db_path())
            .await.context(OpenDatabaseSnafu)?;
        
        Ok(Self {
            pool,
            config
        })
    }

    pub fn render (&self, markup: Markup) -> Markup {
        html! {
            (DOCTYPE)
            html {
                head {
                    meta charset="UTF-8" {}
                    meta name="viewport" content="width=device-width, initial-scale=1.0" {}
                    script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous" {}
                    script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" {}
                    title { "Denim?" }
                }
                body class="bg-gray-900 h-screen flex flex-col items-center justify-center text-white" {
                    (markup)
                }
            }
        }
    }

    pub async fn get_connection (&self) -> DenimResult<PoolConnection<Postgres>> {
        self.pool.acquire().await.context(GetDatabaseConnectionSnafu)
    }
}