use std::sync::LazyLock;
use crate::{
    data::{DataType, IdForm},
    error::{DenimResult, MakeQuerySnafu},
    state::DenimState,
};
use axum_login::AuthUser;
use futures::StreamExt;
use maud::Render;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{Postgres, pool::PoolConnection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FormGroup {
    #[allow(dead_code)]
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct HouseGroup {
    #[allow(dead_code)]
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum UserKind {
    User,
    Student {
        form: FormGroup,
        house: HouseGroup,
        events_participated: Vec<Uuid>,
    },
    Staff,
    Developer,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub pref_name: Option<String>,
    pub surname: String,
    pub email: String,
    pub bcrypt_hashed_password: Option<SecretString>,
    pub access_token: Option<SecretString>,
    #[allow(dead_code)]
    pub current_password_is_default: bool,
    pub kind: UserKind,
}

#[derive(Deserialize)]
pub struct AddPersonForm {
    pub first_name: String,
    pub pref_name: String,
    pub surname: String,
    pub email: String,
}

impl DataType for User {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = AddPersonForm;

    async fn get_from_db_by_id(
        id: Self::Id,
        mut conn: PoolConnection<Postgres>,
    ) -> DenimResult<Option<Self>> {
        let Some(most_bits) = sqlx::query!("SELECT * FROM users WHERE id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        else {
            return Ok(None);
        };

        let user_kind = if sqlx::query!("SELECT * FROM developers WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Developer
        } else if sqlx::query!("SELECT * FROM staff WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Staff
        } else if let Some(record) = sqlx::query!("SELECT * FROM students WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        {
            let form = sqlx::query_as!(
                FormGroup,
                "SELECT * FROM forms WHERE id = $1",
                record.form_id
            )
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
            let house = sqlx::query_as!(
                HouseGroup,
                "SELECT * FROM houses WHERE id = $1",
                record.house_id
            )
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
            let events_participated = sqlx::query!(
                "SELECT event_id FROM participation WHERE student_id = $1",
                id
            )
            .fetch_all(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .into_iter()
            .map(|x| x.event_id)
            .collect();

            UserKind::Student {
                form,
                house,
                events_participated,
            }
        } else {
            UserKind::User
        };

        Ok(Some(Self {
            id,
            first_name: most_bits.first_name,
            pref_name: most_bits.pref_name,
            surname: most_bits.surname,
            email: most_bits.email,
            bcrypt_hashed_password: most_bits.bcrypt_hashed_password.map(SecretString::from),
            access_token: most_bits.access_token.map(SecretString::from),
            current_password_is_default: most_bits.current_password_is_default,
            kind: user_kind,
        }))
    }

    async fn get_all(state: DenimState) -> DenimResult<Vec<Self>> {
        let mut start_connection = state.get_connection().await?;
        let mut ids = sqlx::query!("SELECT id FROM users").fetch(&mut *start_connection);
        let mut all = vec![];

        while let Some(next_id) = ids.next().await {
            let next_id = next_id.context(MakeQuerySnafu)?.id;
            if let Some(next_user) =
                Self::get_from_db_by_id(next_id, state.get_connection().await?).await?
            {
                all.push(next_user);
            }
        }

        Ok(all)
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        mut conn: PoolConnection<Postgres>,
    ) -> DenimResult<Self::Id> {
        let AddPersonForm {
            first_name,
            pref_name,
            surname,
            email,
        } = to_be_added;

        let pref_name = if pref_name.is_empty() {
            None
        } else {
            Some(pref_name)
        };
        Ok(sqlx::query!("INSERT INTO users (first_name, pref_name, surname, email) VALUES ($1, $2, $3, $4) RETURNING id", first_name, pref_name, surname, email).fetch_one(&mut *conn).await.context(MakeQuerySnafu)?.id)
    }

    async fn remove_from_database(
        id: Self::Id,
        mut conn: PoolConnection<Postgres>,
    ) -> DenimResult<()> {
        sqlx::query!("DELETE FROM users WHERE id = $1", id)
            .execute(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}

impl User {
    pub async fn get_all_staff(state: DenimState) -> DenimResult<Vec<Self>> {
        let mut start_connection = state.get_connection().await?;
        let mut ids = sqlx::query!("SELECT user_id FROM staff").fetch(&mut *start_connection);
        let mut all = vec![];

        while let Some(next_id) = ids.next().await {
            let next_id = next_id.context(MakeQuerySnafu)?.user_id;
            if let Some(next_user) =
                Self::get_from_db_by_id(next_id, state.get_connection().await?).await?
            {
                all.push(next_user);
            }
        }

        Ok(all)
    }

    pub async fn get_all_students(state: DenimState) -> DenimResult<Vec<Self>> {
        let mut start_connection = state.get_connection().await?;
        let mut ids = sqlx::query!("SELECT user_id FROM students").fetch(&mut *start_connection);
        let mut all = vec![];

        while let Some(next_id) = ids.next().await {
            let next_id = next_id.context(MakeQuerySnafu)?.user_id;
            if let Some(next_user) =
                Self::get_from_db_by_id(next_id, state.get_connection().await?).await?
            {
                all.push(next_user);
            }
        }

        Ok(all)
    }

    pub async fn get_all_developers(state: DenimState) -> DenimResult<Vec<Self>> {
        let mut start_connection = state.get_connection().await?;
        let mut ids = sqlx::query!("SELECT user_id FROM developers").fetch(&mut *start_connection);
        let mut all = vec![];

        while let Some(next_id) = ids.next().await {
            let next_id = next_id.context(MakeQuerySnafu)?.user_id;
            if let Some(next_user) =
                Self::get_from_db_by_id(next_id, state.get_connection().await?).await?
            {
                all.push(next_user);
            }
        }

        Ok(all)
    }
}

impl Render for User {
    fn render_to(&self, buffer: &mut String) {
        match self.pref_name.as_deref() {
            Some(pn) => buffer.push_str(pn),
            None => buffer.push_str(&self.first_name),
        }
        buffer.push(' ');
        buffer.push_str(&self.surname);
    }
}

impl AuthUser for User {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        static EMPTY_SECRET_STRING: LazyLock<SecretString> = LazyLock::new(|| SecretString::from(""));

        self.access_token.as_ref().unwrap_or_else(
            || self.bcrypt_hashed_password.as_ref()
                .unwrap_or(&EMPTY_SECRET_STRING),
        ).expose_secret().as_bytes()
    }
}
