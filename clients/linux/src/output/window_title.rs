use x11rb::{
    protocol::xproto::{AtomEnum, PropMode},
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
};

use crate::window::AtomCollection;

pub fn handle(
    conn: &XCBConnection, window_id: u32, atoms: &AtomCollection, set_window_title: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // let title = set_window_title.unwrap_or_else(|| "Lockbook".to_string());
    let title = match set_window_title {
        Some(title) => title,
        None => return Ok(()),
    };

    // set the window title with a null-terminated string
    println!("conn.change_property8 (2)");
    conn.change_property8(
        PropMode::REPLACE,
        window_id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    )?;

    // set the window title (Extended Window Manager Hints) with a UTF-8 string
    conn.change_property8(
        PropMode::REPLACE,
        window_id,
        atoms._NET_WM_NAME,
        atoms.UTF8_STRING,
        title.as_bytes(),
    )?;

    Ok(())
}
