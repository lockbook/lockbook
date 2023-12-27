pub fn handle(copied_text: String) {
    if !copied_text.is_empty() {
        clipboard_win::set_clipboard(clipboard_win::formats::Unicode, copied_text)
            .expect("set clipboard");
    }
}
