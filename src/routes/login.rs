use crate::{
    auth::{DenimSession, backend::DenimAuthCredentials},
    error::DenimResult,
    state::DenimState,
};
use axum::{
    Form,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use maud::{Markup, html};
use secrecy::SecretString;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginOptions {
    pub to: Option<String>,
    pub login_failed: Option<bool>,
}

pub async fn get_login(
    State(state): State<DenimState>,
    session: DenimSession,
    Query(LoginOptions { to, login_failed }): Query<LoginOptions>,
) -> DenimResult<Markup> {
    let is_logged_in = session.user.is_some();
    let login_failed = login_failed.unwrap_or(false);

    Ok(state.render(session, html! {
        div class="bg-gray-800 shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-sm" {
            @if login_failed {
                div role="alert" class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative" {
                    strong class="font-bold" {"Alert!"}
                    br;
                    span class="block sm:inline" {"Email/Password not found or password incorrect"}
                }
                br;
            }

            @if is_logged_in {
                h2 class="text-2xl font-semibold mb-6 text-gray-300 text-center" {
                    "Already logged in!"
                }
                form method="post" action="/logout" {
                    input type="submit" value="Logout?" class="bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {}
                }
            } @else {
                h2 class="text-2xl font-semibold mb-6 text-gray-300 text-center" {
                    "Login"
                }
                form method="post" {
                    @if let Some(to) = to {
                        input type="hidden" name="next" value=(to) {} 
                    }
                    div class="mb-4" {
                        label for="email" class="block text-sm font-bold mb-2 text-gray-300" {"Email Address:"}
                        input id="email" name="email" type="email" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
                    }
                    div class="mb-4" {
                        label for="password" class="block text-sm font-bold mb-2 text-gray-300" {"Password:"}
                        input id="password" name="password" type="password" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
                    }
                    div class="flex justify-between items-center" {
                        input type="submit" value="Login" class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {}
                    }
                }
            }
        }
    }))
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
                let next = next.as_deref().unwrap_or("/");
                Ok(Redirect::to(next))
            }
            Err(e) => Err(e.into()),
        },
        Ok(None) => {
            //TODO: if default password, send to change default password page :)
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
