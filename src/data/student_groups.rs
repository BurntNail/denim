use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{PgConnection, Pool, Postgres};
use crate::data::{DataType, IntIdForm};
use crate::error::{DenimResult, MakeQuerySnafu};

#[derive(Debug, Clone)]
pub struct FormGroup {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct HouseGroup {
    pub id: i32,
    pub name: String,
}

#[derive(Deserialize)]
pub struct NewHouseOrFormGroup {
    pub name: String,
}

impl DataType for FormGroup {
    type Id = i32;
    type FormForId = IntIdForm;
    type FormForAdding = NewHouseOrFormGroup;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        sqlx::query_as!(FormGroup, "SELECT * FROM public.forms WHERE id = $1", id)
            .fetch_optional(conn)
            .await
            .context(MakeQuerySnafu)
    }

    async fn get_all(conn: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        sqlx::query_as!(FormGroup, "SELECT * FROM public.forms")
            .fetch_all(conn)
            .await
            .context(MakeQuerySnafu)
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id> {
        let NewHouseOrFormGroup { name } = to_be_added;

        Ok(sqlx::query!(
            "INSERT INTO public.forms (name) VALUES ($1) RETURNING id",
            name
        )
            .fetch_one(conn)
            .await
            .context(MakeQuerySnafu)?
            .id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM public.forms WHERE id = $1", id)
            .execute(conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}

impl DataType for HouseGroup {
    type Id = i32;
    type FormForId = IntIdForm;
    type FormForAdding = NewHouseOrFormGroup;

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
        let NewHouseOrFormGroup { name } = to_be_added;

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
