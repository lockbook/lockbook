use crate::input;
use egui::Modifiers;
use lbeguiapp::WgpuLockbook;
use x11rb::protocol::xproto::KeyButMask;
use x11rb::xcb_ffi::XCBConnection;
use xkbcommon::xkb::{self, Keycode, x11};

pub struct Keyboard {
    state: xkb::State,
}

impl Keyboard {
    pub fn new(conn: &XCBConnection) -> Self {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let device_id = x11::get_core_keyboard_device_id(conn);
        let keymap =
            x11::keymap_new_from_device(&context, conn, device_id, xkb::KEYMAP_COMPILE_NO_FLAGS);
        let state = x11::state_new_from_device(&keymap, conn, device_id);

        Self { state }
    }

    pub fn handle(
        &mut self, detail: u8, _state: KeyButMask, pressed: bool, app: &mut WgpuLockbook,
        paste_context: &mut input::clipboard_paste::Context,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let keycode = detail.into();
        let keysym = self.state.key_get_one_sym(keycode);
        let utf8 = xkb::keysym_to_utf8(keysym);

        if pressed {
            self.state.update_key(keycode, xkb::KeyDirection::Down);
        } else {
            self.state.update_key(keycode, xkb::KeyDirection::Up);
        }

        let modifiers = Modifiers {
            alt: self
                .state
                .mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE),
            ctrl: self
                .state
                .mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE),
            shift: self
                .state
                .mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE),
            command: self
                .state
                .mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE),
            mac_cmd: false,
        };

        // todo: pressing a key then holding ctrl or alt while releasing may cause egui to think it is still pressed
        if (modifiers.is_none() || modifiers.shift_only()) && utf8.chars().any(|c| !c.is_control())
        {
            if pressed {
                app.raw_input.events.push(egui::Event::Text(utf8));
            }
        } else if let Some(key) = egui_key(keycode) {
            // todo: weird negotiation of keyboard shortcuts
            if pressed && key == egui::Key::X && modifiers.command_only() {
                app.raw_input.events.push(egui::Event::Cut);
            } else if pressed && key == egui::Key::C && modifiers.command_only() {
                app.raw_input.events.push(egui::Event::Copy);
            } else if pressed && key == egui::Key::V && modifiers.command_only() {
                paste_context.handle_paste()?;
            } else {
                app.raw_input.events.push(egui::Event::Key {
                    key,
                    pressed,
                    repeat: false,
                    modifiers,
                    physical_key: None,
                });
            }
        }

        Ok(())
    }
}

struct Key {
    scancode: u8,
    egui: Option<egui::Key>,
}

fn egui_key(keycode: Keycode) -> Option<egui::Key> {
    // "X11-style keycodes are offset by 8 from the keycodes the Linux kernel uses."
    // https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/common/keymap.rs#L7
    let scancode = u8::from(keycode).saturating_sub(8);

    KEYS.iter()
        .find(|key| key.scancode == scancode)
        .and_then(|key| key.egui)
}

// written with reference to winit:
// https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/common/keymap.rs#L15
const KEYS: [Key; 89] = [
    Key { scancode: 30, egui: Some(egui::Key::A) },
    Key { scancode: 48, egui: Some(egui::Key::B) },
    Key { scancode: 46, egui: Some(egui::Key::C) },
    Key { scancode: 32, egui: Some(egui::Key::D) },
    Key { scancode: 18, egui: Some(egui::Key::E) },
    Key { scancode: 33, egui: Some(egui::Key::F) },
    Key { scancode: 34, egui: Some(egui::Key::G) },
    Key { scancode: 35, egui: Some(egui::Key::H) },
    Key { scancode: 23, egui: Some(egui::Key::I) },
    Key { scancode: 36, egui: Some(egui::Key::J) },
    Key { scancode: 37, egui: Some(egui::Key::K) },
    Key { scancode: 38, egui: Some(egui::Key::L) },
    Key { scancode: 50, egui: Some(egui::Key::M) },
    Key { scancode: 49, egui: Some(egui::Key::N) },
    Key { scancode: 24, egui: Some(egui::Key::O) },
    Key { scancode: 25, egui: Some(egui::Key::P) },
    Key { scancode: 16, egui: Some(egui::Key::Q) },
    Key { scancode: 19, egui: Some(egui::Key::R) },
    Key { scancode: 31, egui: Some(egui::Key::S) },
    Key { scancode: 20, egui: Some(egui::Key::T) },
    Key { scancode: 22, egui: Some(egui::Key::U) },
    Key { scancode: 47, egui: Some(egui::Key::V) },
    Key { scancode: 17, egui: Some(egui::Key::W) },
    Key { scancode: 45, egui: Some(egui::Key::X) },
    Key { scancode: 21, egui: Some(egui::Key::Y) },
    Key { scancode: 44, egui: Some(egui::Key::Z) },
    Key { scancode: 11, egui: Some(egui::Key::Num0) },
    Key { scancode: 2, egui: Some(egui::Key::Num1) },
    Key { scancode: 3, egui: Some(egui::Key::Num2) },
    Key { scancode: 4, egui: Some(egui::Key::Num3) },
    Key { scancode: 5, egui: Some(egui::Key::Num4) },
    Key { scancode: 6, egui: Some(egui::Key::Num5) },
    Key { scancode: 7, egui: Some(egui::Key::Num6) },
    Key { scancode: 8, egui: Some(egui::Key::Num7) },
    Key { scancode: 9, egui: Some(egui::Key::Num8) },
    Key { scancode: 10, egui: Some(egui::Key::Num9) },
    Key { scancode: 1, egui: Some(egui::Key::Escape) },
    Key { scancode: 12, egui: Some(egui::Key::Minus) },
    Key { scancode: 13, egui: Some(egui::Key::Equals) },
    Key { scancode: 14, egui: Some(egui::Key::Backspace) },
    Key { scancode: 15, egui: Some(egui::Key::Tab) },
    Key { scancode: 26, egui: Some(egui::Key::OpenBracket) },
    Key { scancode: 27, egui: Some(egui::Key::CloseBracket) },
    Key { scancode: 28, egui: Some(egui::Key::Enter) },
    Key { scancode: 39, egui: Some(egui::Key::Semicolon) },
    Key { scancode: 40, egui: None },
    Key { scancode: 41, egui: Some(egui::Key::Backtick) },
    Key { scancode: 43, egui: Some(egui::Key::Backslash) },
    Key { scancode: 51, egui: Some(egui::Key::Comma) },
    Key { scancode: 52, egui: Some(egui::Key::Period) },
    Key { scancode: 53, egui: Some(egui::Key::Slash) },
    Key { scancode: 55, egui: None },
    Key { scancode: 57, egui: Some(egui::Key::Space) },
    Key { scancode: 59, egui: Some(egui::Key::F1) },
    Key { scancode: 60, egui: Some(egui::Key::F2) },
    Key { scancode: 61, egui: Some(egui::Key::F3) },
    Key { scancode: 62, egui: Some(egui::Key::F4) },
    Key { scancode: 63, egui: Some(egui::Key::F5) },
    Key { scancode: 64, egui: Some(egui::Key::F6) },
    Key { scancode: 65, egui: Some(egui::Key::F7) },
    Key { scancode: 66, egui: Some(egui::Key::F8) },
    Key { scancode: 67, egui: Some(egui::Key::F9) },
    Key { scancode: 68, egui: Some(egui::Key::F10) },
    Key { scancode: 71, egui: Some(egui::Key::Num7) },
    Key { scancode: 72, egui: Some(egui::Key::Num8) },
    Key { scancode: 73, egui: Some(egui::Key::Num9) },
    Key { scancode: 74, egui: Some(egui::Key::Minus) },
    Key { scancode: 75, egui: Some(egui::Key::Num4) },
    Key { scancode: 76, egui: Some(egui::Key::Num5) },
    Key { scancode: 77, egui: Some(egui::Key::Num6) },
    Key { scancode: 78, egui: Some(egui::Key::Plus) },
    Key { scancode: 79, egui: Some(egui::Key::Num1) },
    Key { scancode: 80, egui: Some(egui::Key::Num2) },
    Key { scancode: 81, egui: Some(egui::Key::Num3) },
    Key { scancode: 82, egui: Some(egui::Key::Num0) },
    Key { scancode: 83, egui: Some(egui::Key::Period) },
    Key { scancode: 87, egui: Some(egui::Key::F11) },
    Key { scancode: 88, egui: Some(egui::Key::F12) },
    Key { scancode: 96, egui: Some(egui::Key::Enter) },
    Key { scancode: 102, egui: Some(egui::Key::Home) },
    Key { scancode: 103, egui: Some(egui::Key::ArrowUp) },
    Key { scancode: 104, egui: Some(egui::Key::PageUp) },
    Key { scancode: 105, egui: Some(egui::Key::ArrowLeft) },
    Key { scancode: 106, egui: Some(egui::Key::ArrowRight) },
    Key { scancode: 107, egui: Some(egui::Key::End) },
    Key { scancode: 108, egui: Some(egui::Key::ArrowDown) },
    Key { scancode: 109, egui: Some(egui::Key::PageDown) },
    Key { scancode: 110, egui: Some(egui::Key::Insert) },
    Key { scancode: 111, egui: Some(egui::Key::Delete) },
];
