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
