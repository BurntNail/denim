use crate::error::{CommitTransactionSnafu, DenimResult, MakeQuerySnafu};
use futures::{TryStreamExt, stream::BoxStream};
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{PgConnection, Pool, Postgres, Transaction};
use uuid::Uuid;

pub mod event;
pub mod photo;
pub mod student_groups;
pub mod user;

#[derive(Deserialize)]
pub struct IdForm {
    pub id: Uuid,
}

#[derive(Deserialize)]
pub struct IntIdForm {
    pub id: i32,
}

#[derive(Deserialize)]
pub struct FilterQuery {
    pub filter: Option<String>,
}

//NB: would love to use something more generic
//and i tried
//but
//https://github.com/launchbadge/sqlx/issues/1015

pub trait DataType: Sized {
    type Id;
    type FormForId;
    type FormForAdding;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>>;

    //takes in a pool rather than a connection due to probably needing multiple requests and faffery with streams

    async fn get_all(conn: &Pool<Postgres>) -> DenimResult<Vec<Self>>;

    #[allow(dead_code)]
    async fn get_from_iter_of_ids(
        ids: impl IntoIterator<Item = Self::Id>,
        conn: &mut PgConnection,
    ) -> DenimResult<Vec<Self>> {
        let iter = ids.into_iter();

        let mut all = Vec::with_capacity(iter.size_hint().0);
        for id in iter {
            if let Some(next_event) = Self::get_from_db_by_id(id, conn).await? {
                all.push(next_event);
            }
        }
        Ok(all)
    }

    async fn get_from_fetch_stream_of_ids(
        mut ids: BoxStream<'_, Result<Self::Id, sqlx::Error>>,
        conn: &mut PgConnection,
    ) -> DenimResult<Vec<Self>> {
        let mut all = vec![];

        while let Some(next_id) = ids.try_next().await.context(MakeQuerySnafu)? {
            if let Some(next_event) = Self::get_from_db_by_id(next_id, conn).await? {
                all.push(next_event);
            }
        }

        Ok(all)
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id>;

    async fn insert_into_database_transaction(
        to_be_added: Self::FormForAdding,
        mut conn: Transaction<'_, Postgres>,
    ) -> DenimResult<Self::Id> {
        let id = Self::insert_into_database(to_be_added, &mut conn).await?;
        conn.commit().await.context(CommitTransactionSnafu)?;
        Ok(id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()>;
}
