//! Families read and measured off the UI thread. The UI asks by name and
//! never parses a font; until a family is ready it draws as before and asks
//! again on a later frame.

use super::system;
use glyphon::fontdb::FaceInfo;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

/// A family's text faces, read and measured, ready to add to a font system.
#[derive(Clone)]
pub struct Family {
    pub name: &'static str,
    pub faces: Arc<[FaceInfo]>,
    /// Its lowercase height, in em.
    pub x_height: Option<f32>,
    /// How tall its `d` stands, in em: where its ascenders reach.
    pub ascender: Option<f32>,
}

/// What a family is read for: text takes only its faces that draw ASCII, a
/// script fallback takes every face.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Text,
    Script,
    /// The family the system draws a character in, asked as the character
    /// and a language tag, tab-separated.
    Fallback,
}

#[derive(Clone)]
pub enum Load {
    Loading,
    Ready(Family),
    /// Not installed, or no face of it draws text.
    Missing,
}

/// `name`'s family read for `kind`, starting its read the first time it's
/// asked for. The lock is held only to look up or record a finished read.
pub fn get(name: &str, kind: Kind) -> Load {
    let key = (name.to_owned(), kind);
    let mut loads = loads().lock().unwrap();
    if let Some(load) = loads.get(&key) {
        return load.clone();
    }
    loads.insert(key.clone(), Load::Loading);
    drop(loads);
    let _ = reader().send(key);
    Load::Loading
}

type Key = (String, Kind);

fn loads() -> &'static Mutex<HashMap<Key, Load>> {
    static LOADS: OnceLock<Mutex<HashMap<Key, Load>>> = OnceLock::new();
    LOADS.get_or_init(Default::default)
}

/// The thread that reads families, one at a time in the order asked.
fn reader() -> &'static Sender<Key> {
    static READER: OnceLock<Sender<Key>> = OnceLock::new();
    READER.get_or_init(|| {
        let (send, receive) = channel::<Key>();
        std::thread::spawn(move || {
            for key in receive {
                let load =
                    std::panic::catch_unwind(|| read(&key.0, key.1)).unwrap_or(Load::Missing);
                loads().lock().unwrap().insert(key, load);
            }
        });
        send
    })
}

/// Whether any family is still being read.
pub fn busy() -> bool {
    loads()
        .lock()
        .unwrap()
        .values()
        .any(|load| matches!(load, Load::Loading))
}

fn read(name: &str, kind: Kind) -> Load {
    let found = match kind {
        Kind::Fallback => name
            .split_once('\t')
            .and_then(|(c, language)| system::fallback_faces(c.chars().next()?, language)),
        _ => system::family_faces(name),
    };
    let Some((family, faces)) = found else { return Load::Missing };
    let faces: Arc<[FaceInfo]> = faces
        .iter()
        .filter(|face| kind != Kind::Text || system::draws_text(face))
        .map(system::shared)
        .collect();
    if faces.is_empty() {
        return Load::Missing;
    }
    let measured = |letter| match kind {
        Kind::Text => super::letter_height(&faces, letter),
        Kind::Script | Kind::Fallback => None,
    };
    let (x_height, ascender) = (measured('x'), measured('d'));
    Load::Ready(Family { name: family, faces, x_height, ascender })
}
