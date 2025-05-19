use crate::error::{BadDateTimeFormatterSnafu, BadEnvVarSnafu, DenimError, DenimResult, GeneratePasswordSnafu, InvalidLocaleSnafu, InvalidTimezoneSnafu, ParsePortSnafu};
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
use icu::calendar::preferences::CalendarAlgorithm;
use icu::datetime::{DateTimeFormatter, DateTimeFormatterPreferences};
use icu::datetime::fieldsets::{YMD, YMDET};
use icu::datetime::options::{Alignment, TimePrecision};
use icu::datetime::preferences::HourCycle;
use icu::locale::Locale;
use icu::time::ZonedDateTime;
use jiff::tz::TimeZone;
use jiff::Zoned;
use jiff_icu::ConvertFrom;
use tokio::sync::{RwLock, RwLockReadGuard};

#[derive(Clone, Debug)]
pub struct RuntimeConfiguration {
    db_config: Arc<DbConfig>,
    auth_config: Arc<RwLock<AuthConfig>>,
    s3_bucket: ImportantItem<Bucket>,
    date_locale_config: ImportantItem<DateLocaleConfig>
}

#[derive(Copy, Clone, Debug)]
pub enum ImportantItemTy {
    Bucket,
    DateLocaleConfig
}

#[derive(Debug)]
pub struct ImportantItem<T>(Arc<OnceLock<T>>, ImportantItemTy);
impl<T> ImportantItem<T> {
    pub fn new (kind: ImportantItemTy) -> Self {
        Self(Arc::new(OnceLock::new()), kind)
    }

    pub fn exists (&self) -> bool {
        self.0.get().is_some()
    }

    pub fn get (&self) -> DenimResult<&T> {
        self.0.get().ok_or_else(|| self.1.into())
    }
    
    pub fn set (&self, item: T) -> Result<(), T> {
        self.0.set(item)
    } 
}

impl<T> Clone for ImportantItem<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), self.1)
    }
}

impl<T: Clone> ImportantItem<T> {
    #[allow(dead_code)]
    pub fn get_owned (&self) -> DenimResult<T> {
        self.0.get().ok_or_else(|| self.1.into()).cloned()
    }
}

impl RuntimeConfiguration {
    pub fn new() -> DenimResult<Self> {
        Ok(Self {
            db_config: Arc::new(DbConfig::new()?),
            auth_config: Arc::new(RwLock::new(AuthConfig::new())),
            s3_bucket: ImportantItem::new(ImportantItemTy::Bucket),
            date_locale_config: ImportantItem::new(ImportantItemTy::DateLocaleConfig),
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

    pub fn s3_bucket (&self) -> ImportantItem<Bucket> {
        self.s3_bucket.clone()
    }
    
    pub fn date_locale_config(&self) -> ImportantItem<DateLocaleConfig> {
        self.date_locale_config.clone()
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

    pub fn generate(&self) -> DenimResult<String> {
        let mut rng = rng();

        let word_len = rng.random_range(self.word_len_range.clone());
        let list_to_pick_from = self.words.get(&word_len).context(GeneratePasswordSnafu)?;
        let chosen_word = list_to_pick_from[rng.random_range(0..list_to_pick_from.len())].clone();

        let chosen_number = rng.random_range(self.numbers_range.clone());

        Ok(format!("{chosen_word}_{chosen_number}"))
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

#[derive(Debug, Clone)]
pub struct DateLocaleConfig {
    pub timezone: TimeZone,
    pub locale: Locale,
    dtf_prefs: DateTimeFormatterPreferences,
}

impl DateLocaleConfig {
    pub fn new ( timezone: String, locale: String, hour_cycle: String, calendar_algorithm: String) -> DenimResult<Self> {
        let timezone = TimeZone::get(&timezone).context(InvalidTimezoneSnafu {
            tz: timezone
        })?;
        let locale = Locale::try_from_str(&locale).context(InvalidLocaleSnafu {provided: locale})?;
        let hour_cycle = match hour_cycle.as_str() {
            "h23" => HourCycle::H23,
            "h11" => HourCycle::H11,
            "h12" => HourCycle::H12,
            _ => return Err(DenimError::InvalidHourCycle {provided: hour_cycle})
        };
        let calendar_algorithm = match calendar_algorithm.as_str() {
            "gregorian" => CalendarAlgorithm::Iso8601,
            "buddhist" => CalendarAlgorithm::Buddhist,
            "chinese" => CalendarAlgorithm::Chinese,
            "japanese" => CalendarAlgorithm::Japanese,
            "hebrew" => CalendarAlgorithm::Hebrew,
            "dangi" => CalendarAlgorithm::Dangi,
            _ => return Err(DenimError::InvalidCalendarAlgorithm {provided: calendar_algorithm})
        };

        let dtf_prefs = {
            let mut prefs = DateTimeFormatterPreferences::default();
            prefs.locale_preferences = (&locale).into();
            prefs.hour_cycle = Some(hour_cycle);
            prefs.calendar_algorithm = Some(calendar_algorithm);
            prefs
        };
        
        Ok(Self {
            timezone,
            locale,
            dtf_prefs
        })
    }

    //TODO: optimise these to not re-gen every run
    pub fn short_ymdet (&self, zoned: Zoned) -> DenimResult<String> {
        let zoned = zoned.with_time_zone(self.timezone.clone());
        let zdt = ZonedDateTime::convert_from(&zoned);

        let short_ymdet_formatter = DateTimeFormatter::try_new(
            self.dtf_prefs,
            {
                let mut fieldset = YMDET::short();
                fieldset.alignment = Some(Alignment::Column);
                fieldset.time_precision = Some(TimePrecision::Minute);
                fieldset
            }
        ).context(BadDateTimeFormatterSnafu)?;

        Ok(short_ymdet_formatter.format(&zdt).to_string())
    }
    pub fn long_ymdet (&self, zoned: Zoned) -> DenimResult<String> {
        let zoned = zoned.with_time_zone(self.timezone.clone());
        let zdt = ZonedDateTime::convert_from(&zoned);

        let long_ymdet_formatter = DateTimeFormatter::try_new(
            self.dtf_prefs,
            {
                let mut fieldset = YMDET::long();
                fieldset.alignment = Some(Alignment::Column);
                fieldset.time_precision = Some(TimePrecision::Minute);
                fieldset
            }
        ).context(BadDateTimeFormatterSnafu)?;

        Ok(long_ymdet_formatter.format(&zdt).to_string())
    }
    
    pub fn short_ymd (&self, zoned: Zoned) -> DenimResult<String> {
        let zoned = zoned.with_time_zone(self.timezone.clone());
        let zdt = ZonedDateTime::convert_from(&zoned);

        let short_ymd_formatter = DateTimeFormatter::try_new(
            self.dtf_prefs,
            {
                let mut fieldset = YMD::short();
                fieldset.alignment = Some(Alignment::Column);
                fieldset
            }
        ).context(BadDateTimeFormatterSnafu)?;

        Ok(short_ymd_formatter.format(&zdt).to_string())
    }
}