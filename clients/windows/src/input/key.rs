use lbeguiapp::WgpuLockbook;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::clipboard_paste;
use super::message::MessageAppDep;

struct Key {
    vk: VIRTUAL_KEY,
    egui: Option<egui::Key>,
    text: Option<&'static str>,
    s_text: Option<&'static str>, // shift text
}

const KEYS: [Key; 74] = [
    Key { vk: VK_A, egui: Some(egui::Key::A), text: Some("a"), s_text: Some("A") },
    Key { vk: VK_B, egui: Some(egui::Key::B), text: Some("b"), s_text: Some("B") },
    Key { vk: VK_C, egui: Some(egui::Key::C), text: Some("c"), s_text: Some("C") },
    Key { vk: VK_D, egui: Some(egui::Key::D), text: Some("d"), s_text: Some("D") },
    Key { vk: VK_E, egui: Some(egui::Key::E), text: Some("e"), s_text: Some("E") },
    Key { vk: VK_F, egui: Some(egui::Key::F), text: Some("f"), s_text: Some("F") },
    Key { vk: VK_G, egui: Some(egui::Key::G), text: Some("g"), s_text: Some("G") },
    Key { vk: VK_H, egui: Some(egui::Key::H), text: Some("h"), s_text: Some("H") },
    Key { vk: VK_I, egui: Some(egui::Key::I), text: Some("i"), s_text: Some("I") },
    Key { vk: VK_J, egui: Some(egui::Key::J), text: Some("j"), s_text: Some("J") },
    Key { vk: VK_K, egui: Some(egui::Key::K), text: Some("k"), s_text: Some("K") },
    Key { vk: VK_L, egui: Some(egui::Key::L), text: Some("l"), s_text: Some("L") },
    Key { vk: VK_M, egui: Some(egui::Key::M), text: Some("m"), s_text: Some("M") },
    Key { vk: VK_N, egui: Some(egui::Key::N), text: Some("n"), s_text: Some("N") },
    Key { vk: VK_O, egui: Some(egui::Key::O), text: Some("o"), s_text: Some("O") },
    Key { vk: VK_P, egui: Some(egui::Key::P), text: Some("p"), s_text: Some("P") },
    Key { vk: VK_Q, egui: Some(egui::Key::Q), text: Some("q"), s_text: Some("Q") },
    Key { vk: VK_R, egui: Some(egui::Key::R), text: Some("r"), s_text: Some("R") },
    Key { vk: VK_S, egui: Some(egui::Key::S), text: Some("s"), s_text: Some("S") },
    Key { vk: VK_T, egui: Some(egui::Key::T), text: Some("t"), s_text: Some("T") },
    Key { vk: VK_U, egui: Some(egui::Key::U), text: Some("u"), s_text: Some("U") },
    Key { vk: VK_V, egui: Some(egui::Key::V), text: Some("v"), s_text: Some("V") },
    Key { vk: VK_W, egui: Some(egui::Key::W), text: Some("w"), s_text: Some("W") },
    Key { vk: VK_X, egui: Some(egui::Key::X), text: Some("x"), s_text: Some("X") },
    Key { vk: VK_Y, egui: Some(egui::Key::Y), text: Some("y"), s_text: Some("Y") },
    Key { vk: VK_Z, egui: Some(egui::Key::Z), text: Some("z"), s_text: Some("Z") },
    Key { vk: VK_0, egui: Some(egui::Key::Num0), text: Some("0"), s_text: Some(")") },
    Key { vk: VK_1, egui: Some(egui::Key::Num1), text: Some("1"), s_text: Some("!") },
    Key { vk: VK_2, egui: Some(egui::Key::Num2), text: Some("2"), s_text: Some("@") },
    Key { vk: VK_3, egui: Some(egui::Key::Num3), text: Some("3"), s_text: Some("#") },
    Key { vk: VK_4, egui: Some(egui::Key::Num4), text: Some("4"), s_text: Some("$") },
    Key { vk: VK_5, egui: Some(egui::Key::Num5), text: Some("5"), s_text: Some("%") },
    Key { vk: VK_6, egui: Some(egui::Key::Num6), text: Some("6"), s_text: Some("^") },
    Key { vk: VK_7, egui: Some(egui::Key::Num7), text: Some("7"), s_text: Some("&") },
    Key { vk: VK_8, egui: Some(egui::Key::Num8), text: Some("8"), s_text: Some("*") },
    Key { vk: VK_9, egui: Some(egui::Key::Num9), text: Some("9"), s_text: Some("(") },
    Key { vk: VK_OEM_1, egui: Some(egui::Key::Semicolon), text: Some(";"), s_text: Some(":") },
    Key { vk: VK_OEM_PLUS, egui: Some(egui::Key::Equals), text: Some("="), s_text: Some("+") },
    Key { vk: VK_OEM_COMMA, egui: Some(egui::Key::Comma), text: Some(","), s_text: Some("<") },
    Key { vk: VK_OEM_MINUS, egui: Some(egui::Key::Minus), text: Some("-"), s_text: Some("_") },
    Key { vk: VK_OEM_PERIOD, egui: Some(egui::Key::Period), text: Some("."), s_text: Some(">") },
    Key { vk: VK_OEM_2, egui: Some(egui::Key::Slash), text: Some("/"), s_text: Some("?") },
    Key { vk: VK_OEM_3, egui: Some(egui::Key::Backtick), text: Some("`"), s_text: Some("~") },
    Key { vk: VK_OEM_4, egui: Some(egui::Key::OpenBracket), text: Some("["), s_text: Some("{") },
    Key { vk: VK_OEM_5, egui: Some(egui::Key::Backslash), text: Some("\\"), s_text: Some("|") },
    Key { vk: VK_OEM_6, egui: Some(egui::Key::CloseBracket), text: Some("]"), s_text: Some("}") },
    Key { vk: VK_OEM_7, egui: None, text: Some("'"), s_text: Some("\"") },
    Key { vk: VK_SPACE, egui: Some(egui::Key::Space), text: Some(" "), s_text: Some(" ") },
    Key { vk: VK_ESCAPE, egui: Some(egui::Key::Escape), text: None, s_text: None },
    Key { vk: VK_RETURN, egui: Some(egui::Key::Enter), text: None, s_text: None },
    Key { vk: VK_TAB, egui: Some(egui::Key::Tab), text: None, s_text: None },
    Key { vk: VK_LEFT, egui: Some(egui::Key::ArrowLeft), text: None, s_text: None },
    Key { vk: VK_RIGHT, egui: Some(egui::Key::ArrowRight), text: None, s_text: None },
    Key { vk: VK_UP, egui: Some(egui::Key::ArrowUp), text: None, s_text: None },
    Key { vk: VK_DOWN, egui: Some(egui::Key::ArrowDown), text: None, s_text: None },
    Key { vk: VK_DELETE, egui: Some(egui::Key::Delete), text: None, s_text: None },
    Key { vk: VK_BACK, egui: Some(egui::Key::Backspace), text: None, s_text: None },
    Key { vk: VK_F1, egui: Some(egui::Key::F1), text: None, s_text: None },
    Key { vk: VK_F2, egui: Some(egui::Key::F2), text: None, s_text: None },
    Key { vk: VK_F3, egui: Some(egui::Key::F3), text: None, s_text: None },
    Key { vk: VK_F4, egui: Some(egui::Key::F4), text: None, s_text: None },
    Key { vk: VK_F5, egui: Some(egui::Key::F5), text: None, s_text: None },
    Key { vk: VK_F6, egui: Some(egui::Key::F6), text: None, s_text: None },
    Key { vk: VK_F7, egui: Some(egui::Key::F7), text: None, s_text: None },
    Key { vk: VK_F8, egui: Some(egui::Key::F8), text: None, s_text: None },
    Key { vk: VK_F9, egui: Some(egui::Key::F9), text: None, s_text: None },
    Key { vk: VK_F10, egui: Some(egui::Key::F10), text: None, s_text: None },
    Key { vk: VK_F11, egui: Some(egui::Key::F11), text: None, s_text: None },
    Key { vk: VK_F12, egui: Some(egui::Key::F12), text: None, s_text: None },
    Key { vk: VK_HOME, egui: Some(egui::Key::Home), text: None, s_text: None },
    Key { vk: VK_END, egui: Some(egui::Key::End), text: None, s_text: None },
    Key { vk: VK_PRIOR, egui: Some(egui::Key::PageUp), text: None, s_text: None },
    Key { vk: VK_NEXT, egui: Some(egui::Key::PageDown), text: None, s_text: None },
    Key { vk: VK_INSERT, egui: Some(egui::Key::Insert), text: None, s_text: None },
];

pub fn handle(
    app: &mut WgpuLockbook, message: MessageAppDep, key: VIRTUAL_KEY, modifiers: egui::Modifiers,
) -> bool {
    let pressed = matches!(message, MessageAppDep::KeyDown { .. });
    let mut consumed = false;

    // text
    if pressed && (modifiers.shift_only() || modifiers.is_none()) {
        if let Some(text) = key_text(key, modifiers.shift) {
            app.raw_input
                .events
                .push(egui::Event::Text(text.to_owned()));
            consumed = true;
        }
    }

    // todo: something feels weird about this
    if let Some(key) = egui_key(key) {
        if pressed && key == egui::Key::X && modifiers.command {
            app.raw_input.events.push(egui::Event::Cut);
        } else if pressed && key == egui::Key::C && modifiers.command {
            app.raw_input.events.push(egui::Event::Copy);
        } else if pressed && key == egui::Key::V && modifiers.command {
            clipboard_paste::handle(app);
        } else {
            // other egui keys
            app.raw_input.events.push(egui::Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers,
                physical_key: None,
            });
        }
        consumed = true;
    }

    consumed
}

pub fn key_text(vk: VIRTUAL_KEY, shift: bool) -> Option<&'static str> {
    KEYS.iter()
        .find(|key| key.vk == vk)
        .and_then(|key| if shift { key.s_text } else { key.text })
}

pub fn egui_key(vk: VIRTUAL_KEY) -> Option<egui::Key> {
    KEYS.iter()
        .find(|key| key.vk == vk)
        .and_then(|key| key.egui)
}
