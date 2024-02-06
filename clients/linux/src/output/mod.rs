pub mod clipboard_copy;
pub mod cursor;
pub mod open_url;
pub mod window_title;

pub fn close() {
    // todo: save open tabs etc
    std::process::exit(0);
}
