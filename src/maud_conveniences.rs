use email_address::EmailAddress;
use jiff::tz::{TimeZone, TimeZoneName, db};
use maud::{Escaper, Markup, PreEscaped, Render, html};
use std::fmt::Write;

#[inline]
#[allow(clippy::needless_pass_by_value)]
pub fn table<const N: usize>(
    overall_title: Markup,
    titles: [&str; N],
    items: Vec<[impl Render; N]>,
) -> Markup {
    html! {
        div class="container mx-auto" {
            (overall_title)
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

#[inline]
#[allow(dead_code)]
pub fn escape(s: impl AsRef<str>) -> PreEscaped<String> {
    let mut output = String::new();
    Escaper::new(&mut output).write_str(s.as_ref()).unwrap(); //this method always succeeds - strange api!
    PreEscaped(output)
}

#[inline]
pub fn supertitle(s: impl Render) -> Markup {
    html! {
        h1 class="text-3xl font-semibold mb-4" {(s)}
    }
}

#[inline]
pub fn title(s: impl Render) -> Markup {
    html! {
        h1 class="text-2xl font-semibold mb-4" {(s)}
    }
}

#[inline]
pub fn subtitle(s: impl Render) -> Markup {
    html! {
        h2 class="text-xl font-semibold mb-4" {(s)}
    }
}

#[inline]
pub fn subsubtitle(s: impl Render) -> Markup {
    html! {
        h3 class="text-lg font-semibold mb-4" {(s)}
    }
}

#[inline]
pub fn simple_form_element(
    id: impl Render + Clone,
    text: impl Render,
    required: bool,
    ty: Option<&str>,
    current: Option<&str>,
) -> Markup {
    form_element(
        id.clone(),
        text,
        html! {
            input value=[current] required[required] type=(ty.unwrap_or("text")) id=(id) name=(id) class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {}
        },
    )
}

#[inline]
#[allow(clippy::needless_pass_by_value)]
pub fn form_element(id: impl Render, text: impl Render, input_element: Markup) -> Markup {
    html! {
        div class="mb-4" {
            label for=(id) class="block text-sm font-bold mb-2 text-gray-300" {(text)}
            (input_element)
        }
    }
}

#[inline]
pub fn form_submit_button(txt: Option<&str>) -> Markup {
    html! {
        div class="flex justify-between items-center" {
            input type="submit" value=(txt.unwrap_or("Submit")) class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" {}
        }
    }
}

#[inline]
pub fn errors_list(title: Option<&'static str>, list: impl Iterator<Item = impl Render>) -> Markup {
    let title = title.unwrap_or("Errors:");

    html! {
        div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative mb-4" role="alert" {
            strong class="font-bold" {(title)}
            ul class="list-disc pl-5 overflow-y-clip overflow-y-scroll max-h-64" {
                @for error in list {
                    li {(error)}
                }
            }
        }
    }
}

pub fn timezone_picker(current: Option<TimeZone>) -> Markup {
    let current = current.map_or_else(
        || match TimeZone::try_system() {
            Ok(tz) => Some(tz),
            Err(e) => {
                warn!(?e, "Failed to get system timezone");
                None
            }
        },
        Some,
    );

    let current_is_system = |test: &TimeZoneName| {
        let Ok(actual_tz) = TimeZone::get(test.as_str()) else {
            return false;
        };
        current.as_ref().is_none_or(|sys_tz| &actual_tz == sys_tz)
    };

    form_element(
        "tz",
        "Timezone",
        html! {
            select required id="tz" name="tz" class="shadow appearance-none border rounded w-full py-2 px-3 leading-tight focus:outline-none focus:shadow-outline bg-gray-700 border-gray-600" {
                @for tz in db().available() {
                    @let selected = current_is_system(&tz);
                    option value={(tz)} selected[selected] {(tz)}
                }
            }
        },
    )
}

pub struct Email<'a>(pub &'a EmailAddress);
impl Render for Email<'_> {
    fn render(&self) -> Markup {
        html! {
            a href={"mailto:" (self.0)} class="text-blue-500" {(self.0)}
        }
    }
}
