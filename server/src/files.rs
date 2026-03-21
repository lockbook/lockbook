use askama::Template;

#[derive(Template)]
#[template(path = "redirect.html")]
struct PreviewTemplate<'a> {
    uuid: &'a str,
}

pub fn get_files_preview_html(uuid: &str) -> String {
    PreviewTemplate { uuid }.render().unwrap()
}
