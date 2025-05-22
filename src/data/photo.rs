use crate::{
    data::{DataType, IdForm},
    error::{
        CommitTransactionSnafu, DenimError, DenimResult, GetDatabaseConnectionSnafu,
        MakeQuerySnafu, RollbackTransactionSnafu, S3Snafu,
    },
};
use futures::StreamExt;
use s3::Bucket;
use snafu::ResultExt;
use sqlx::{PgConnection, Pool, Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug)]
pub struct Photo {
    pub id: Uuid,
    pub event_id: Uuid,
    pub file_name: String,
    pub content_type: String,
}

pub struct NewPhotoForm {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub extension: String,
    pub s3_bucket_to_add_to: Arc<Bucket>,
    pub event_id: Uuid,
}

impl DataType for Photo {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = NewPhotoForm;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        sqlx::query_as!(Photo, "SELECT id, event_id, ('/photos/' || id || '.' || extension) as \"file_name!\", content_type  FROM photos WHERE event_id = $1", id)
            .fetch_optional(conn)
            .await
            .context(MakeQuerySnafu)
    }

    async fn get_all(conn: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = conn.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = conn.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT id FROM photos")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    async fn insert_into_database(
        _: Self::FormForAdding,
        _: &mut PgConnection,
    ) -> DenimResult<Self::Id> {
        Err(DenimError::TransactionMustBeUsed {
            datatype_name: "photo",
        })
    }

    async fn insert_into_database_transaction(
        NewPhotoForm {
            bytes,
            content_type,
            extension,
            s3_bucket_to_add_to,
            event_id,
        }: Self::FormForAdding,
        mut conn: Transaction<'_, Postgres>,
    ) -> DenimResult<Self::Id> {
        let id = sqlx::query!("INSERT INTO photos (event_id, content_type, extension) VALUES ($1, $2, $3) RETURNING id", event_id, &content_type, &extension)
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .id;

        match s3_bucket_to_add_to
            .put_object_with_content_type(
                format!("/photos/{id}.{extension}"),
                &bytes,
                &content_type,
            )
            .await
            .context(S3Snafu)
        {
            Ok(_) => {
                conn.commit().await.context(CommitTransactionSnafu)?;
            }
            Err(e) => {
                error!(?e, "Error uploading photo, rolling back");
                conn.rollback().await.context(RollbackTransactionSnafu)?;
            }
        }

        Ok(id)
    }

    ///ensure the photo has also been removed from S3!
    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
        sqlx::query!("DELETE FROM photos WHERE id = $1", id)
            .execute(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}

impl Photo {
    pub async fn get_s3_url(&self, s3: Arc<Bucket>) -> DenimResult<String> {
        s3.presign_get(
            &self.file_name,
            60 * 5, //5 mins
            None,
        )
        .await
        .context(S3Snafu)
    }
}
