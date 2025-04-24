use crate::error::{BadEnvVarSnafu, DenimResult, ParsePortSnafu};
use dotenvy::var;
use secrecy::{ExposeSecret, SecretString};
use snafu::ResultExt;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct RuntimeConfiguration {
    db_config: Arc<DbConfig>,
}

impl RuntimeConfiguration {
    pub fn new() -> DenimResult<Self> {
        Ok(Self {
            db_config: Arc::new(DbConfig::new()?),
        })
    }

    pub fn db_config(&self) -> Arc<DbConfig> {
        self.db_config.clone()
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
