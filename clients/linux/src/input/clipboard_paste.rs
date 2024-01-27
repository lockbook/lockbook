use lb::FileType;
use lbeguiapp::WgpuLockbook;
use std::time::{SystemTime, UNIX_EPOCH};
use x11rb::protocol::xproto::{Atom, ConnectionExt as _};
use x11rb::reexports::x11rb_protocol::protocol::xproto;
use x11rb::xcb_ffi::XCBConnection;

use crate::window::AtomCollection;

pub fn handle_paste(
    conn: &XCBConnection, atoms: &AtomCollection, window: xproto::Window,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("handle paste: request targets");

    // request clipboard targets; we'll get a SelectionNotify event when it's ready
    conn.convert_selection(
        window,
        atoms.CLIPBOARD,
        atoms.TARGETS,
        atoms.CLIPBOARD,
        x11rb::CURRENT_TIME,
    )?;

    Ok(())
}

pub fn handle_selection_notify(
    conn: &XCBConnection, atoms: &AtomCollection, event: &xproto::SelectionNotifyEvent,
    app: &mut WgpuLockbook,
) -> Result<(), Box<dyn std::error::Error>> {
    // todo: support middle mouse button paste
    if event.property == atoms.CLIPBOARD {
        if event.target == atoms.TARGETS {
            // request clipboard data in a supported/preferred format; we'll get another SelectionNotify event when it's ready
            let targets = get_targets(conn, atoms, event.requestor)?;

            if targets.contains(&atoms.ImagePng) {
                println!("handle selection: request image png");
                conn.convert_selection(
                    event.requestor,
                    atoms.CLIPBOARD,
                    atoms.ImagePng,
                    atoms.CLIPBOARD,
                    event.time,
                )?;
            } else if targets.contains(&atoms.UTF8_STRING) {
                println!("handle selection: request utf-8 string");
                conn.convert_selection(
                    event.requestor,
                    atoms.CLIPBOARD,
                    atoms.UTF8_STRING,
                    atoms.CLIPBOARD,
                    event.time,
                )?;
            }
        } else {
            println!("handle selection: request clipboard data");

            // get clipboard data
            let data = read_clipboard_data(conn, atoms, event.requestor, event.target)?;
            if data.is_empty() {
                println!("handle selection: clipboard data is empty");
                return Ok(());
            }

            if event.target == atoms.UTF8_STRING {
                println!("handle selection: paste utf-8 string");

                // utf8 -> egui Paste event
                let text = String::from_utf8_lossy(&data);
                app.raw_input
                    .events
                    .push(egui::Event::Paste(text.to_string()));
            } else if event.target == atoms.ImagePng {
                println!("handle selection: paste image png");

                // png -> import lockbook file and paste markdown image link
                // todo: dedupe with code in windows app, possibly other places
                image_paste(app, data)?;
            } else {
                // print type of unsupported clipboard data
                let name = conn.get_atom_name(event.target)?.reply()?.name;
                let name = String::from_utf8(name).expect("get atom name as utf8");
                println!("handle selection: unsupported clipboard type: {}", name);
            }
        }
    }

    Ok(())
}

pub fn handle_property_notify(
    conn: &XCBConnection, atoms: &AtomCollection, event: &xproto::PropertyNotifyEvent,
    app: &mut WgpuLockbook, clipboard_paste_incremental_transfer: &mut Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    // if event.state == xproto::Property::NEW_VALUE {
    //     let increment = conn
    //         .get_property(false, event.window, atoms.CLIPBOARD, atoms.ImagePng, 0, u32::MAX)?
    //         .reply()?
    //         .value;
    //     clipboard_paste_incremental_transfer.extend_from_slice(&increment);
    //     conn.delete_property(event.window, atoms.CLIPBOARD)?;

    //     println!("got {} bytes of incremental transfer", increment.len());
    //     if increment.is_empty() {
    //         println!("done with incremental transfer");
    //     }
    // }

    Ok(())
}

fn image_paste(app: &mut WgpuLockbook, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    let core = match &app.app {
        lbeguiapp::Lockbook::Splash(_) => {
            return Ok(());
        }
        lbeguiapp::Lockbook::Onboard(screen) => &screen.core,
        lbeguiapp::Lockbook::Account(screen) => &screen.core,
    };
    let file = core
        .create_file(
            &format!(
                "pasted_image_{}.png",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
            ),
            core.get_root().expect("get lockbook root").id,
            FileType::Document,
        )
        .expect("create lockbook file for image");
    core.write_document(file.id, &data)
        .expect("write lockbook file for image");
    let markdown_image_link = format!("![pasted image](lb://{})", file.id);
    app.raw_input
        .events
        .push(egui::Event::Paste(markdown_image_link));

    Ok(())
}

fn get_targets(
    conn: &XCBConnection, atoms: &AtomCollection, source_window: xproto::Window,
) -> Result<Vec<Atom>, Box<dyn std::error::Error>> {
    let type_list = conn
        .get_property(
            false,
            source_window,
            atoms.CLIPBOARD,
            xproto::Atom::from(xproto::AtomEnum::ATOM),
            0,
            std::u32::MAX,
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

fn read_clipboard_data(
    conn: &XCBConnection, atoms: &AtomCollection, window: xproto::Window, type_: Atom,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // print type
    let name = conn.get_atom_name(type_)?.reply()?.name;
    let name = String::from_utf8(name).expect("get atom name as utf8");
    println!("read clipboard data: clipboard type: {}", name);

    let mut data = Vec::new();
    let mut offset = 0;
    let mut is_incr = false;
    loop {
        let reply = conn
            .get_property(false, window, atoms.CLIPBOARD, type_, offset, std::u32::MAX)?
            .reply()?;
        is_incr |= reply.type_ == atoms.INCR;

        data.extend_from_slice(&reply.value);
        offset += reply.value_len;
        if reply.bytes_after == 0 {
            break;
        }
    }

    if is_incr {
        println!("read clipboard data: clipboard data is incremental");

        // this is how we indicate we're ready to receive an incremental transfer
        conn.delete_property(window, atoms.CLIPBOARD)?;
    }

    Ok(data)
}
