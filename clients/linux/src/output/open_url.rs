use egui::output::OpenUrl;

pub fn handle(open_urls: Vec<OpenUrl>) {
    for open_url in open_urls {
        let _ = open::that(open_url.url);
    }
}
