use crate::input::{self, modifiers};
use lbeguiapp::WgpuLockbook;
use x11rb::{
    protocol::xproto::{GetKeyboardMappingReply, KeyButMask},
    xcb_ffi::XCBConnection,
};
use xkbcommon::xkb::{self, x11, Context, KEYMAP_COMPILE_NO_FLAGS};

struct Key {
    sc: u8,
    egui: Option<egui::Key>,
    text: Option<&'static str>,
    s_text: Option<&'static str>, // shift text
}

// written with reference to winit:
// https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/common/keymap.rs#L15
const KEYS: [Key; 89] = [
    Key { sc: 30, egui: Some(egui::Key::A), text: Some("a"), s_text: Some("A") },
    Key { sc: 48, egui: Some(egui::Key::B), text: Some("b"), s_text: Some("B") },
    Key { sc: 46, egui: Some(egui::Key::C), text: Some("c"), s_text: Some("C") },
    Key { sc: 32, egui: Some(egui::Key::D), text: Some("d"), s_text: Some("D") },
    Key { sc: 18, egui: Some(egui::Key::E), text: Some("e"), s_text: Some("E") },
    Key { sc: 33, egui: Some(egui::Key::F), text: Some("f"), s_text: Some("F") },
    Key { sc: 34, egui: Some(egui::Key::G), text: Some("g"), s_text: Some("G") },
    Key { sc: 35, egui: Some(egui::Key::H), text: Some("h"), s_text: Some("H") },
    Key { sc: 23, egui: Some(egui::Key::I), text: Some("i"), s_text: Some("I") },
    Key { sc: 36, egui: Some(egui::Key::J), text: Some("j"), s_text: Some("J") },
    Key { sc: 37, egui: Some(egui::Key::K), text: Some("k"), s_text: Some("K") },
    Key { sc: 38, egui: Some(egui::Key::L), text: Some("l"), s_text: Some("L") },
    Key { sc: 50, egui: Some(egui::Key::M), text: Some("m"), s_text: Some("M") },
    Key { sc: 49, egui: Some(egui::Key::N), text: Some("n"), s_text: Some("N") },
    Key { sc: 24, egui: Some(egui::Key::O), text: Some("o"), s_text: Some("O") },
    Key { sc: 25, egui: Some(egui::Key::P), text: Some("p"), s_text: Some("P") },
    Key { sc: 16, egui: Some(egui::Key::Q), text: Some("q"), s_text: Some("Q") },
    Key { sc: 19, egui: Some(egui::Key::R), text: Some("r"), s_text: Some("R") },
    Key { sc: 31, egui: Some(egui::Key::S), text: Some("s"), s_text: Some("S") },
    Key { sc: 20, egui: Some(egui::Key::T), text: Some("t"), s_text: Some("T") },
    Key { sc: 22, egui: Some(egui::Key::U), text: Some("u"), s_text: Some("U") },
    Key { sc: 47, egui: Some(egui::Key::V), text: Some("v"), s_text: Some("V") },
    Key { sc: 17, egui: Some(egui::Key::W), text: Some("w"), s_text: Some("W") },
    Key { sc: 45, egui: Some(egui::Key::X), text: Some("x"), s_text: Some("X") },
    Key { sc: 21, egui: Some(egui::Key::Y), text: Some("y"), s_text: Some("Y") },
    Key { sc: 44, egui: Some(egui::Key::Z), text: Some("z"), s_text: Some("Z") },
    Key { sc: 11, egui: Some(egui::Key::Num0), text: Some("0"), s_text: Some(")") },
    Key { sc: 2, egui: Some(egui::Key::Num1), text: Some("1"), s_text: Some("!") },
    Key { sc: 3, egui: Some(egui::Key::Num2), text: Some("2"), s_text: Some("@") },
    Key { sc: 4, egui: Some(egui::Key::Num3), text: Some("3"), s_text: Some("#") },
    Key { sc: 5, egui: Some(egui::Key::Num4), text: Some("4"), s_text: Some("$") },
    Key { sc: 6, egui: Some(egui::Key::Num5), text: Some("5"), s_text: Some("%") },
    Key { sc: 7, egui: Some(egui::Key::Num6), text: Some("6"), s_text: Some("^") },
    Key { sc: 8, egui: Some(egui::Key::Num7), text: Some("7"), s_text: Some("&") },
    Key { sc: 9, egui: Some(egui::Key::Num8), text: Some("8"), s_text: Some("*") },
    Key { sc: 10, egui: Some(egui::Key::Num9), text: Some("9"), s_text: Some("(") },
    Key { sc: 1, egui: Some(egui::Key::Escape), text: None, s_text: None },
    Key { sc: 12, egui: Some(egui::Key::Minus), text: Some("-"), s_text: Some("_") },
    Key { sc: 13, egui: Some(egui::Key::Equals), text: Some("="), s_text: Some("+") },
    Key { sc: 14, egui: Some(egui::Key::Backspace), text: None, s_text: None },
    Key { sc: 15, egui: Some(egui::Key::Tab), text: None, s_text: None },
    Key { sc: 26, egui: Some(egui::Key::OpenBracket), text: Some("["), s_text: Some("{") },
    Key { sc: 27, egui: Some(egui::Key::CloseBracket), text: Some("]"), s_text: Some("}") },
    Key { sc: 28, egui: Some(egui::Key::Enter), text: None, s_text: None },
    Key { sc: 39, egui: Some(egui::Key::Semicolon), text: Some(";"), s_text: Some(":") },
    Key { sc: 40, egui: None, text: Some("'"), s_text: Some("\"") },
    Key { sc: 41, egui: Some(egui::Key::Backtick), text: Some("`"), s_text: Some("~") },
    Key { sc: 43, egui: Some(egui::Key::Backslash), text: Some("\\"), s_text: Some("|") },
    Key { sc: 51, egui: Some(egui::Key::Comma), text: Some(","), s_text: Some("<") },
    Key { sc: 52, egui: Some(egui::Key::Period), text: Some("."), s_text: Some(">") },
    Key { sc: 53, egui: Some(egui::Key::Slash), text: Some("/"), s_text: Some("?") },
    Key { sc: 55, egui: None, text: Some("*"), s_text: Some("*") },
    Key { sc: 57, egui: Some(egui::Key::Space), text: Some(" "), s_text: Some(" ") },
    Key { sc: 59, egui: Some(egui::Key::F1), text: None, s_text: None },
    Key { sc: 60, egui: Some(egui::Key::F2), text: None, s_text: None },
    Key { sc: 61, egui: Some(egui::Key::F3), text: None, s_text: None },
    Key { sc: 62, egui: Some(egui::Key::F4), text: None, s_text: None },
    Key { sc: 63, egui: Some(egui::Key::F5), text: None, s_text: None },
    Key { sc: 64, egui: Some(egui::Key::F6), text: None, s_text: None },
    Key { sc: 65, egui: Some(egui::Key::F7), text: None, s_text: None },
    Key { sc: 66, egui: Some(egui::Key::F8), text: None, s_text: None },
    Key { sc: 67, egui: Some(egui::Key::F9), text: None, s_text: None },
    Key { sc: 68, egui: Some(egui::Key::F10), text: None, s_text: None },
    Key { sc: 71, egui: Some(egui::Key::Num7), text: Some("7"), s_text: None },
    Key { sc: 72, egui: Some(egui::Key::Num8), text: Some("8"), s_text: None },
    Key { sc: 73, egui: Some(egui::Key::Num9), text: Some("9"), s_text: None },
    Key { sc: 74, egui: Some(egui::Key::Minus), text: Some("-"), s_text: None },
    Key { sc: 75, egui: Some(egui::Key::Num4), text: Some("4"), s_text: None },
    Key { sc: 76, egui: Some(egui::Key::Num5), text: Some("5"), s_text: None },
    Key { sc: 77, egui: Some(egui::Key::Num6), text: Some("6"), s_text: None },
    Key { sc: 78, egui: Some(egui::Key::Plus), text: Some("+"), s_text: None },
    Key { sc: 79, egui: Some(egui::Key::Num1), text: Some("1"), s_text: None },
    Key { sc: 80, egui: Some(egui::Key::Num2), text: Some("2"), s_text: None },
    Key { sc: 81, egui: Some(egui::Key::Num3), text: Some("3"), s_text: None },
    Key { sc: 82, egui: Some(egui::Key::Num0), text: Some("0"), s_text: None },
    Key { sc: 83, egui: Some(egui::Key::Period), text: Some("."), s_text: None },
    Key { sc: 87, egui: Some(egui::Key::F11), text: None, s_text: None },
    Key { sc: 88, egui: Some(egui::Key::F12), text: None, s_text: None },
    Key { sc: 96, egui: Some(egui::Key::Enter), text: None, s_text: None },
    Key { sc: 102, egui: Some(egui::Key::Home), text: None, s_text: None },
    Key { sc: 103, egui: Some(egui::Key::ArrowUp), text: None, s_text: None },
    Key { sc: 104, egui: Some(egui::Key::PageUp), text: None, s_text: None },
    Key { sc: 105, egui: Some(egui::Key::ArrowLeft), text: None, s_text: None },
    Key { sc: 106, egui: Some(egui::Key::ArrowRight), text: None, s_text: None },
    Key { sc: 107, egui: Some(egui::Key::End), text: None, s_text: None },
    Key { sc: 108, egui: Some(egui::Key::ArrowDown), text: None, s_text: None },
    Key { sc: 109, egui: Some(egui::Key::PageDown), text: None, s_text: None },
    Key { sc: 110, egui: Some(egui::Key::Insert), text: None, s_text: None },
    Key { sc: 111, egui: Some(egui::Key::Delete), text: None, s_text: None },
];

pub fn handle(
    conn: &XCBConnection, detail: u8, state: KeyButMask, pressed: bool, app: &mut WgpuLockbook,
    paste_context: &mut input::clipboard_paste::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    // "X11-style keycodes are offset by 8 from the keycodes the Linux kernel uses."
    // https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/common/keymap.rs#L7
    //let key = detail.saturating_sub(8);
    let key = detail;

    let modifiers = modifiers(state);

    // text
    if pressed && (modifiers.shift_only() || modifiers.is_none()) {
        if let Some(text) = key_text(key, modifiers.shift) {
            app.raw_input
                .events
                .push(egui::Event::Text(text.to_owned()));
        }
    }
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    println!(
        "STATUS: {}",
        x11::setup_xkb_extension(
            conn,
            1,
            0,
            x11::SetupXkbExtensionFlags::NoFlags,
            &mut 0,
            &mut 0,
            &mut 0,
            &mut 0,
        )
    );
    let device_id = x11::get_core_keyboard_device_id(conn);
    println!("{device_id}");
    let keymap =
        x11::keymap_new_from_device(&context, conn, device_id, xkb::KEYMAP_COMPILE_NO_FLAGS);
    println!("got map");
    let state = x11::state_new_from_device(&keymap, conn, device_id);
    println!("key0: {} active {:?}", keymap.layout_get_name(0), state.layout_index_is_active(0, xkb::STATE_LAYOUT_EFFECTIVE));
    println!("key1: {} active {:?}", keymap.layout_get_name(1), state.layout_index_is_active(1, xkb::STATE_LAYOUT_EFFECTIVE));
    let keysym = state.key_get_one_sym(key.into());



    // let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    // let keymap =
    //     xkb::Keymap::new_from_names(&context, "", "", "", "", None, KEYMAP_COMPILE_NO_FLAGS)
    //         .unwrap();
    // println!("num layouts: {}", keymap.num_layouts());
    // println!("keymap 0 name: {}", keymap.layout_get_name(0));
    // println!("keymap 1 name: {}", keymap.layout_get_name(1));
    // let keysyms_per_keycode = mapping.keysyms_per_keycode as usize;

    // // Calculate the starting index for the keysyms for the given keycode
    // let keycode = detail;
    // let index = (keycode - min_keycode) as usize * keysyms_per_keycode;
    // let keysyms_slice = &mapping.keysyms[index..index + keysyms_per_keycode];
    // // let state = xkb::State::new(&keymap);
    // let keysym = keysyms_slice[0];
    let s = xkb::keysym_to_utf8(keysym.into());
    println!("key: {key}, s: {s}" );

    // todo: something feels weird about this
    if let Some(key) = egui_key(key) {
        if pressed && key == egui::Key::X && modifiers.command {
            app.raw_input.events.push(egui::Event::Cut);
        } else if pressed && key == egui::Key::C && modifiers.command {
            app.raw_input.events.push(egui::Event::Copy);
        } else if pressed && key == egui::Key::V && modifiers.command {
            paste_context.handle_paste()?;
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
    }

    Ok(())
}

fn key_text(sc: u8, shift: bool) -> Option<&'static str> {
    KEYS.iter()
        .find(|key| key.sc == sc)
        .and_then(|key| if shift { key.s_text } else { key.text })
}

fn egui_key(sc: u8) -> Option<egui::Key> {
    KEYS.iter()
        .find(|key| key.sc == sc)
        .and_then(|key| key.egui)
}
