use crate::{
    data::{DataType, IdForm, photo::Photo, user::User},
    error::{
        DenimError, DenimResult, GetDatabaseConnectionSnafu, InvalidTimezoneSnafu, MakeQuerySnafu,
    },
};
use futures::{StreamExt, TryStreamExt};
use jiff::{Timestamp, Zoned, tz::TimeZone};
use snafu::ResultExt;
use sqlx::{PgConnection, Pool, Postgres};
use time::{Date, Month, PrimitiveDateTime, Time};
use uuid::Uuid;

#[derive(Debug)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub datetime: Zoned,
    pub location: Option<String>,
    pub extra_info: Option<String>,
    pub associated_staff_member: Option<User>,
    pub signed_up: Vec<Uuid>,
    pub verified: Vec<Uuid>,
    pub photos: Vec<Photo>,
}

pub struct AddEvent {
    pub name: String,
    pub date: Zoned,
    pub location: Option<String>,
    pub extra_info: Option<String>,
    pub associated_staff_member: Option<Uuid>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EventSignUpState {
    Nothing,
    SignedUp,
    Verified,
}

impl DataType for Event {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = AddEvent;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        let Some(most_bits) = sqlx::query!("SELECT * FROM public.events WHERE id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        else {
            return Ok(None);
        };
        let associated_staff_member = match most_bits.associated_staff_member {
            Some(id) => User::get_from_db_by_id(id, &mut *conn).await?,
            None => None,
        };

        let timezone =
            TimeZone::get(&most_bits.tz).context(InvalidTimezoneSnafu { tz: most_bits.tz })?;

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        let datetime = {
            let date = most_bits.date.assume_utc();
            Timestamp::new(date.unix_timestamp(), date.nanosecond() as _)
                .expect("`date` guarantees timestamps are in valid intervals")
                .to_zoned(timezone)
        };

        let mut signed_up = vec![];
        let mut verified = vec![];

        let mut participation_stream = sqlx::query!(
            "SELECT student_id, is_verified FROM participation WHERE event_id = $1",
            id
        )
        .fetch(&mut *conn);
        while let Some(record) = participation_stream
            .try_next()
            .await
            .context(MakeQuerySnafu)?
        {
            if record.is_verified {
                verified.push(record.student_id);
            } else {
                signed_up.push(record.student_id);
            }
        }
        drop(participation_stream);

        let photos = Photo::get_by_event_id(id, conn).await?;

        Ok(Some(Self {
            id,
            name: most_bits.name,
            datetime,
            location: most_bits.location,
            extra_info: most_bits.extra_info,
            associated_staff_member,
            signed_up,
            verified,
            photos,
        }))
    }

    async fn get_all(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT id FROM public.events")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id> {
        let AddEvent {
            name,
            date,
            location,
            extra_info,
            associated_staff_member,
        } = to_be_added;

        //verify that the staff member exists :)
        if let Some(asm) = &associated_staff_member {
            if !sqlx::query!(
                "SELECT exists(SELECT 1 FROM public.staff WHERE user_id = $1) as \"exists!\"",
                asm
            )
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .exists
            {
                return Err(DenimError::MissingUser { id: *asm });
            }
        }

        let timestamp = {
            let back_to_utc = date.with_time_zone(TimeZone::UTC);
            let date = back_to_utc.date();
            let time = back_to_utc.time();

            #[allow(clippy::cast_lossless, clippy::cast_sign_loss)]
            PrimitiveDateTime::new(
                Date::from_calendar_date(
                    date.year() as _,
                    Month::try_from(date.month() as u8)
                        .expect("`jiff` assures me the date is in range"),
                    date.day() as _,
                )
                .expect("`jiff` assures me the values are sensible"),
                Time::from_hms_nano(
                    time.hour() as _,
                    time.minute() as _,
                    time.second() as _,
                    time.subsec_nanosecond() as _,
                )
                .expect("`jiff` assures me the values are sensible"),
            )
        };

        let timezone = date.time_zone().iana_name().unwrap_or_else(|| {
            warn!(%name, %date, "Unable to find IANA timezone, using UTC");
            "UTC"
        });

        //gets weird when i try to use query_as, idk
        Ok(sqlx::query!("INSERT INTO public.events (name, date, location, extra_info, associated_staff_member, tz) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id", name, timestamp, location, extra_info, associated_staff_member, timezone).fetch_one(conn).await.context(MakeQuerySnafu)?.id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM public.events WHERE id = $1", id)
            .execute(conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}

impl Event {
    pub async fn get_future_events(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT id FROM public.events WHERE date > NOW() ORDER BY date")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    pub async fn get_past_events(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids =
            sqlx::query!("SELECT id FROM public.events WHERE date <= NOW() ORDER BY date DESC")
                .fetch(&mut *first_conn)
                .map(|result| result.map(|record| record.id))
                .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    pub async fn user_is_signed_up_to_event(
        event_id: Uuid,
        student_id: Uuid,
        conn: &mut PgConnection,
    ) -> DenimResult<Option<EventSignUpState>> {
        if !sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM public.students WHERE user_id = $1)  as \"exists!\"",
            student_id
        )
        .fetch_one(&mut *conn)
        .await
        .context(MakeQuerySnafu)?
        .exists
        {
            return Ok(None);
        }

        Ok(Some(match sqlx::query!("SELECT is_verified FROM public.participation WHERE event_id = $1 AND student_id = $2", event_id, student_id)
            .fetch_optional(conn)
            .await
            .context(MakeQuerySnafu)?
            .map(|rec| rec.is_verified) {
            None => EventSignUpState::Nothing,
            Some(false) => EventSignUpState::SignedUp,
            Some(true) => EventSignUpState::Verified,
        }))
    }
}
