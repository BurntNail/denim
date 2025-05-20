use crate::{
    config::{
        auth::AuthConfig, date_locale::DateLocaleConfig, db::DbConfig,
        important_item::ImportantItemContainer,
    },
    error::DenimResult,
};
use s3::Bucket;
use std::sync::Arc;

pub mod auth;
pub mod date_locale;
pub mod db;
pub mod important_item;

#[derive(Clone, Debug)]
pub struct RuntimeConfiguration {
    db_config: Arc<DbConfig>,
    auth_config: ImportantItemContainer<AuthConfig>,
    s3_bucket: ImportantItemContainer<Bucket>,
    date_locale_config: ImportantItemContainer<DateLocaleConfig>,
}

impl RuntimeConfiguration {
    pub fn new() -> DenimResult<Self> {
        Ok(Self {
            db_config: Arc::new(DbConfig::new()?),
            auth_config: ImportantItemContainer::new(),
            s3_bucket: ImportantItemContainer::new(),
            date_locale_config: ImportantItemContainer::new(),
        })
    }

    pub fn db_config(&self) -> Arc<DbConfig> {
        self.db_config.clone()
    }

    pub fn auth_config(&self) -> ImportantItemContainer<AuthConfig> {
        self.auth_config.clone()
    }

    pub fn s3_bucket(&self) -> ImportantItemContainer<Bucket> {
        self.s3_bucket.clone()
    }

    pub fn date_locale_config(&self) -> ImportantItemContainer<DateLocaleConfig> {
        self.date_locale_config.clone()
    }

    pub async fn save(&self) -> DenimResult<()> {
        if let Ok(bucket) = self.s3_bucket.get() {
            self.auth_config.save(&bucket).await?;
            self.date_locale_config.save(&bucket).await?;
        }

        Ok(())
    }
}
