use x11rb::protocol::xproto::KeyButMask;

pub mod clipboard_paste;
pub mod file_drop;
pub mod key;
pub mod pointer;

pub fn modifiers(mask: KeyButMask) -> egui::Modifiers {
    egui::Modifiers {
        alt: mask.contains(KeyButMask::MOD1),
        ctrl: mask.contains(KeyButMask::CONTROL),
        command: mask.contains(KeyButMask::CONTROL),
        shift: mask.contains(KeyButMask::SHIFT),
        mac_cmd: false,
    }
}
