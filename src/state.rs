use axum::response::{Html};
use maud::{html, Markup};
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
        println!("opening pool");
        let pool = options
            .connect(&config.db_config().get_db_path())
            .await.context(OpenDatabaseSnafu)?;
        
        println!("opened pool");
        
        Ok(Self {
            pool,
            config
        })
    }

    pub fn render (&self, markup: Markup) -> Html<String> {
        let header = html!{
            script src="https://unpkg.com/htmx.org@2.0.4" integrity="sha384-HGfztofotfshcF7+8n44JQL2oJmowVChPTg48S+jvZoztPfvwD79OC/LTtG6dMp+" crossorigin="anonymous" {}
            title { "Denim?" }
        }.into_string();
        let footer = html! {}.into_string();
        let markup = markup.into_string();

        Html(format!("<html><head>{header}</head><body>{markup}{footer}</body></html>"))
    }

    pub async fn get_connection (&self) -> DenimResult<PoolConnection<Postgres>> {
        self.pool.acquire().await.context(GetDatabaseConnectionSnafu)
    }
}