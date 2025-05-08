use crate::data::user::User;
use maud::{Escaper, Markup, PreEscaped, Render, html};
use std::fmt::Write;

pub fn render_table<const N: usize>(
    overall_title: &'static str,
    titles: [&'static str; N],
    items: Vec<[Markup; N]>,
) -> Markup {
    html! {
        div class="container mx-auto" {
            (title(overall_title))
            div class="overflow-x-auto" {
                table class="min-w-full bg-gray-800 rounded shadow-md" {
                    thead class="bg-gray-700" {
                        tr {
                            @for title in titles {
                                th class="py-2 px-4 text-left font-semibold text-gray-300" {(title)}
                            }
                        }
                    }
                    tbody {
                        @for row in items {
                            tr {
                                @for col in row {
                                    td class="py-2 px-4 border-b border-gray-600 text-gray-200" {(col)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn render_nav(logged_in_user: Option<User>) -> Markup {
    html! {
        nav class="bg-gray-800 shadow fixed top-0 z-10 rounded-lg" id="nav" {
            div class="container mx-auto px-4" {
                @let height_class = if logged_in_user.is_some() {"h-24"} else {"h-16"};
                div class={"flex items-center justify-center space-x-4 " (height_class)} {
                    a href="/events" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"Events"}
                    a href="/people" class="text-gray-300 bg-slate-900 hover:bg-slate-700 px-3 py-2 rounded-md text-sm font-medium" {"People"}
                    a href="/" class="text-gray-300 bg-fuchsia-900 hover:bg-fuchsia-700 px-3 py-2 rounded-md text-md font-bold" {"Denim"}
                    @match logged_in_user {
                        Some(logged_in_user) => {
                            div class="flex flex-col space-y-2 text-center" {
                                a href="/profile" id="nav_username" class="text-gray-300 bg-green-900 hover:bg-green-700 px-3 py-2 rounded-md text-sm font-medium" {(logged_in_user)}
                                form method="post" action="/logout" {
                                    input type="submit" value="Logout" class="text-gray-300 bg-red-900 hover:bg-red-700 px-3 py-2 rounded-md text-sm font-medium" {}
                                }
                            }
                        },
                        None => {
                            a href="/login" class="text-gray-300 bg-green-900 hover:bg-green-700 px-3 py-2 rounded-md text-sm font-medium" {"Login"}
                        }
                    }
                }
            }
        }
    }
}

pub fn escape(s: impl AsRef<str>) -> PreEscaped<String> {
    let mut output = String::new();
    Escaper::new(&mut output).write_str(s.as_ref()).unwrap(); //this method always succeeds - strange api!
    PreEscaped(output)
}

pub fn title(s: impl Render) -> Markup {
    html! {
        h1 class="text-2xl font-semibold mb-4" {(s)}
    }
}
