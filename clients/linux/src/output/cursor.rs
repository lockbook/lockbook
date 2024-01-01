use egui::CursorIcon;
use x11rb::{
    connection::Connection as _,
    protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt, CreateGCAux, Screen},
    xcb_ffi::{ReplyOrIdError, XCBConnection},
};

pub fn handle(conn: &XCBConnection, screen: &Screen, window: u32, cursor_icon: CursorIcon) {
    match handle_impl(conn, screen, window, cursor_icon) {
        Ok(_) => {}
        Err(e) => {
            println!("Failed to set cursor: {:?}", e);
        }
    }
}

fn handle_impl(
    conn: &XCBConnection, screen: &Screen, window: u32, cursor_icon: CursorIcon,
) -> Result<(), ReplyOrIdError> {
    let font = conn.generate_id()?;
    conn.open_font(font, b"cursor")?;

    let cursor = conn.generate_id()?;
    let cursor_id = cursor_icon as u16;
    conn.create_glyph_cursor(cursor, font, font, cursor_id, cursor_id + 1, 0, 0, 0, 0, 0, 0)?;

    let gc = conn.generate_id()?;
    let values = CreateGCAux::default()
        .foreground(screen.black_pixel)
        .background(screen.black_pixel)
        .font(font);
    conn.create_gc(gc, window, &values)?;

    let values = ChangeWindowAttributesAux::default().cursor(cursor);
    conn.change_window_attributes(window, &values)?;

    conn.free_cursor(cursor)?;
    conn.close_font(font)?;

    Ok(())
}
