use crate::{
    auth::{DenimSession, backend::DenimAuthCredentials},
    error::{DenimResult, MakeQuerySnafu},
    maud_conveniences::{form_submit_button, simple_form_element, title},
    state::DenimState,
};
use axum::{
    Form,
    body::Body,
    extract::{Query, State},
    http::Response,
    response::{IntoResponse, Redirect},
};
use maud::html;
use secrecy::SecretString;
use serde::Deserialize;
use snafu::ResultExt;

#[derive(Deserialize)]
pub struct LoginOptions {
    pub to: Option<String>,
    pub login_failed: Option<bool>,
}

pub async fn get_login(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(LoginOptions { to, login_failed }): Query<LoginOptions>,
) -> DenimResult<Response<Body>> {
    if !sqlx::query!("SELECT exists(SELECT 1 FROM public.users)")
        .fetch_one(&mut *state.get_connection().await?)
        .await
        .context(MakeQuerySnafu)?
        .exists
        .unwrap_or(false)
    {
        return Ok(Redirect::to("/onboarding/create_admin_acc").into_response());
    }

    if session.user.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let login_failed = login_failed.unwrap_or(false);

    Ok(state.render(session, html! {
        div class="bg-gray-800 shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-sm" {
            (title("Login"))
            @if login_failed {
                div role="alert" class="bg-red-100 border border-red-400 text-red-700 px-4 py-4 rounded relative" {
                    strong class="font-bold" {"Alert!"}
                    br;
                    // avoid giving extra details for security reasons :)
                    span class="block sm:inline" {"Email/Password not found or password incorrect"}
                }
                br;
            }

            form method="post" {
                @if let Some(to) = to {
                    input type="hidden" name="next" value=(to) {} 
                }
                (simple_form_element("email", "Email", true, Some("email"), None))
                (simple_form_element("password", "Password", true, Some("password"), None))
                (form_submit_button(Some("Login")))
            }
        }
    }).into_response())
}

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: SecretString,
    next: Option<String>,
}

pub async fn post_login(
    mut session: DenimSession,
    Form(LoginForm {
        email,
        password,
        next,
    }): Form<LoginForm>,
) -> DenimResult<Redirect> {
    match session
        .authenticate(DenimAuthCredentials::EmailPassword { email, password })
        .await
    {
        Err(e) => Err(e.into()),
        Ok(Some(user)) => match session.login(&user).await {
            Ok(()) => {
                let next = next.as_deref().unwrap_or("");
                Ok(if user.current_password_is_default {
                    Redirect::to(&format!("/replace_default_password?next={next}"))
                } else {
                    Redirect::to(next)
                })
            }
            Err(e) => Err(e.into()),
        },
        Ok(None) => {
            let mut redirect = "/login?login_failed=true".to_string();
            if let Some(next) = next {
                redirect += format!("&to={next}").as_str();
            }
            Ok(Redirect::to(redirect.as_ref()))
        }
    }
}

pub async fn post_logout(mut session: DenimSession) -> DenimResult<impl IntoResponse> {
    session.logout().await?;
    Ok(Redirect::to("/"))
}
