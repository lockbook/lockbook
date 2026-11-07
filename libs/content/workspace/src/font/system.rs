//! The system's fonts. Apple platforms and Android draw text, symbols, and
//! every script with them; Windows draws text and every script with them, with
//! the bundled Noto behind; Linux draws whatever the bundled fonts lack with
//! those its fallback lists name; elsewhere only CJK comes from the system.

use epaint::text::Blob;
use glyphon::fontdb::{Database, FaceInfo, Source, Style};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use unicode_script::Script;

/// The faces the interface's text draws in, found once at startup: the
/// system's text and monospace families. Every other family is read when
/// it's first needed, off the UI thread, by [`super::loader`].
pub fn found() -> &'static Database {
    static FOUND: OnceLock<Database> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut found = Database::new();
        #[cfg(target_os = "macos")]
        for font in super::os::interface_fonts() {
            found.load_font_source(font);
        }
        for path in interface_files() {
            if path.is_dir() {
                found.load_fonts_dir(&path);
            } else if path.is_file() {
                let _ = found.load_font_file(&path);
            }
        }
        #[cfg(target_os = "ios")]
        found.load_font_source(Source::Binary(Arc::new(lb_fonts::NOTO_SANS_SC_REGULAR)));
        drawable(found)
    })
}

/// Every installed drawable face, read the first time a family is asked for
/// by name where no system service names its files.
#[cfg(not(any(target_vendor = "apple", windows)))]
fn everything() -> &'static Database {
    EVERYTHING.get_or_init(|| {
        let mut everything = Database::new();
        #[cfg(target_os = "android")]
        everything.load_fonts_dir("/system/fonts");
        #[cfg(not(target_os = "android"))]
        {
            everything.load_system_fonts();
            // A snap's fontconfig names the user's fonts under the snap's own
            // home, which has none; its desktop interface lets it read the user's.
            if let Some(home) = std::env::var_os("SNAP_REAL_HOME").map(PathBuf::from) {
                for dir in [".local/share/fonts", ".fonts"] {
                    everything.load_fonts_dir(home.join(dir));
                }
            }
        }
        drawable(everything)
    })
}

/// `name`'s faces, read where the system keeps them: the interface's from
/// startup, else the files the system's font service names for it, else
/// every installed face, read once. Reads font files, so only
/// [`super::loader`] calls it.
pub fn family_faces(name: &str) -> Option<(&'static str, Vec<FaceInfo>)> {
    let named = |f: &FaceInfo, name: &str| f.families.iter().any(|(n, _)| n == name);
    if let Some(family) = family_in(found(), name) {
        return Some((
            family,
            found()
                .faces()
                .filter(|f| named(f, family))
                .cloned()
                .collect(),
        ));
    }
    #[cfg(any(target_vendor = "apple", windows))]
    {
        let mut db = Database::new();
        for path in super::os::family_files(name) {
            let _ = db.load_font_file(&path);
        }
        let db = drawable(db);
        let faces: Vec<FaceInfo> = db.faces().filter(|f| named(f, name)).cloned().collect();
        if faces.is_empty() {
            return None;
        }
        // one leaked name per family read, as every other family name is static
        Some((Box::leak(name.into()), faces))
    }
    #[cfg(not(any(target_vendor = "apple", windows)))]
    {
        let family = family_in(everything(), name)?;
        Some((
            family,
            everything()
                .faces()
                .filter(|f| named(f, family))
                .cloned()
                .collect(),
        ))
    }
}

/// The faces of the family the system draws `c` in for text in `language`,
/// where the system answers (macOS, Windows). Reads font files, so only
/// [`super::loader`] calls it.
pub fn fallback_faces(c: char, language: &str) -> Option<(&'static str, Vec<FaceInfo>)> {
    #[cfg(target_os = "macos")]
    {
        // the font Core Text picks, which may go by a hidden name
        let (post_script, source) = super::os::fallback_source(c, language)?;
        let mut db = Database::new();
        db.load_font_source(source);
        let db = drawable(db);
        let picked = db.faces().find(|f| f.post_script_name == post_script)?;
        let family = picked.families[0].0.clone();
        let faces: Vec<FaceInfo> = db
            .faces()
            .filter(|f| f.families.iter().any(|(n, _)| *n == family))
            .cloned()
            .collect();
        Some((Box::leak(family.into_boxed_str()), faces))
    }
    #[cfg(windows)]
    {
        family_faces(&super::os::fallback_family(c, language)?)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (c, language);
        None
    }
}

/// Whether `face` draws printable ASCII, as a text face does: some families
/// carry ornaments under their name (Hoefler Text).
pub fn draws_text(face: &FaceInfo) -> bool {
    face_data(face).is_some_and(|data| draws_ascii((*data).as_ref(), face.index))
}

static EVERYTHING: OnceLock<Database> = OnceLock::new();

/// `db` without the faces Swash can't draw: Apple's `hvgl` outlines (as iOS
/// 26's and macOS 27's PingFang have) shape but leave invisible or broken
/// glyphs, even beside other outlines.
fn drawable(mut db: Database) -> Database {
    let undrawable: Vec<_> = db
        .faces()
        .filter(|f| db.with_face_data(f.id, has_outlines) != Some(true))
        .map(|f| f.id)
        .collect();
    for id in undrawable {
        db.remove_face(id);
    }
    db
}

fn has_outlines(data: &[u8], index: u32) -> bool {
    ttf_parser::RawFace::parse(data, index).is_ok_and(|face| {
        let has = |tag: &[u8; 4]| face.table(ttf_parser::Tag::from_bytes(tag)).is_some();
        !has(b"hvgl") && [b"glyf", b"CFF ", b"CFF2"].iter().any(|tag| has(tag))
    })
}

/// The faces of `family`, in the order found.
pub fn faces(family: &str) -> impl Iterator<Item = &'static FaceInfo> + '_ {
    let db = match EVERYTHING.get() {
        Some(everything) if family_in(found(), family).is_none() => everything,
        _ => found(),
    };
    db.faces()
        .filter(move |f| f.families.iter().any(|(n, _)| n == family))
}

/// The faces of `family` that draw printable ASCII, as a text face does.
pub fn text_faces(family: &str) -> impl Iterator<Item = &'static FaceInfo> + '_ {
    faces(family).filter(|f| draws_text(f))
}

/// `family` as `db` names it, where `db` has it.
fn family_in(db: &'static Database, family: &str) -> Option<&'static str> {
    db.faces()
        .flat_map(|f| &f.families)
        .map(|(n, _)| n.as_str())
        .find(|n| *n == family)
}

/// The first of `families` the system has.
fn first_present(families: &[&'static str]) -> Option<&'static str> {
    families
        .iter()
        .copied()
        .find(|family| faces(family).next().is_some())
}

/// Whether a face draws printable ASCII as letters. Symbol fonts map ASCII
/// too, to glyphs they name for what they draw (`alpha`, `a10`).
fn draws_ascii(data: &[u8], index: u32) -> bool {
    let named_as = |face: &ttf_parser::Face, c: char| {
        let name = face.glyph_index(c).and_then(|g| face.glyph_name(g));
        name.is_none_or(|n| n == c.to_string() || n == format!("uni{:04X}", c as u32))
    };
    ttf_parser::Face::parse(data, index).is_ok_and(|face| {
        (' '..='~').all(|c| face.glyph_index(c).is_some())
            && ['A', 'a'].into_iter().all(|c| named_as(&face, c))
    })
}

/// The system's text family, where it supplies one.
pub fn platform_sans() -> Option<&'static str> {
    static SANS_FAMILY: OnceLock<Option<&'static str>> = OnceLock::new();
    *SANS_FAMILY.get_or_init(|| first_present(SANS))
}

/// The system's monospace family, where it supplies one.
pub fn platform_mono() -> Option<&'static str> {
    static MONO_FAMILY: OnceLock<Option<&'static str>> = OnceLock::new();
    *MONO_FAMILY.get_or_init(|| first_present(MONO))
}

/// The family text draws in, where the system supplies one.
pub fn sans_family() -> Option<&'static str> {
    platform_sans()
}

/// The family code draws in, where the system supplies one.
pub fn mono_family() -> Option<&'static str> {
    platform_mono()
}

/// Appends the system's text and monospace faces after everything in `db`,
/// where the system supplies them; scripts and symbols come as text needs
/// them, from [`super::Fallback`]. Linux gives the bundled symbol fonts bold
/// copies, as fallback takes only faces of the weight asked for.
pub fn load(db: &mut Database) {
    #[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
    bold_symbols(db);
    #[cfg(not(all(unix, not(any(target_vendor = "apple", target_os = "android")))))]
    load_families(db);
}

#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
fn bold_symbols(db: &mut Database) {
    use glyphon::fontdb::Weight;
    let family = |f: &FaceInfo| f.families[0].0.clone();
    let bold: Vec<String> = db
        .faces()
        .filter(|f| f.weight == Weight::BOLD)
        .map(family)
        .collect();
    let copies: Vec<FaceInfo> = db
        .faces()
        .filter(|f| SYMBOLS.contains(&family(f).as_str()) && !bold.contains(&family(f)))
        .cloned()
        .collect();
    for mut face in copies {
        face.weight = Weight::BOLD;
        db.push_face_info(face);
    }
}

#[cfg(not(all(unix, not(any(target_vendor = "apple", target_os = "android")))))]
fn load_families(db: &mut Database) {
    let (sans, mono) = (sans_family(), mono_family());
    if let Some(sans) = sans {
        db.set_sans_serif_family(sans);
    }
    if let Some(mono) = mono {
        db.set_monospace_family(mono);
    }
    for family in sans.into_iter().chain(mono) {
        for face in text_faces(family) {
            db.push_face_info(shared(face));
        }
    }
}

/// The system families for a script, best first.
pub fn script_families(script: Script) -> Option<&'static [&'static str]> {
    SCRIPTS
        .iter()
        .find(|(s, _)| *s == script)
        .map(|(_, families)| *families)
}

/// Families to try for any character the script lists miss.
pub fn symbol_families() -> &'static [&'static str] {
    SYMBOLS
}

/// The face of `family` nearest `weight` in `style`, else in normal, else in
/// any style: some families have only an italic.
pub fn nearest(family: &str, weight: u16, style: Style) -> Option<&'static FaceInfo> {
    text_faces(family)
        .min_by_key(|f| (f.style != style, f.style != Style::Normal, f.weight.0.abs_diff(weight)))
}

/// `face` with its file mapped once and shared: the fallback reads every face
/// in a font system with each new style, and would otherwise reopen each file.
pub fn shared(face: &FaceInfo) -> FaceInfo {
    let mut face = face.clone();
    if let (Source::File(path), Some(data)) = (&face.source, face_data(&face)) {
        face.source = Source::SharedFile(path.clone(), data);
    }
    face
}

/// A face's bytes, shared and mapped rather than read, so only the glyphs
/// drawn cost memory.
pub fn face_data(face: &FaceInfo) -> Option<Blob> {
    static MAPPED: OnceLock<Mutex<HashMap<PathBuf, Blob>>> = OnceLock::new();
    let path = match &face.source {
        Source::Binary(data) => return Some(data.clone()),
        Source::File(path) | Source::SharedFile(path, _) => path,
    };
    let mapped = MAPPED.get_or_init(Default::default);
    if let Some(data) = mapped.lock().unwrap().get(path) {
        return Some(data.clone());
    }
    // mapped outside the lock, so one thread's read never holds up another's lookup
    let data: Blob = Arc::new(map(path)?);
    Some(
        mapped
            .lock()
            .unwrap()
            .entry(path.clone())
            .or_insert(data)
            .clone(),
    )
}

#[cfg(not(target_family = "wasm"))]
fn map(path: &Path) -> Option<memmap2::Mmap> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: system font files don't change under a running app.
    unsafe { memmap2::Mmap::map(&file) }.ok()
}

#[cfg(target_family = "wasm")]
fn map(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[cfg(target_vendor = "apple")]
const SANS: &[&str] = &[".SF NS", ".SF UI"];
#[cfg(target_vendor = "apple")]
const MONO: &[&str] = &[".SF NS Mono", ".SF UI Mono"];

#[cfg(target_os = "android")]
const SANS: &[&str] = &["Roboto"];
#[cfg(target_os = "android")]
const MONO: &[&str] = &["Droid Sans Mono"];

// Segoe UI over Segoe UI Variable, which has no italic
#[cfg(target_os = "windows")]
const SANS: &[&str] = &["Segoe UI"];
#[cfg(target_os = "windows")]
const MONO: &[&str] = &["Cascadia Mono", "Consolas"];

#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
const SANS: &[&str] = &[];
#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
const MONO: &[&str] = &[];

#[cfg(target_os = "android")]
const SCRIPTS: &[(Script, &[&str])] = &[
    (Script::Arabic, &["Noto Naskh Arabic"]),
    (Script::Armenian, &["Noto Sans Armenian"]),
    (Script::Bengali, &["Noto Sans Bengali"]),
    (Script::Cherokee, &["Noto Sans Cherokee"]),
    (Script::Devanagari, &["Noto Sans Devanagari"]),
    (Script::Ethiopic, &["Noto Sans Ethiopic"]),
    (Script::Georgian, &["Noto Sans Georgian"]),
    (Script::Gujarati, &["Noto Sans Gujarati"]),
    (Script::Gurmukhi, &["Noto Sans Gurmukhi"]),
    (Script::Hebrew, &["Noto Sans Hebrew"]),
    (Script::Kannada, &["Noto Sans Kannada"]),
    (Script::Khmer, &["Noto Sans Khmer"]),
    (Script::Lao, &["Noto Sans Lao"]),
    (Script::Malayalam, &["Noto Sans Malayalam"]),
    (Script::Mongolian, &["Noto Sans Mongolian"]),
    (Script::Myanmar, &["Noto Sans Myanmar"]),
    (Script::Oriya, &["Noto Sans Oriya"]),
    (Script::Sinhala, &["Noto Sans Sinhala"]),
    (Script::Syriac, &["Noto Sans Syriac Estrangela", "Noto Sans Syriac"]),
    (Script::Tamil, &["Noto Sans Tamil"]),
    (Script::Telugu, &["Noto Sans Telugu"]),
    (Script::Thaana, &["Noto Sans Thaana"]),
    (Script::Thai, &["Noto Sans Thai"]),
    (Script::Tibetan, &["Noto Serif Tibetan"]),
];
#[cfg(target_os = "android")]
const SYMBOLS: &[&str] = &["Noto Sans Symbols"];

/// Where the system names each script's font (macOS, Windows) it's asked per
/// script instead, and Linux's come from fontconfig's lists.
#[cfg(not(target_os = "android"))]
const SCRIPTS: &[(Script, &[&str])] = &[];
/// The bundled text and emoji faces, ahead of the system's for what a
/// script's fonts lack: DejaVu Sans would draw emoji in monochrome. Noto Sans
/// Math draws the arrows and math Noto Sans lacks on its axis; Noto Sans
/// Mono's sit low.
#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
const SYMBOLS: &[&str] = &["Noto Sans", "Noto Sans Math", "Noto Sans Mono", "Twemoji Mozilla"];
#[cfg(any(target_vendor = "apple", windows))]
const SYMBOLS: &[&str] = &[];

#[cfg(target_os = "macos")]
fn interface_files() -> Vec<PathBuf> {
    vec![]
}

#[cfg(target_os = "windows")]
fn interface_files() -> Vec<PathBuf> {
    SANS.iter()
        .chain(MONO)
        .flat_map(|family| super::os::family_files(family))
        .collect()
}

#[cfg(target_os = "ios")]
fn interface_files() -> Vec<PathBuf> {
    // Simulator processes see the host's /System; the runtime's is here.
    let root = std::env::var("IPHONE_SIMULATOR_ROOT").unwrap_or_default();
    vec![PathBuf::from(format!("{root}/System/Library/Fonts/Core"))]
}

#[cfg(target_os = "android")]
fn interface_files() -> Vec<PathBuf> {
    vec!["/system/fonts".into()]
}

/// Linux's interface draws in the bundled Noto.
#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
fn interface_files() -> Vec<PathBuf> {
    vec![]
}
