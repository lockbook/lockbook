use x11rb::protocol::xproto::{
    ConnectionExt as _, EventMask, PropMode, SelectionNotifyEvent, SelectionRequestEvent,
};
use x11rb::reexports::x11rb_protocol::protocol::xproto;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

use crate::window::AtomCollection;

pub fn handle_copy(
    conn: &XCBConnection, atoms: &AtomCollection, window_id: xproto::Window, copied_text: String,
    last_copied_text: &mut String,
) -> Result<(), Box<dyn std::error::Error>> {
    if !copied_text.is_empty() && copied_text != *last_copied_text {
        conn.set_selection_owner(window_id, atoms.CLIPBOARD, x11rb::CURRENT_TIME)?;
        *last_copied_text = copied_text;
    }

    Ok(())
}

pub fn handle_selection_request(
    conn: &XCBConnection, atoms: &AtomCollection, event: &SelectionRequestEvent,
    last_copied_text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut notification = SelectionNotifyEvent {
        response_type: xproto::SELECTION_NOTIFY_EVENT,
        sequence: event.sequence,
        time: event.time,
        requestor: event.requestor,
        selection: event.selection,
        target: event.target,
        property: atoms.NONE, // by default, we don't have the requested selection
    };

    // todo: support middle mouse button paste (uses PRIMARY selection rather than CLIPBOARD)
    if event.selection == atoms.CLIPBOARD {
        if event.target == atoms.TARGETS {
            // deliver the supported types
            conn.change_property32(
                PropMode::REPLACE,
                event.requestor,
                event.property,
                xproto::Atom::from(xproto::AtomEnum::ATOM),
                &[atoms.TARGETS, atoms.UTF8_STRING], // we only support copying UTF-8 strings
            )?;
            notification.property = event.property;
        }
        if event.target == atoms.UTF8_STRING {
            // deliver the copied text
            conn.change_property8(
                PropMode::REPLACE,
                event.requestor,
                event.property,
                event.target,
                last_copied_text.as_bytes(),
            )?;
            notification.property = event.property;
        }
    }

    // notify requestor
    conn.send_event(false, event.requestor, EventMask::NO_EVENT, notification)?;

    Ok(())
}
