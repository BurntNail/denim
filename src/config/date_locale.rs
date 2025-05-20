use crate::{
    config::important_item::{ImportantItem, ImportantItemTy},
    error::{
        BadDateTimeFormatterSnafu, DenimError, DenimResult, InvalidLocaleSnafu,
        InvalidTimezoneSnafu, RmpSerdeDecodeSnafu, RmpSerdeEncodeSnafu, S3Snafu,
    },
};
use icu::{
    calendar::preferences::CalendarAlgorithm,
    datetime::{
        DateTimeFormatter, DateTimeFormatterPreferences,
        fieldsets::{YMD, YMDET},
        options::{Alignment, TimePrecision},
        preferences::HourCycle,
    },
    locale::Locale,
    time::ZonedDateTime,
};
use jiff::{Zoned, tz::TimeZone};
use jiff_icu::ConvertFrom;
use s3::{Bucket, error::S3Error};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

#[derive(Debug, Clone)]
pub struct DateLocaleConfig {
    pub timezone: TimeZone,
    pub locale: Locale,
    dtf_prefs: DateTimeFormatterPreferences,
}

#[derive(Deserialize, Serialize)]
struct DateLocaleConfigInterchange {
    tz: String,
    locale: String,
    hour_cycle: String,
    calendar_algorithm: String,
}

impl From<&DateLocaleConfig> for DateLocaleConfigInterchange {
    fn from(value: &DateLocaleConfig) -> Self {
        Self {
            tz: value.timezone.iana_name().unwrap_or("UTC").to_string(),
            locale: value.locale.to_string(),
            hour_cycle: match value.dtf_prefs.hour_cycle {
                Some(HourCycle::H11) => "h11",
                Some(HourCycle::H12) => "h12",
                _ => "h23",
            }
            .to_string(),
            calendar_algorithm: match value.dtf_prefs.calendar_algorithm {
                Some(CalendarAlgorithm::Buddhist) => "buddhist",
                Some(CalendarAlgorithm::Chinese) => "chinese",
                Some(CalendarAlgorithm::Japanese) => "japanese",
                Some(CalendarAlgorithm::Hebrew) => "hebrew",
                Some(CalendarAlgorithm::Dangi) => "dangi",
                _ => "gregorian",
            }
            .to_string(),
        }
    }
}

impl TryFrom<DateLocaleConfigInterchange> for DateLocaleConfig {
    type Error = DenimError;

    fn try_from(
        DateLocaleConfigInterchange {
            tz,
            locale,
            hour_cycle,
            calendar_algorithm,
        }: DateLocaleConfigInterchange,
    ) -> Result<Self, Self::Error> {
        Self::new(tz, locale, hour_cycle, calendar_algorithm)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum DateFormat {
    ShortYMDET,
    LongYMDET,
    ShortYMD,
}

impl DateLocaleConfig {
    fn dtf_prefs_and_locale_from_strings(
        locale: String,
        hour_cycle: String,
        calendar_algorithm: String,
    ) -> DenimResult<(Locale, DateTimeFormatterPreferences)> {
        let locale =
            Locale::try_from_str(&locale).context(InvalidLocaleSnafu { provided: locale })?;
        let hour_cycle = match hour_cycle.as_str() {
            "h23" => HourCycle::H23,
            "h11" => HourCycle::H11,
            "h12" => HourCycle::H12,
            _ => {
                return Err(DenimError::InvalidHourCycle {
                    provided: hour_cycle,
                });
            }
        };
        let calendar_algorithm = match calendar_algorithm.as_str() {
            "gregorian" => CalendarAlgorithm::Iso8601,
            "buddhist" => CalendarAlgorithm::Buddhist,
            "chinese" => CalendarAlgorithm::Chinese,
            "japanese" => CalendarAlgorithm::Japanese,
            "hebrew" => CalendarAlgorithm::Hebrew,
            "dangi" => CalendarAlgorithm::Dangi,
            _ => {
                return Err(DenimError::InvalidCalendarAlgorithm {
                    provided: calendar_algorithm,
                });
            }
        };

        let mut prefs = DateTimeFormatterPreferences::default();
        prefs.locale_preferences = (&locale).into();
        prefs.hour_cycle = Some(hour_cycle);
        prefs.calendar_algorithm = Some(calendar_algorithm);
        Ok((locale, prefs))
    }

    pub fn new(
        timezone: String,
        locale: String,
        hour_cycle: String,
        calendar_algorithm: String,
    ) -> DenimResult<Self> {
        let timezone = TimeZone::get(&timezone).context(InvalidTimezoneSnafu { tz: timezone })?;

        let (locale, dtf_prefs) =
            Self::dtf_prefs_and_locale_from_strings(locale, hour_cycle, calendar_algorithm)?;

        Ok(Self {
            timezone,
            locale,
            dtf_prefs,
        })
    }

    //TODO: optimise these to not re-gen every run
    pub fn format(
        &self,
        zoned: &Zoned,
        date_format: DateFormat,
        set_to_global_timezone: bool,
    ) -> DenimResult<String> {
        let zdt = if set_to_global_timezone {
            let new_tz = zoned.with_time_zone(self.timezone.clone());
            ZonedDateTime::convert_from(&new_tz)
        } else {
            ZonedDateTime::convert_from(zoned)
        };

        Ok(match date_format {
            DateFormat::ShortYMDET => DateTimeFormatter::try_new(self.dtf_prefs, {
                let mut fieldset = YMDET::short();
                fieldset.alignment = Some(Alignment::Column);
                fieldset.time_precision = Some(TimePrecision::Minute);
                fieldset
            })
            .context(BadDateTimeFormatterSnafu)?
            .format(&zdt)
            .to_string(),
            DateFormat::LongYMDET => DateTimeFormatter::try_new(self.dtf_prefs, {
                let mut fieldset = YMDET::long();
                fieldset.alignment = Some(Alignment::Column);
                fieldset.time_precision = Some(TimePrecision::Minute);
                fieldset
            })
            .context(BadDateTimeFormatterSnafu)?
            .format(&zdt)
            .to_string(),
            DateFormat::ShortYMD => DateTimeFormatter::try_new(self.dtf_prefs, {
                let mut fieldset = YMD::short();
                fieldset.alignment = Some(Alignment::Column);
                fieldset
            })
            .context(BadDateTimeFormatterSnafu)?
            .format(&zdt)
            .to_string(),
        })
    }

    pub fn short_ymdet(&self, zoned: &Zoned) -> DenimResult<String> {
        self.format(zoned, DateFormat::ShortYMDET, true)
    }
    pub fn long_ymdet(&self, zoned: &Zoned) -> DenimResult<String> {
        self.format(zoned, DateFormat::LongYMDET, true)
    }

    pub fn short_ymd(&self, zoned: &Zoned) -> DenimResult<String> {
        self.format(zoned, DateFormat::ShortYMD, true)
    }

    pub fn serialise(&self) -> DenimResult<Vec<u8>> {
        let interchange: DateLocaleConfigInterchange = self.into();
        rmp_serde::to_vec(&interchange).context(RmpSerdeEncodeSnafu)
    }
    pub fn deserialise(bytes: impl AsRef<[u8]>) -> DenimResult<Self> {
        let interchange: DateLocaleConfigInterchange =
            rmp_serde::from_slice(bytes.as_ref()).context(RmpSerdeDecodeSnafu)?;
        Self::try_from(interchange)
    }
}

impl ImportantItem for DateLocaleConfig {
    const TY: ImportantItemTy = ImportantItemTy::DateLocaleConfig;

    async fn get_from_bucket(bucket: &Bucket) -> DenimResult<Option<Self>> {
        let rsp = match bucket.get_object("date_locale_config.bin").await {
            Err(S3Error::HttpFailWithBody(404, _)) => return Ok(None),
            Err(e) => return Err(DenimError::S3 { source: e }),
            Ok(rsp) => rsp,
        };
        Self::deserialise(rsp.bytes()).map(Some)
    }

    async fn save_to_bucket(&self, bucket: &Bucket) -> DenimResult<()> {
        let serialised = self.serialise()?;
        bucket
            .put_object_with_content_type(
                "date_locale_config.bin",
                &serialised,
                "application/octet.stream",
            )
            .await
            .context(S3Snafu)?;
        Ok(())
    }
}
