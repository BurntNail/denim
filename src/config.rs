use crate::error::{
    BadEnvVarSnafu, DenimResult, MissingS3BucketSnafu, ParsePortSnafu,
};
use dotenvy::var;
use rand::{Rng, rng};
use s3::Bucket;
use secrecy::{ExposeSecret, SecretString};
use snafu::{OptionExt, ResultExt};
use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, OnceLock},
};
use tokio::sync::{RwLock, RwLockReadGuard};

#[derive(Clone, Debug)]
pub struct RuntimeConfiguration {
    db_config: Arc<DbConfig>,
    auth_config: Arc<RwLock<AuthConfig>>,
    s3_bucket: Arc<OnceLock<Bucket>>,
}

impl RuntimeConfiguration {
    pub fn new() -> DenimResult<Self> {
        Ok(Self {
            db_config: Arc::new(DbConfig::new()?),
            auth_config: Arc::new(RwLock::new(AuthConfig::new())),
            s3_bucket: Arc::new(OnceLock::new()),
        })
    }

    pub fn db_config(&self) -> Arc<DbConfig> {
        self.db_config.clone()
    }

    pub async fn auth_config(&self) -> RwLockReadGuard<'_, AuthConfig> {
        self.auth_config.read().await
    }

    pub async fn set_auth_config(&self, conf: AuthConfig) {
        *self.auth_config.write().await = conf;
    }

    pub fn set_s3_bucket(&self, bucket: Bucket) -> Result<(), Bucket> {
        self.s3_bucket.set(bucket)
    }

    pub fn s3_bucket_exists(&self) -> bool {
        self.s3_bucket.get().is_some()
    }

    pub fn s3_bucket(&self) -> DenimResult<&Bucket> {
        self.s3_bucket.get().context(MissingS3BucketSnafu)
    }
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub word_len_range: Range<usize>,
    words: HashMap<usize, Vec<Arc<str>>>,
    pub numbers_range: Range<usize>,
}

impl AuthConfig {
    pub fn new() -> Self {
        //TODO: let users actually configure this lol
        let default_word_len_range = 5..9;
        let default_numbers_range = 1_000..10_000;

        let words = {
            let all_words = include_str!("../words.txt");
            let mut map: HashMap<usize, Vec<Arc<str>>> = HashMap::new();

            for (len, word) in all_words
                .lines()
                .map(str::trim)
                .map(|word| (word.len(), word))
            {
                map.entry(len).or_default().push(word.into());
            }

            map
        };

        Self {
            word_len_range: default_word_len_range,
            numbers_range: default_numbers_range,
            words,
        }
    }

    pub fn generate(&self) -> Option<String> {
        let mut rng = rng();

        let word_len = rng.random_range(self.word_len_range.clone());
        let list_to_pick_from = self.words.get(&word_len)?;
        let chosen_word = list_to_pick_from[rng.random_range(0..list_to_pick_from.len())].clone();

        let chosen_number = rng.random_range(self.numbers_range.clone());

        Some(format!("{chosen_word}_{chosen_number}"))
    }
}

#[derive(Debug)]
pub struct DbConfig {
    user: String,
    password: SecretString,
    path: String,
    port: u16,
    database: String,
}

impl DbConfig {
    pub fn new() -> DenimResult<Self> {
        let get_env_var = |name| var(name).context(BadEnvVarSnafu { name });

        Ok(Self {
            user: get_env_var("DB_USER")?,
            password: SecretString::from(get_env_var("DB_PASSWORD")?),
            path: get_env_var("DB_PATH")?,
            port: get_env_var("DB_PORT")?.parse().context(ParsePortSnafu)?,
            database: get_env_var("DB_NAME")?,
        })
    }

    pub fn get_db_path(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user,
            self.password.expose_secret(),
            self.path,
            self.port,
            self.database
        )
    }
}
