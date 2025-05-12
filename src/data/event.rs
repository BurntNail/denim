use crate::{
    data::{DataType, IdForm, user::User},
    error::{
        DenimResult, GetDatabaseConnectionSnafu, MakeQuerySnafu, ParseTimeSnafu, ParseUuidSnafu,
    },
};
use chrono::NaiveDateTime;
use futures::StreamExt;
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{PgConnection, Pool, Postgres};
use uuid::Uuid;

#[derive(Debug)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub date: NaiveDateTime,
    pub location: Option<String>,
    pub extra_info: Option<String>,
    pub associated_staff_member: Option<User>,
}

#[derive(Deserialize)]
pub struct AddEventForm {
    pub name: String,
    pub date: String,
    pub location: String,
    pub extra_info: String,
    pub associated_staff_member: String,
}

impl DataType for Event {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = AddEventForm;

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

        Ok(Some(Self {
            id,
            name: most_bits.name,
            date: most_bits.date,
            location: most_bits.location,
            extra_info: most_bits.extra_info,
            associated_staff_member,
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
        let AddEventForm {
            name,
            date,
            location,
            extra_info,
            associated_staff_member,
        } = to_be_added;

        let date = NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M")
            .context(ParseTimeSnafu { original: date })?;

        let location = if location.is_empty() {
            None
        } else {
            Some(location)
        };
        let extra_info = if extra_info.is_empty() {
            None
        } else {
            Some(extra_info)
        };
        let associated_staff_member = if associated_staff_member.is_empty() {
            None
        } else {
            Some(
                Uuid::try_parse(&associated_staff_member).context(ParseUuidSnafu {
                    original: associated_staff_member,
                })?,
            )
        };

        //gets weird when i try to use query_as, idk
        Ok(sqlx::query!("INSERT INTO public.events (name, date, location, extra_info, associated_staff_member) VALUES ($1, $2, $3, $4, $5) RETURNING id", name, date, location, extra_info, associated_staff_member).fetch_one(conn).await.context(MakeQuerySnafu)?.id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM public.events WHERE id = $1", id)
            .execute(conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}
