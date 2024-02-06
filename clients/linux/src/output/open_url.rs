use egui::output::OpenUrl;

pub fn handle(open_url: Option<OpenUrl>) {
    if let Some(open_url) = open_url {
        open::that(open_url.url).unwrap();
    }
}
