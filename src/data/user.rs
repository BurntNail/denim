use crate::{
    auth::PermissionsTarget,
    data::{
        DataType, IdForm,
        student_groups::{HouseGroup, TutorGroup},
    },
    error::{
        BcryptSnafu, DenimResult, EmailSnafu, GetDatabaseConnectionSnafu, MakeQuerySnafu,
        MissingHouseGroupSnafu, MissingTutorGroupSnafu,
    },
};
use axum_login::AuthUser;
use bcrypt::DEFAULT_COST;
use bitflags::bitflags;
use email_address::EmailAddress;
use futures::StreamExt;
use maud::{Markup, Render, html};
use secrecy::{ExposeSecret, SecretString};
use snafu::{OptionExt, ResultExt};
use sqlx::{PgConnection, Pool, Postgres};
use std::{str::FromStr, sync::LazyLock};
use uuid::Uuid;
use crate::maud_conveniences::subtitle;

#[derive(Debug, Clone)]
pub enum UserKind {
    User,
    Student {
        tutor_group: TutorGroup,
        house: HouseGroup,
        events_participated: Vec<Uuid>,
    },
    Staff,
    Admin,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub first_name: String,
    pub pref_name: Option<String>,
    pub surname: String,
    pub email: EmailAddress,
    pub bcrypt_hashed_password: Option<SecretString>,
    pub access_token: Option<SecretString>,
    pub current_password_is_default: bool,
    pub kind: UserKind,
}

pub struct AddPersonForm {
    pub first_name: String,
    pub pref_name: String,
    pub surname: String,
    pub email: EmailAddress,
    pub password: Option<SecretString>,
    pub current_password_is_default: bool,
    pub user_kind: AddUserKind,
}

pub enum AddUserKind {
    Student { tutor_group: Uuid, house: i32 },
    Staff,
    Dev,
}

impl DataType for User {
    type Id = Uuid;
    type FormForId = IdForm;
    type FormForAdding = AddPersonForm;

    async fn get_from_db_by_id(id: Self::Id, conn: &mut PgConnection) -> DenimResult<Option<Self>> {
        let Some(most_bits) = sqlx::query!("SELECT * FROM public.users WHERE id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
        else {
            return Ok(None);
        };

        let user_kind = if sqlx::query!("SELECT * FROM public.admins WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Admin
        } else if sqlx::query!("SELECT * FROM public.staff WHERE user_id = $1", id)
            .fetch_optional(&mut *conn)
            .await
            .context(MakeQuerySnafu)?
            .is_some()
        {
            UserKind::Staff
        } else if let Some(record) =
            sqlx::query!("SELECT * FROM public.students WHERE user_id = $1", id)
                .fetch_optional(&mut *conn)
                .await
                .context(MakeQuerySnafu)?
        {
            let tutor_group = Box::pin(TutorGroup::get_from_db_by_id(
                record.tutor_group_id,
                &mut *conn,
            ))
            .await?
            .context(MissingTutorGroupSnafu {
                id: record.tutor_group_id,
            })?;
            let house = HouseGroup::get_from_db_by_id(record.house_id, &mut *conn)
                .await?
                .context(MissingHouseGroupSnafu {
                    id: record.house_id,
                })?;

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
                tutor_group,
                house,
                events_participated,
            }
        } else {
            UserKind::User
        };

        let email = EmailAddress::from_str(&most_bits.email).context(EmailSnafu)?;

        Ok(Some(Self {
            id,
            first_name: most_bits.first_name,
            pref_name: most_bits.pref_name,
            surname: most_bits.surname,
            email,
            bcrypt_hashed_password: most_bits.bcrypt_hashed_password.map(SecretString::from),
            access_token: most_bits.access_token.map(SecretString::from),
            current_password_is_default: most_bits.current_password_is_default,
            kind: user_kind,
        }))
    }

    async fn get_all(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT id FROM public.users")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    async fn insert_into_database(
        to_be_added: Self::FormForAdding,
        conn: &mut PgConnection,
    ) -> DenimResult<Self::Id> {
        let AddPersonForm {
            first_name,
            pref_name,
            surname,
            email,
            password,
            current_password_is_default,
            user_kind,
        } = to_be_added;

        let pref_name = if pref_name.is_empty() {
            None
        } else {
            Some(pref_name)
        };

        let bcrypt_hashed_password = if let Some(password) = password {
            Some(
                tokio::task::spawn_blocking(move || {
                    bcrypt::hash(password.expose_secret().as_bytes(), DEFAULT_COST)
                })
                .await
                .expect("unable to join tokio task")
                .context(BcryptSnafu)?,
            )
        } else {
            None
        };

        let id = sqlx::query!(
            "INSERT INTO public.users (first_name, pref_name, surname, email, bcrypt_hashed_password, current_password_is_default) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (email) DO UPDATE SET first_name = $1, pref_name = $2, surname = $3, bcrypt_hashed_password = $5, current_password_is_default = $6 RETURNING id",
            first_name, pref_name, surname, email.as_str(), bcrypt_hashed_password, current_password_is_default)
            .fetch_one(&mut *conn).await.context(MakeQuerySnafu)?.id;
        match user_kind {
            AddUserKind::Student { tutor_group, house } => {
                sqlx::query!(
                    "INSERT INTO public.students (user_id, tutor_group_id, house_id) VALUES ($1, $2, $3)",
                    id,
                    tutor_group,
                    house
                )
                .execute(&mut *conn)
                .await
                .context(MakeQuerySnafu)?;
            }
            AddUserKind::Staff => {
                sqlx::query!("INSERT INTO public.staff VALUES ($1)", id)
                    .execute(&mut *conn)
                    .await
                    .context(MakeQuerySnafu)?;
            }
            AddUserKind::Dev => {
                sqlx::query!("INSERT INTO public.admins VALUES ($1)", id)
                    .execute(&mut *conn)
                    .await
                    .context(MakeQuerySnafu)?;
            }
        }

        Ok(id)
    }

    async fn remove_from_database(id: Self::Id, conn: &mut PgConnection) -> DenimResult<()> {
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
            UserKind::User => {
                PermissionsTarget::SEE_PHOTOS | PermissionsTarget::VIEW_SENSITIVE_DETAILS
            }
            UserKind::Student { .. } => {
                PermissionsTarget::SEE_PHOTOS
                    | PermissionsTarget::SIGN_SELF_UP
                    | PermissionsTarget::VIEW_SENSITIVE_DETAILS
            }
            UserKind::Staff => {
                PermissionsTarget::all()
                    - PermissionsTarget::IMPORT_CSVS
                    - PermissionsTarget::CRUD_ADMINS
            }
            UserKind::Admin => PermissionsTarget::all(),
        }
    }

    pub async fn get_all_staff(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT user_id FROM public.staff")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.user_id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    pub async fn get_all_students(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT user_id FROM public.students")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.user_id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }

    pub async fn get_all_admins(pool: &Pool<Postgres>) -> DenimResult<Vec<Self>> {
        let mut first_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;
        let mut second_conn = pool.acquire().await.context(GetDatabaseConnectionSnafu)?;

        let ids = sqlx::query!("SELECT user_id FROM public.admins")
            .fetch(&mut *first_conn)
            .map(|result| result.map(|record| record.user_id))
            .boxed();
        Self::get_from_fetch_stream_of_ids(ids, &mut second_conn).await
    }
    
    pub fn name (&self) -> String {
        self.render().0
    }
}

impl Render for User {
    fn render_to(&self, buffer: &mut String) {
        //if this ever includes HTML, update the name function above
        let first_part = self
            .pref_name
            .as_deref()
            .unwrap_or(self.first_name.as_str());
        let second_part = self.surname.as_str();

        buffer.push_str(first_part);
        buffer.push(' ');

        if matches!(self.kind, UserKind::Student { .. }) {
            buffer.push_str(&second_part[0..1]);
        } else {
            buffer.push_str(second_part);
        }
    }
}

bitflags! {
    pub struct UsernameDisplay: u8 {
        const EMAIL = 0b0000_0001;
        const TITLE = 0b0000_0010;
    }
}

pub struct FullUserNameDisplay<'a>(pub &'a User, pub UsernameDisplay);
impl Render for FullUserNameDisplay<'_> {
    fn render(&self) -> Markup {
        let name_part = html! {
            @if let Some(pref_name) = self.0.pref_name.as_ref() {
                (pref_name)
                " "
                span class="italic text-sm" {
                    "("
                    (self.0.first_name)
                    ")"
                }
                " "
            } @else {
                (self.0.first_name)
            }
            " "
            (self.0.surname)
        };

        let name_part = if self.1.contains(UsernameDisplay::TITLE) {
            subtitle(name_part)
        } else {
            name_part
        };
        let email_part = if self.1.contains(UsernameDisplay::EMAIL) {
            html! {a href={"mailto:" (self.0.email)} target="_blank" class="text-blue-200 underline" {(self.0.email)}}
        } else {
            Markup::default()
        };

        html! {
            (name_part)
            (email_part)
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
