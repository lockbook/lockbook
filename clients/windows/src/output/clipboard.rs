use std::mem;

use lbeguiapp::WgpuLockbook;

pub fn handle(app: &mut WgpuLockbook) {
    if let Some(copied_text) = mem::take(&mut app.from_egui) {
        clipboard_win::set_clipboard(clipboard_win::formats::Unicode, copied_text)
            .expect("set clipboard");
    }
}
