use crate::window::AtomCollection;
use egui::DroppedFile;
use lbeguiapp::WgpuLockbook;
use percent_encoding::percent_decode;
use std::path::{Path, PathBuf};
use x11rb::protocol::xproto::{self, Atom, ConnectionExt};
use x11rb::xcb_ffi::{ConnectionError, XCBConnection};

/// we're a drop target: let the drag source know if we support the file type
pub fn handle_enter(
    conn: &XCBConnection, atoms: &AtomCollection, event: &xproto::ClientMessageEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = event.data.as_data32();
    let source_window = data[0];
    let more_types = data[1] & 1 == 1;

    let mut types = Vec::new();
    for &atom in &data[2..5] {
        if atom == 0 {
            continue;
        }
        types.push(atom);
    }
    if more_types {
        for atom in get_more_types(conn, atoms, source_window)? {
            types.push(atom);
        }
    }

    // todo: verify that we support one of the types (i.e. text/uri-list)
    convert_selection(conn, atoms, event.window, x11rb::CURRENT_TIME);
    send_status(conn, atoms, event.window, source_window, true)?;

    Ok(())
}

/// todo: differentiate drop positions (e.g. dropping into the file tree vs editor)
pub fn handle_position(
    _conn: &XCBConnection, _atoms: &AtomCollection, _event: &xproto::ClientMessageEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    // todo: do something?
    Ok(())
}

/// we're a drag source: does the drop target support the file type?
pub fn handle_status(
    _conn: &XCBConnection, _atoms: &AtomCollection, _event: &xproto::ClientMessageEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    // todo: do something?
    Ok(())
}

/// we're no longer a drop target
pub fn handle_leave(
    _conn: &XCBConnection, _atoms: &AtomCollection, _event: &xproto::ClientMessageEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    // todo: do something?
    Ok(())
}

/// indicate that a drop has completed and the source can do whatever it wants with the data now
pub fn handle_drop(
    conn: &XCBConnection, atoms: &AtomCollection, event: &xproto::ClientMessageEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = event.data.as_data32();
    let source_window = data[0];

    send_finished(conn, atoms, event.window, source_window, true)?;

    Ok(())
}

/// once the selection has been converted, read the data
pub fn handle_selection_notify(
    conn: &XCBConnection, atoms: &AtomCollection, event: &xproto::SelectionNotifyEvent,
    app: &mut WgpuLockbook,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = read_data(conn, atoms, event.requestor)?;
    let paths = parse_paths(&data)?;
    for path in paths {
        app.raw_input
            .dropped_files
            .push(DroppedFile { path: Some(path), ..Default::default() });
    }
    Ok(())
}

// https://freedesktop.org/wiki/Specifications/XDND/#clientmessages
fn send_status(
    conn: &XCBConnection, atoms: &AtomCollection, this_window: xproto::Window,
    target_window: xproto::Window, accepted: bool,
) -> Result<(), ConnectionError> {
    let (accepted, action) =
        if accepted { (1, atoms.XdndActionCopy) } else { (0, atoms.XdndActionNone) };
    conn.send_event(
        false,
        target_window,
        xproto::EventMask::NO_EVENT,
        xproto::ClientMessageEvent::new(
            32,
            target_window,
            atoms.XdndStatus,
            xproto::ClientMessageData::from([this_window, accepted, 0, 0, action]),
        ),
    )?;
    Ok(())
}

// https://freedesktop.org/wiki/Specifications/XDND/#clientmessages
fn send_finished(
    conn: &XCBConnection, atoms: &AtomCollection, this_window: xproto::Window,
    target_window: xproto::Window, accepted: bool,
) -> Result<(), ConnectionError> {
    let (accepted, action) =
        if accepted { (1, atoms.XdndActionCopy) } else { (0, atoms.XdndActionNone) };
    conn.send_event(
        false,
        target_window,
        xproto::EventMask::NO_EVENT,
        xproto::ClientMessageEvent::new(
            32,
            target_window,
            atoms.XdndFinished,
            xproto::ClientMessageData::from([this_window, accepted, action, 0, 0]),
        ),
    )?;
    Ok(())
}

fn get_more_types(
    conn: &XCBConnection, atoms: &AtomCollection, source_window: xproto::Window,
) -> Result<Vec<Atom>, Box<dyn std::error::Error>> {
    let type_list = conn
        .get_property(
            false,
            source_window,
            atoms.XdndTypeList,
            xproto::Atom::from(xproto::AtomEnum::ATOM),
            0,
            u32::MAX,
        )?
        .reply()?
        .value;

    let mut types = Vec::new();
    for atom in type_list.chunks_exact(4) {
        let atom = xproto::Atom::from_ne_bytes([atom[0], atom[1], atom[2], atom[3]]);
        types.push(atom);
    }
    Ok(types)
}

fn convert_selection(
    conn: &XCBConnection, atoms: &AtomCollection, window: xproto::Window, time: xproto::Timestamp,
) {
    conn.convert_selection(
        window,
        atoms.XdndSelection,
        atoms.TextUriList,
        atoms.XdndSelection,
        time,
    )
    .expect("Failed to convert selection");
}

fn read_data(
    conn: &XCBConnection, atoms: &AtomCollection, window: xproto::Window,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let data = conn
        .get_property(false, window, atoms.XdndSelection, atoms.TextUriList, 0, u32::MAX)?
        .reply()?
        .value;

    Ok(data)
}

fn parse_paths(data: &[u8]) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if !data.is_empty() {
        let mut path_list = Vec::new();
        let decoded = percent_decode(data).decode_utf8()?.into_owned();
        for uri in decoded.split("\r\n").filter(|u| !u.is_empty()) {
            // The format is specified as protocol://host/path
            // However, it's typically simply protocol:///path
            let path_str = if uri.starts_with("file://") {
                let path_str = uri.replace("file://", "");
                if !path_str.starts_with('/') {
                    // todo: support files with hostnames (e.g. file:://hostname/path)
                    return Err("dropped file URI has hostname".into());
                }
                path_str
            } else {
                // todo: support other protocols (e.g. sftp://path)
                return Err("dropped file has non-file protocol".into());
            };

            let path = Path::new(&path_str).canonicalize()?;
            path_list.push(path);
        }
        Ok(path_list)
    } else {
        Err("dropped file has empty path".into())
    }
}
