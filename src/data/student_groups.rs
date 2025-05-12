use futures::StreamExt;
use crate::{
    data::{DataType, IntIdForm},
    error::{DenimResult, MakeQuerySnafu},
};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt};
use sqlx::{PgConnection, Pool, Postgres};
use uuid::Uuid;
use crate::data::IdForm;
use crate::data::user::User;
use crate::error::{GetDatabaseConnectionSnafu, MissingUserSnafu};

#[derive(Debug, Clone)]
pub struct TutorGroup {
    pub id: Uuid,
    pub staff_member: Box<User>, //break infinite loop - technically I know it can't happen, but eh
    pub house_id: i32,
}

#[derive(Deserialize)]
pub struct NewTutorGroup {
    pub staff_id: Uuid,
    pub house_id: i32,
}

impl DataType for TutorGroup {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = NewTutorGroup;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        let Some(rec) = sqlx::query!("SELECT * FROM public.tutor_groups WHERE id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)? else {
            return Ok(None)
        };
        
        let staff_member = User::get_from_db_by_id(rec.staff_id, conn).await?.context(MissingUserSnafu {id: rec.staff_id})?;
        
        Ok(Some(Self {
            id,
            staff_member: Box::new(staff_member),
            house_id: rec.house_id,
        }))
    }

    async fn get_all(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT id FROM public.tutor_groups")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.id))
            .boxed();

        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    async fn insert_into_database(to_be_added: Self::FormForAdding, conn: &mut PgConnection) -> DenimResult<Self::Id> {
        let id = sqlx::query!("INSERT INTO public.tutor_groups (staff_id, house_id) VALUES ($1, $2) RETURNING id", to_be_added.staff_id, to_be_added.house_id)
            .fetch_one(conn)
            .await
            .context(MakeQuerySnafu)?;
        
        Ok(id.id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM public.tutor_groups WHERE id = $1", id).execute(conn).await.context(MakeQuerySnafu)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HouseGroup {
    pub id: i32,
    pub name: String,
}

#[derive(Deserialize)]
pub struct NewHouse {
    pub name: String,
}

impl DataType for HouseGroup {
    type Id = i32;
    type FormForId = IntIdForm;
    type FormForAdding = NewHouse;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        sqlx::query_as!(HouseGroup, "SELECT * FROM public.houses WHERE id = $1", id)
            .fetch_optional(conn)
            .await
            .context(MakeQuerySnafu)
    }

    async fn get_all(conn: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        sqlx::query_as!(HouseGroup, "SELECT * FROM public.houses")
            .fetch_all(conn)
            .await
            .context(MakeQuerySnafu)
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id> {
        let NewHouse { name } = to_be_added;

        Ok(sqlx::query!(
            "INSERT INTO public.houses (name) VALUES ($1) RETURNING id",
            name
        )
        .fetch_one(conn)
        .await
        .context(MakeQuerySnafu)?
        .id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM public.houses WHERE id = $1", id)
            .execute(conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}
