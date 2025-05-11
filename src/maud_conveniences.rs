use maud::{Escaper, Markup, PreEscaped, Render, html};
use std::fmt::Write;

#[inline]
pub fn table<const N: usize>(
    overall_title: &str,
    titles: [&str; N],
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

#[inline]
pub fn escape(s: impl AsRef<str>) -> PreEscaped<String> {
    let mut output = String::new();
    Escaper::new(&mut output).write_str(s.as_ref()).unwrap(); //this method always succeeds - strange api!
    PreEscaped(output)
}

#[inline]
pub fn title(s: impl Render) -> Markup {
    html! {
        h1 class="text-2xl font-semibold mb-4" {(s)}
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
pub fn errors_list(list: impl Iterator<Item = impl Render>) -> Markup {
    html! {
        div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative mb-4" role="alert" {
            strong class="font-bold" {"Errors:"}
            ul class="list-disc pl-5" {
                @for error in list {
                    li {(error)}
                }
            }
        }
    }
}

pub struct Email<'a>(pub &'a str);
impl Render for Email<'_> {
    fn render(&self) -> Markup {
        html!{
            a href={"mailto:" (self.0)} class="text-blue-500" {(self.0)}
        }
    }
}