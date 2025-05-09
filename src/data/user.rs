use crate::{
    auth::PermissionsTarget,
    data::{DataType, IdForm},
    error::{DenimError, DenimResult, MakeQuerySnafu},
    maud_conveniences::title,
};
use axum_login::AuthUser;
use futures::StreamExt;
use maud::{Markup, Render, html};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::ResultExt;
use sqlx::{Postgres, Pool, PgConnection};
use std::sync::LazyLock;
use uuid::Uuid;
use crate::error::GetDatabaseConnectionSnafu;

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
        conn: &mut PgConnection,
    ) -> DenimResult<Option<Self>>
    {
        let Some(most_bits) = sqlx::query!("SELECT * FROM public.users WHERE id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        else {
            return Ok(None);
        };

        let user_kind = if sqlx::query!("SELECT * FROM public.developers WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Developer
        } else if sqlx::query!("SELECT * FROM public.staff WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Staff
        } else if let Some(record) = sqlx::query!("SELECT * FROM public.students WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        {
            let form = sqlx::query_as!(
                FormGroup,
                "SELECT * FROM public.forms WHERE id = $1",
                record.form_id
            )
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
            let house = sqlx::query_as!(
                HouseGroup,
                "SELECT * FROM public.houses WHERE id = $1",
                record.house_id
            )
            .fetch_one(&mut *conn)
            .await
            .context(MakeQuerySnafu)?;
            let events_participated = sqlx::query!(
                "SELECT event_id FROM public.participation WHERE student_id = $1",
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

    async fn get_all(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;


        let ids = sqlx::query!("SELECT id FROM public.users").fetch(&mut *first_conn).map(|result| result.map(|record| record.id)).boxed();
        Self::get_ids_from_fetch_stream(ids, &mut *second_conn).await
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id>
    {
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
        Ok(sqlx::query!("INSERT INTO public.users (first_name, pref_name, surname, email) VALUES ($1, $2, $3, $4) RETURNING id", first_name, pref_name, surname, email).fetch_one(conn).await.context(MakeQuerySnafu)?.id)
    }

    async fn remove_from_database(
        id: Self::Id,
        conn: &mut PgConnection,
    ) -> DenimResult<()>
    {
        sqlx::query!("DELETE FROM public.users WHERE id = $1", id)
            .execute(conn)
            .await
            .context(MakeQuerySnafu)?;
        Ok(())
    }
}

impl User {
    pub fn get_permissions(&self) -> PermissionsTarget {
        match self.kind {
            UserKind::User => PermissionsTarget::SEE_PHOTOS,
            UserKind::Student { .. } => {
                PermissionsTarget::SEE_PHOTOS | PermissionsTarget::SIGN_SELF_UP
            }
            UserKind::Staff => PermissionsTarget::all() - PermissionsTarget::IMPORT_CSVS,
            UserKind::Developer => PermissionsTarget::all(),
        }
    }

    pub fn ensure_can(&self, needed: PermissionsTarget) -> DenimResult<()> {
        let found = self.get_permissions();

        if found.contains(needed) {
            Ok(())
        } else {
            Err(DenimError::IncorrectPermissions { needed, found })
        }
    }

    pub async fn get_all_staff(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>>
    {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;


        let ids = sqlx::query!("SELECT user_id FROM public.staff").fetch(&mut *first_conn).map(|result| result.map(|record| record.user_id)).boxed();
        Self::get_ids_from_fetch_stream(ids, &mut *second_conn).await
    }

    pub async fn get_all_students(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT user_id FROM public.students").fetch(&mut *first_conn).map(|result| result.map(|record| record.user_id)).boxed();
        Self::get_ids_from_fetch_stream(ids, &mut *second_conn).await
    }

    pub async fn get_all_developers(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>>
    {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT user_id FROM public.developers").fetch(&mut *first_conn).map(|result| result.map(|record| record.user_id)).boxed();
        Self::get_ids_from_fetch_stream(ids, &mut *second_conn).await
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

pub struct FullUserNameDisplay<'a>(pub &'a User);
impl Render for FullUserNameDisplay<'_> {
    fn render(&self) -> Markup {
        let name_part = html! {
            (self.0.first_name)
            " "
            @if let Some(pref_name) = self.0.pref_name.as_ref() {
                span class="italic" {
                    "\""
                    (pref_name)
                    "\""
                }
                " "
            }
            (self.0.surname)
        };

        html! {
            (title(name_part))
            a href={"mailto:" (self.0.email)} target="_blank" class="text-blue-200 underline" {(self.0.email)}
        }
    }
}

impl AuthUser for User {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        static EMPTY_SECRET_STRING: LazyLock<SecretString> =
            LazyLock::new(|| SecretString::from(""));

        self.access_token
            .as_ref()
            .unwrap_or_else(|| {
                self.bcrypt_hashed_password
                    .as_ref()
                    .unwrap_or(&EMPTY_SECRET_STRING)
            })
            .expose_secret()
            .as_bytes()
    }
}
