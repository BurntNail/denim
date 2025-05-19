use crate::{
    error::{
        DenimError, MakeQuerySnafu,
        RmpSerdeEncodeSnafu,
    },
    state::DenimState,
};
use async_trait::async_trait;
use axum_login::tower_sessions::{
    ExpiredDeletion, SessionStore,
    session::{Id, Record},
    session_store::Error as SSError,
};
use snafu::{ResultExt};
use sqlx::PgConnection;

#[derive(Debug, Clone)]
pub struct PostgresSessionStore {
    state: DenimState,
}

impl PostgresSessionStore {
    pub const fn new(state: DenimState) -> Self {
        Self { state }
    }
}

impl PostgresSessionStore {
    async fn id_exists(id: Id, conn: &mut PgConnection) -> Result<bool, DenimError> {
        Ok(sqlx::query!(
            "SELECT EXISTS (SELECT 1 FROM public.sessions WHERE id = $1)",
            id.to_string()
        )
        .fetch_one(conn)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false))
    }

    async fn save_session(record: &Record, conn: &mut PgConnection) -> Result<(), DenimError> {
        let serialised_data = rmp_serde::to_vec(&record.data).context(RmpSerdeEncodeSnafu)?;

        sqlx::query!("INSERT INTO sessions VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET data = excluded.data, expiry_date = excluded.expiry_date", record.id.to_string(), serialised_data, record.expiry_date)
            .execute(conn)
            .await.context(MakeQuerySnafu)?;

        Ok(())
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn create(&self, session_record: &mut Record) -> Result<(), SSError> {
        let mut connection = self
            .state
            .get_connection()
            .await
            .map_err(|e| SSError::Backend(e.to_string()))?;

        while Self::id_exists(session_record.id, &mut connection)
            .await
            .map_err(|e| SSError::Encode(e.to_string()))?
        {
            session_record.id = Id::default();
        }

        //TODO: ensure we can't get duplicate IDs here through some sort of lock

        Self::save_session(session_record, &mut connection)
            .await
            .map_err(|e| SSError::Encode(e.to_string()))?;

        Ok(())
    }

    async fn save(&self, session_record: &Record) -> Result<(), SSError> {
        let mut connection = self
            .state
            .get_connection()
            .await
            .map_err(|e| SSError::Backend(e.to_string()))?;

        Self::save_session(session_record, &mut connection)
            .await
            .map_err(|e| SSError::Encode(e.to_string()))?;

        Ok(())
    }

    async fn load(&self, session_id: &Id) -> Result<Option<Record>, SSError> {
        let mut connection = self
            .state
            .get_connection()
            .await
            .map_err(|e| SSError::Backend(e.to_string()))?;

        let Some(sql_record) = sqlx::query!(
            "SELECT * FROM public.sessions WHERE id = $1",
            session_id.to_string()
        )
        .fetch_optional(&mut *connection)
        .await
        .context(MakeQuerySnafu)
        .map_err(|e| SSError::Decode(e.to_string()))?
        else {
            return Ok(None);
        };

        let id = *session_id;
        let data =
            rmp_serde::from_slice(&sql_record.data).map_err(|e| SSError::Decode(e.to_string()))?;
        let expiry_date = sql_record.expiry_date;
        
        Ok(Some(Record {
            id,
            data,
            expiry_date,
        }))
    }

    async fn delete(&self, session_id: &Id) -> Result<(), SSError> {
        let mut connection = self
            .state
            .get_connection()
            .await
            .map_err(|e| SSError::Backend(e.to_string()))?;

        sqlx::query!(
            "DELETE FROM public.sessions WHERE id = $1",
            session_id.to_string()
        )
        .execute(&mut *connection)
        .await
        .context(MakeQuerySnafu)
        .map_err(|e| SSError::Backend(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for PostgresSessionStore {
    async fn delete_expired(&self) -> Result<(), SSError> {
        let mut connection = self
            .state
            .get_connection()
            .await
            .map_err(|e| SSError::Backend(e.to_string()))?;

        sqlx::query!("DELETE FROM public.sessions WHERE expiry_date < now()")
            .execute(&mut *connection)
            .await
            .context(MakeQuerySnafu)
            .map_err(|e| SSError::Backend(e.to_string()))?;
        Ok(())
    }
}
