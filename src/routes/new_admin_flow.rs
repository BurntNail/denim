use crate::{
    auth::{AuthUtilities, DenimSession, PermissionsTarget},
    data::{
        DataType,
        user::{AddPerson, AddUserKind, User},
    },
    error::{
        BadEnvVarSnafu, CommitTransactionSnafu, DenimResult, MakeQuerySnafu,
        RollbackTransactionSnafu, S3CredsSnafu, S3Snafu,
    },
    maud_conveniences::{
        errors_list, form_element, form_submit_button, simple_form_element, supertitle, title,
    },
    state::DenimState,
};
use axum::{
    Form,
    body::Body,
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use bitflags::bitflags;
use dotenvy::var;
use email_address::EmailAddress;
use maud::{Markup, html};
use s3::{Bucket, Region, creds::Credentials};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use snafu::ResultExt;

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    struct NewAdminDetailsError: u16 {
        const EMPTY_FIRST_NAME =  0b0000_0000_0000_0001;
        const EMPTY_SURNAME =     0b0000_0000_0000_0010;
        const EMPTY_PASSWORD =    0b0000_0000_0000_0100;

        const MISMATCH_PASSWORD = 0b0000_0000_0010_0000;
    }
}

impl NewAdminDetailsError {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|x| match x {
            Self::EMPTY_FIRST_NAME => Some("Provided First Name was empty"),
            Self::EMPTY_SURNAME => Some("Provided surname was empty"),
            Self::EMPTY_PASSWORD => Some("Provided password was empty"),
            Self::MISMATCH_PASSWORD => Some("Passwords didn't match"),
            _ => None,
        })
    }
}

pub async fn get_start_onboarding(
    State(state): State<DenimState>,
    session: DenimSession,
) -> DenimResult<Response<Body>> {
    let mut replacement_internal_markup = None;

    //double check that no admins exist
    if sqlx::query!("SELECT exists(SELECT 1 FROM public.admins)")
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false)
    {
        if session.can(PermissionsTarget::RUN_ONBOARDING) {
            replacement_internal_markup = Some(
                internal_get_setup_s3(State(state.clone()), session.clone(), S3Failure::empty())
                    .await?,
            );
        } else {
            return Ok(Redirect::to("/").into_response());
        }
    }

    let interior = replacement_internal_markup
        .unwrap_or_else(|| internal_get_create_admin_account(NewAdminDetailsError::empty()));

    Ok(state.render(session, html! {
        div class="flex items-center justify-center" {
            div id="current_section" class="bg-gray-800 p-8 rounded-lg shadow-xl w-full max-w-md" {
                (interior)
            }
        }
    }).into_response())
}

fn internal_get_create_admin_account(errors: NewAdminDetailsError) -> Markup {
    html! {
        (supertitle("Create new Admin Account"))

        @if !errors.is_empty() {
            (errors_list(None, errors.as_nice_list()))
        }

        form hx-post="/internal/onboarding/create_admin" hx-target="#current_section" {
            (simple_form_element("first_name", "First Name", true, None, None))
            (simple_form_element("pref_name", "Preferred Name", false, None, None))
            (simple_form_element("surname", "Surname", true, None, None))
            (simple_form_element("email", "Email", true, Some("email"), None))
            (simple_form_element("password", "Password", true, Some("password"), None))
            (simple_form_element("confirm_password", "Confirm Password", true, Some("password"), None))
            (form_submit_button(Some("Create Admin User")))
        }
    }
}

#[derive(Deserialize)]
pub struct CreateAdminAccountForm {
    first_name: String,
    pref_name: String,
    surname: String,
    email: EmailAddress,
    password: SecretString,
    confirm_password: SecretString,
}

pub async fn internal_post_add_new_admin(
    State(state): State<DenimState>,
    mut session: DenimSession,
    Form(CreateAdminAccountForm {
        first_name,
        pref_name,
        surname,
        email,
        password,
        confirm_password,
    }): Form<CreateAdminAccountForm>,
) -> DenimResult<Markup> {
    let mut transaction = state.get_transaction().await?;

    //double check that no admins exist
    if sqlx::query!("SELECT exists(SELECT 1 FROM public.admins)")
        .fetch_one(&mut *transaction)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false)
    {
        transaction
            .rollback()
            .await
            .context(RollbackTransactionSnafu)?;

        return Ok(if session.can(PermissionsTarget::RUN_ONBOARDING) {
            internal_get_setup_s3(State(state), session, S3Failure::empty()).await?
        } else {
            errors_list(
                Some("Admin Account already exists, and it isn't you."),
                std::iter::empty::<String>(),
            )
        });
    }

    let mut errors = NewAdminDetailsError::empty();
    if first_name.is_empty() {
        errors |= NewAdminDetailsError::EMPTY_FIRST_NAME;
    }
    if surname.is_empty() {
        errors |= NewAdminDetailsError::EMPTY_SURNAME;
    }
    if password.expose_secret().trim().is_empty() {
        errors |= NewAdminDetailsError::EMPTY_PASSWORD;
    }
    if password.expose_secret() != confirm_password.expose_secret() {
        errors |= NewAdminDetailsError::MISMATCH_PASSWORD;
    }

    if !errors.is_empty() {
        return Ok(internal_get_create_admin_account(errors));
    }

    let id = User::insert_into_database(
        AddPerson {
            first_name,
            pref_name,
            surname,
            email,
            password: Some(password),
            current_password_is_default: false,
            user_kind: AddUserKind::Dev,
        },
        &mut transaction,
    )
    .await?;

    let user = User::get_from_db_by_id(id, &mut transaction)
        .await?
        .expect("just added user to the database w/o issue");
    transaction.commit().await.context(CommitTransactionSnafu)?;

    session.login(&user).await?;

    internal_get_setup_s3(State(state), session, S3Failure::empty()).await
}

bitflags! {
    #[derive(Eq, PartialEq)]
    pub struct S3Failure: u8 {
        const EMPTY_ACCESS_ID =  0b0000_0010;
        const EMPTY_ACCESS_KEY = 0b0000_0100;
        const EMPTY_ENDPOINT =   0b0000_1000;
        const EMPTY_REGION =     0b0001_0000;
        const EMPTY_BUCKET =     0b0010_0000;

        const BUCKET_NOT_EXIST = 0b0000_0001;
        const OTHER_S3_ERROR =   0b0100_0000;
    }
}

impl S3Failure {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|x| match x {
            Self::EMPTY_ACCESS_ID => Some("Empty Access ID"),
            Self::EMPTY_ACCESS_KEY => Some("Empty Access Key"),
            Self::EMPTY_ENDPOINT => Some("Empty Endpoint URL"),
            Self::EMPTY_REGION => Some("Empty Region Name"),
            Self::EMPTY_BUCKET => Some("Empty Bucket Name"),
            Self::BUCKET_NOT_EXIST => Some("Provided bucket does not exist"),
            Self::OTHER_S3_ERROR => Some("Error connecting with S3"),
            _ => None,
        })
    }
}

async fn internal_get_setup_s3(
    State(state): State<DenimState>,
    session: DenimSession,
    failure: S3Failure,
) -> DenimResult<Markup> {
    if state.config().s3_bucket_exists() {
        return internal_get_setup_auth_config(State(state), session, AuthConfigFailure::empty())
            .await;
    }

    session.ensure_can(PermissionsTarget::RUN_ONBOARDING)?;

    let required = !cfg!(debug_assertions); //;)

    Ok(html! {
        (title("Setup External Storage"))
        p {
            "Now that you've got your admin account, we now need to setup S3 storage for things like photos."
        }

        @if !failure.is_empty() {
            br;
            (errors_list(Some("Validation Errors"), failure.as_nice_list()))
        }

        br;
        p class="italic" {
            "NB: After clicking submit, it can sometimes take a moment to check that your bucket is all OK."
            br;
            "Don't re-click submit, just give it a second."
        }

        br;
        form hx-post="/internal/onboarding/setup_s3" hx-target="#current_section" {
            (simple_form_element(
                "access_key_id",
                "S3 Access Key ID",
                required,
                Some("password"),
                None
            ))
            (simple_form_element(
                "secret_access_key",
                "S3 Secret Access Key",
                required,
                Some("password"),
                None
            ))
            (simple_form_element(
                "endpoint",
                "S3 Endpoint URL",
                required,
                None,
                None
            ))
            (simple_form_element(
                "region",
                "S3 Region",
                required,
                None,
                None
            ))
            (simple_form_element(
                "bucket",
                "S3 Bucket Name",
                required,
                None,
                None
            ))

            (form_submit_button(Some("Add S3 Bucket")))
        }
    })
}

#[derive(Deserialize)]
pub struct S3Details {
    access_key_id: String,
    secret_access_key: String,
    endpoint: String,
    region: String,
    bucket: String,
}

pub async fn internal_post_setup_s3(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(S3Details {
        access_key_id,
        secret_access_key,
        endpoint,
        region,
        bucket,
    }): Form<S3Details>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::RUN_ONBOARDING)?;
    if state.config().s3_bucket_exists() {
        return internal_get_setup_auth_config(State(state), session, AuthConfigFailure::empty())
            .await;
    }

    let mut errors = S3Failure::empty();
    if access_key_id.trim().is_empty() {
        errors |= S3Failure::EMPTY_ACCESS_ID;
    }
    if secret_access_key.trim().is_empty() {
        errors |= S3Failure::EMPTY_ACCESS_KEY;
    }
    if endpoint.trim().is_empty() {
        errors |= S3Failure::EMPTY_ENDPOINT;
    }
    if region.trim().is_empty() {
        errors |= S3Failure::EMPTY_ENDPOINT;
    }
    if bucket.trim().is_empty() {
        errors |= S3Failure::EMPTY_BUCKET;
    }

    let (access_key_id, secret_access_key, endpoint, region, bucket) = if errors.is_empty() {
        (access_key_id, secret_access_key, endpoint, region, bucket)
    } else if cfg!(debug_assertions) {
        //avoid the faff when running in dev ;)
        let get_env_var = |name| var(name).context(BadEnvVarSnafu { name });

        (
            get_env_var("AWS_ACCESS_KEY_ID")?,
            get_env_var("AWS_SECRET_ACCESS_KEY")?,
            get_env_var("AWS_ENDPOINT_S3_URL")?,
            get_env_var("AWS_REGION")?,
            get_env_var("AWS_BUCKET_NAME")?,
        )
    } else {
        return internal_get_setup_s3(State(state), session, errors).await;
    };

    let creds = Credentials::new(
        Some(&access_key_id),
        Some(&secret_access_key),
        None,
        None,
        None,
    )
    .context(S3CredsSnafu)?;
    let region = Region::Custom { region, endpoint };
    let bucket = Bucket::new(&bucket, region, creds).context(S3Snafu)?;

    let bucket_is_bad = match bucket.exists().await {
        Ok(false) => Some(S3Failure::BUCKET_NOT_EXIST),
        Ok(true) => None,
        Err(e) => {
            warn!(?e, "Tried to connect to bad bucket");
            Some(S3Failure::OTHER_S3_ERROR)
        }
    };
    if let Some(bucket_is_bad) = bucket_is_bad {
        return internal_get_setup_s3(State(state), session, bucket_is_bad).await;
    }

    if state.config().set_s3_bucket(*bucket).is_err() {
        error!("Tried to add new S3 bucket when one already existed...");
    } else {
        info!("Successfully added bucket");
    }

    internal_get_setup_auth_config(State(state), session, AuthConfigFailure::empty()).await
}

bitflags! {
    #[derive(Eq, PartialEq)]
    struct AuthConfigFailure: u8 {
        const WL_OOR =     0b0000_0001;
        const PARSE_WL_L = 0b0000_0010;
        const PARSE_WL_U = 0b0000_0100;

        const NR_OOR =     0b0001_0000;
        const PARSE_NR_L = 0b0010_0000;
        const PARSE_NR_U = 0b0100_0000;
    }
}

impl AuthConfigFailure {
    pub fn as_nice_list(&self) -> impl Iterator<Item = &'static str> {
        self.iter().filter_map(|e| match e {
            Self::PARSE_WL_L => Some("Word Length - Lower Bound: Parse Error"),
            Self::PARSE_WL_U => Some("Word Length - Upper Bound: Parse Error"),
            Self::PARSE_NR_L => Some("Number Range - Lower Bound: Parse Error"),
            Self::PARSE_NR_U => Some("Number Range - Upper Bound: Parse Error"),
            Self::WL_OOR => Some("Word Length: Invalid Range"),
            Self::NR_OOR => Some("Number Range: Invalid Range"),
            _ => None,
        })
    }
}

async fn internal_get_setup_auth_config(
    State(state): State<DenimState>,
    session: DenimSession,
    failure: AuthConfigFailure,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::RUN_ONBOARDING)?;

    let auth_config = state.config().auth_config().await.clone();

    let [
        wordlen_lower,
        worldlen_upper,
        numberrange_lower,
        numberrange_upper,
    ] = [
        auth_config.word_len_range.start,
        auth_config.word_len_range.end,
        auth_config.numbers_range.start,
        auth_config.numbers_range.end,
    ];

    let ranged_number_input = |id: &str,
                               text: &str,
                               current: usize,
                               lower_bound: usize,
                               upper_bound: usize| {
        form_element(
            id,
            text,
            html! {
                input value=(current) required type="number" id=(id) name=(id) min=(lower_bound) max=(upper_bound) class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
            },
        )
    };

    Ok(html! {
        (title("Setup Auth Config"))
        p {"Now that S3's done, we can get the auth config setup - this is how the default passwords are generated"}
        br;
        p {
            "Passwords are generated in the following format: "
            span class="italic" {"word_number"}
            ". The length of the word is controlled by the range and picked randomly, and the number is picked randomly from within a different range."
        }

        @if !failure.is_empty() {
            br;
            (errors_list(Some("Validation Errors"), failure.as_nice_list()))
        }

        br;
        form hx-post="/internal/onboarding/setup_auth_config" hx-target="#current_section" {
            (ranged_number_input("wordlen_lower", "Word Length - Lower (1 - 32)", wordlen_lower, 1, 32))
            (ranged_number_input("wordlen_upper", "Word Length - Upper (1 - 32)", worldlen_upper, 1, 32))
            (ranged_number_input("numberrange_lower", "Word Length - Lower (0 - 1,000,000,000)", numberrange_lower, 0, 1_000_000_000))
            (ranged_number_input("numberrange_upper", "Word Length - Upper (0 - 1,000,000,000)", numberrange_upper, 0, 1_000_000_000))

            (form_submit_button(Some("Submit Ranges for Passwords")))
        }
    })
}

#[derive(Deserialize)]
pub struct AuthConfigForm {
    wordlen_lower: String,
    wordlen_upper: String,
    numberrange_lower: String,
    numberrange_upper: String,
}

pub async fn internal_post_setup_auth_config(
    State(state): State<DenimState>,
    session: DenimSession,
    Form(input): Form<AuthConfigForm>,
) -> DenimResult<Markup> {
    session.ensure_can(PermissionsTarget::RUN_ONBOARDING)?;

    let mut errors = AuthConfigFailure::empty();
    let mut current_config = state.config().auth_config().await.clone();

    {
        let lower = match input.wordlen_lower.parse() {
            Ok(x) => x,
            Err(_e) => {
                errors |= AuthConfigFailure::PARSE_WL_L;
                0
            }
        };
        let upper = match input.wordlen_upper.parse() {
            Ok(x) => x,
            Err(_e) => {
                errors |= AuthConfigFailure::PARSE_WL_U;
                0
            }
        };

        if lower > upper || lower == 0 || upper == 0 || upper > 32 {
            errors |= AuthConfigFailure::WL_OOR;
        } else {
            current_config.word_len_range = lower..upper;
        }
    }
    {
        let lower = match input.numberrange_lower.parse() {
            Ok(x) => x,
            Err(_e) => {
                errors |= AuthConfigFailure::PARSE_NR_L;
                0
            }
        };
        let upper = match input.numberrange_upper.parse() {
            Ok(x) => x,
            Err(_e) => {
                errors |= AuthConfigFailure::PARSE_NR_U;
                0
            }
        };

        if lower > upper || lower == 0 || upper == 0 || upper > 1_000_000_000 {
            errors |= AuthConfigFailure::WL_OOR;
        } else {
            current_config.word_len_range = lower..upper;
        }
    }

    if !errors.is_empty() {
        return internal_get_setup_auth_config(State(state), session, errors).await;
    }

    state.config().set_auth_config(current_config).await;

    Ok(html! {
        (title("All finished with Onboarding!"))
        br;
        p {
            "Enjoy using "
            span class="font-bold" {"Denim"}
            "!"
        }
        br;
        p {
            "A common next step is to "
            a class="text-blue-300 underline" href="/import_export" {"import events and students"}
            "."
        }
    })
}
