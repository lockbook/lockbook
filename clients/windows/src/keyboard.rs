use windows::Win32::UI::Input::KeyboardAndMouse::*;

struct Key {
    vk: VIRTUAL_KEY,
    egui: Option<egui::Key>,
    text: Option<&'static str>,
    shift_text: Option<&'static str>,
}

const KEYS: [Key; 74] = [
    Key { vk: VK_A, egui: Some(egui::Key::A), text: Some("a"), shift_text: Some("A") },
    Key { vk: VK_B, egui: Some(egui::Key::B), text: Some("b"), shift_text: Some("B") },
    Key { vk: VK_C, egui: Some(egui::Key::C), text: Some("c"), shift_text: Some("C") },
    Key { vk: VK_D, egui: Some(egui::Key::D), text: Some("d"), shift_text: Some("D") },
    Key { vk: VK_E, egui: Some(egui::Key::E), text: Some("e"), shift_text: Some("E") },
    Key { vk: VK_F, egui: Some(egui::Key::F), text: Some("f"), shift_text: Some("F") },
    Key { vk: VK_G, egui: Some(egui::Key::G), text: Some("g"), shift_text: Some("G") },
    Key { vk: VK_H, egui: Some(egui::Key::H), text: Some("h"), shift_text: Some("H") },
    Key { vk: VK_I, egui: Some(egui::Key::I), text: Some("i"), shift_text: Some("I") },
    Key { vk: VK_J, egui: Some(egui::Key::J), text: Some("j"), shift_text: Some("J") },
    Key { vk: VK_K, egui: Some(egui::Key::K), text: Some("k"), shift_text: Some("K") },
    Key { vk: VK_L, egui: Some(egui::Key::L), text: Some("l"), shift_text: Some("L") },
    Key { vk: VK_M, egui: Some(egui::Key::M), text: Some("m"), shift_text: Some("M") },
    Key { vk: VK_N, egui: Some(egui::Key::N), text: Some("n"), shift_text: Some("N") },
    Key { vk: VK_O, egui: Some(egui::Key::O), text: Some("o"), shift_text: Some("O") },
    Key { vk: VK_P, egui: Some(egui::Key::P), text: Some("p"), shift_text: Some("P") },
    Key { vk: VK_Q, egui: Some(egui::Key::Q), text: Some("q"), shift_text: Some("Q") },
    Key { vk: VK_R, egui: Some(egui::Key::R), text: Some("r"), shift_text: Some("R") },
    Key { vk: VK_S, egui: Some(egui::Key::S), text: Some("s"), shift_text: Some("S") },
    Key { vk: VK_T, egui: Some(egui::Key::T), text: Some("t"), shift_text: Some("T") },
    Key { vk: VK_U, egui: Some(egui::Key::U), text: Some("u"), shift_text: Some("U") },
    Key { vk: VK_V, egui: Some(egui::Key::V), text: Some("v"), shift_text: Some("V") },
    Key { vk: VK_W, egui: Some(egui::Key::W), text: Some("w"), shift_text: Some("W") },
    Key { vk: VK_X, egui: Some(egui::Key::X), text: Some("x"), shift_text: Some("X") },
    Key { vk: VK_Y, egui: Some(egui::Key::Y), text: Some("y"), shift_text: Some("Y") },
    Key { vk: VK_Z, egui: Some(egui::Key::Z), text: Some("z"), shift_text: Some("Z") },
    Key { vk: VK_0, egui: Some(egui::Key::Num0), text: Some("0"), shift_text: Some(")") },
    Key { vk: VK_1, egui: Some(egui::Key::Num1), text: Some("1"), shift_text: Some("!") },
    Key { vk: VK_2, egui: Some(egui::Key::Num2), text: Some("2"), shift_text: Some("@") },
    Key { vk: VK_3, egui: Some(egui::Key::Num3), text: Some("3"), shift_text: Some("#") },
    Key { vk: VK_4, egui: Some(egui::Key::Num4), text: Some("4"), shift_text: Some("$") },
    Key { vk: VK_5, egui: Some(egui::Key::Num5), text: Some("5"), shift_text: Some("%") },
    Key { vk: VK_6, egui: Some(egui::Key::Num6), text: Some("6"), shift_text: Some("^") },
    Key { vk: VK_7, egui: Some(egui::Key::Num7), text: Some("7"), shift_text: Some("&") },
    Key { vk: VK_8, egui: Some(egui::Key::Num8), text: Some("8"), shift_text: Some("*") },
    Key { vk: VK_9, egui: Some(egui::Key::Num9), text: Some("9"), shift_text: Some("(") },
    Key { vk: VK_OEM_1, egui: None, text: Some(";"), shift_text: Some(":") },
    Key { vk: VK_OEM_PLUS, egui: None, text: Some("="), shift_text: Some("+") },
    Key { vk: VK_OEM_COMMA, egui: None, text: Some(","), shift_text: Some("<") },
    Key { vk: VK_OEM_MINUS, egui: Some(egui::Key::Minus), text: Some("-"), shift_text: Some("_") },
    Key { vk: VK_OEM_PERIOD, egui: None, text: Some("."), shift_text: Some(">") },
    Key { vk: VK_OEM_2, egui: None, text: Some("/"), shift_text: Some("?") },
    Key { vk: VK_OEM_3, egui: None, text: Some("`"), shift_text: Some("~") },
    Key { vk: VK_OEM_4, egui: None, text: Some("["), shift_text: Some("{") },
    Key { vk: VK_OEM_5, egui: None, text: Some("\\"), shift_text: Some("|") },
    Key { vk: VK_OEM_6, egui: None, text: Some("]"), shift_text: Some("}") },
    Key { vk: VK_OEM_7, egui: None, text: Some("'"), shift_text: Some("\"") },
    Key { vk: VK_SPACE, egui: Some(egui::Key::Space), text: Some(" "), shift_text: Some(" ") },
    Key { vk: VK_ESCAPE, egui: Some(egui::Key::Escape), text: None, shift_text: None },
    Key { vk: VK_RETURN, egui: Some(egui::Key::Enter), text: None, shift_text: None },
    Key { vk: VK_TAB, egui: Some(egui::Key::Tab), text: None, shift_text: None },
    Key { vk: VK_LEFT, egui: Some(egui::Key::ArrowLeft), text: None, shift_text: None },
    Key { vk: VK_RIGHT, egui: Some(egui::Key::ArrowRight), text: None, shift_text: None },
    Key { vk: VK_UP, egui: Some(egui::Key::ArrowUp), text: None, shift_text: None },
    Key { vk: VK_DOWN, egui: Some(egui::Key::ArrowDown), text: None, shift_text: None },
    Key { vk: VK_DELETE, egui: Some(egui::Key::Delete), text: None, shift_text: None },
    Key { vk: VK_BACK, egui: Some(egui::Key::Backspace), text: None, shift_text: None },
    Key { vk: VK_F1, egui: Some(egui::Key::F1), text: None, shift_text: None },
    Key { vk: VK_F2, egui: Some(egui::Key::F2), text: None, shift_text: None },
    Key { vk: VK_F3, egui: Some(egui::Key::F3), text: None, shift_text: None },
    Key { vk: VK_F4, egui: Some(egui::Key::F4), text: None, shift_text: None },
    Key { vk: VK_F5, egui: Some(egui::Key::F5), text: None, shift_text: None },
    Key { vk: VK_F6, egui: Some(egui::Key::F6), text: None, shift_text: None },
    Key { vk: VK_F7, egui: Some(egui::Key::F7), text: None, shift_text: None },
    Key { vk: VK_F8, egui: Some(egui::Key::F8), text: None, shift_text: None },
    Key { vk: VK_F9, egui: Some(egui::Key::F9), text: None, shift_text: None },
    Key { vk: VK_F10, egui: Some(egui::Key::F10), text: None, shift_text: None },
    Key { vk: VK_F11, egui: Some(egui::Key::F11), text: None, shift_text: None },
    Key { vk: VK_F12, egui: Some(egui::Key::F12), text: None, shift_text: None },
    Key { vk: VK_HOME, egui: Some(egui::Key::Home), text: None, shift_text: None },
    Key { vk: VK_END, egui: Some(egui::Key::End), text: None, shift_text: None },
    Key { vk: VK_PRIOR, egui: Some(egui::Key::PageUp), text: None, shift_text: None },
    Key { vk: VK_NEXT, egui: Some(egui::Key::PageDown), text: None, shift_text: None },
    Key { vk: VK_INSERT, egui: Some(egui::Key::Insert), text: None, shift_text: None },
];

pub fn egui_key(vk: VIRTUAL_KEY) -> Option<egui::Key> {
    KEYS.iter()
        .find(|key| key.vk == vk)
        .map(|key| key.egui)
        .flatten()
}

pub fn key_text(vk: VIRTUAL_KEY, shift: bool) -> Option<&'static str> {
    KEYS.iter()
        .find(|key| key.vk == vk)
        .map(|key| if shift { key.shift_text } else { key.text })
        .flatten()
}
