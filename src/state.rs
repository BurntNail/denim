use crate::{
    config::RuntimeConfiguration,
    error::{DenimResult, GetDatabaseConnectionSnafu, OpenDatabaseSnafu},
};
use maud::{DOCTYPE, Markup, html};
use snafu::ResultExt;
use sqlx::{Pool, Postgres, pool::PoolConnection, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct DenimState {
    pool: Pool<Postgres>,
    config: RuntimeConfiguration,
}

impl DenimState {
    pub async fn new(options: PgPoolOptions, config: RuntimeConfiguration) -> DenimResult<Self> {
        let pool = options
            .connect(&config.db_config().get_db_path())
            .await
            .context(OpenDatabaseSnafu)?;

        Ok(Self { pool, config })
    }

    pub fn render(&self, markup: Markup) -> Markup {
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
                    nav class="bg-gray-800 shadow fixed top-0 z-10 rounded-lg" {
                        div class="container mx-auto px-4" {
                            div class="flex items-center justify-center h-16 space-x-4" {
                                a href="/" class="text-gray-300 bg-fuchsia-900 hover:bg-fuchsia-700 px-3 py-2 rounded-md text-md font-bold" {"Denim"}
                                a href="/events" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"Events"}
                                a href="/people" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"People"}
                            }
                        }
                    }

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
}
