use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::Postgres;
use uuid::Uuid;
use crate::error::DenimResult;
use crate::state::DenimState;

pub mod user;
pub mod event;


#[derive(Deserialize)]
pub struct IdForm {
    pub id: Uuid
}

pub trait DataType: Sized {
    type Id;
    type FormForId;
    type FormForAdding;
    
    async fn get_from_db_by_id (id: Self::Id, conn: PoolConnection<Postgres>) -> DenimResult<Self>;
    async fn get_all (state: DenimState) -> DenimResult<Vec<Self>>;
    async fn insert_into_database (to_be_added: Self::FormForAdding, conn: PoolConnection<Postgres>) -> DenimResult<Self::Id>;
    async fn remove_from_database (id: Self::Id, conn: PoolConnection<Postgres>) -> DenimResult<()>;
}