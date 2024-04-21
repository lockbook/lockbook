use clipboard_win::SysResult;

pub fn handle(copied_text: String) -> SysResult<()> {
    if !copied_text.is_empty() {
        clipboard_win::set_clipboard(clipboard_win::formats::Unicode, copied_text)
    } else {
        Ok(())
    }
}
