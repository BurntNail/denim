use crate::{error::DenimResult, state::DenimState};
use serde::Deserialize;
use sqlx::{Postgres, pool::PoolConnection};
use uuid::Uuid;

pub mod event;
pub mod user;

#[derive(Deserialize)]
pub struct IdForm {
    pub id: Uuid,
}

pub trait DataType: Sized {
    type Id;
    type FormForId;
    type FormForAdding;

    async fn get_from_db_by_id(
        id: Self::Id,
        conn: PoolConnection<Postgres>,
    ) -> DenimResult<Option<Self>>;
    async fn get_all(state: DenimState) -> DenimResult<Vec<Self>>;
    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: PoolConnection<Postgres>,
    ) -> DenimResult<Self::Id>;
    async fn remove_from_database(id: Self::Id, conn: PoolConnection<Postgres>) -> DenimResult<()>;
}
