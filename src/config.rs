use crate::{
    config::{
        auth::AuthConfig, date_locale::DateLocaleConfig, db::DbConfig,
        important_item::ImportantItemContainer,
    },
    error::DenimResult,
};
use s3::{Bucket, Region};
use std::sync::Arc;
use dotenvy::var;
use s3::creds::Credentials;
use snafu::ResultExt;
use crate::error::{S3CredsSnafu, S3Snafu};

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
    pub async fn new() -> DenimResult<Self> {
        let s3_bucket = ImportantItemContainer::new();
        let mut auth_config_and_date_locale_config = None;
        
        if cfg!(debug_assertions) {
            let get_env_var = |name| var(name).ok();
            
            if let Some(((((access_key_id, secret_access_key), endpoint), region), bucket)) = 
                get_env_var("AWS_ACCESS_KEY_ID")
                    .zip(get_env_var("AWS_SECRET_ACCESS_KEY"))
                    .zip(get_env_var("AWS_ENDPOINT_S3_URL"))
                    .zip(get_env_var("AWS_REGION"))
                    .zip(get_env_var("AWS_BUCKET_NAME")) {
                
                match Credentials::new(
                        Some(&access_key_id),
                        Some(&secret_access_key),
                        None,
                        None,
                        None,
                    )
                    .context(S3CredsSnafu) {
                    Ok(creds) => {
                        let region = Region::Custom { region, endpoint };
                        match Bucket::new(&bucket, region, creds).context(S3Snafu) {
                            Ok(bucket) => {
                                let auth_config = ImportantItemContainer::new();
                                let date_locale_config = ImportantItemContainer::new();
                                
                                if let Err(e) = auth_config.try_set_from_bucket(&bucket).await {
                                    warn!(?e, "Error setting auth config from bucket");
                                }
                                if let Err(e) = date_locale_config.try_set_from_bucket(&bucket).await {
                                    warn!(?e, "Error setting date locale config from bucket");
                                }
                                
                                let _ = s3_bucket.set(*bucket);
                                auth_config_and_date_locale_config = Some((auth_config, date_locale_config));
                            },
                            Err(e) => {
                                warn!(?e, "Error creating new bucket");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(?e, "Error getting credentials for bucket from env vars");
                    }
                }
            }
        }
        
        let (auth_config, date_locale_config) = auth_config_and_date_locale_config.unwrap_or_else(|| (ImportantItemContainer::new(), ImportantItemContainer::new()));
        
        Ok(Self {
            db_config: Arc::new(DbConfig::new()?),
            s3_bucket,
            auth_config,
            date_locale_config,
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
