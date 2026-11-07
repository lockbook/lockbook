//! The system's fonts. Apple platforms and Android draw text, symbols, and
//! every script with them; Windows draws text and every script with them, with
//! the bundled Noto behind; Linux draws whatever the bundled fonts lack with
//! those its fallback lists name; elsewhere only CJK comes from the system.

use epaint::text::Blob;
use glyphon::fontdb::{Database, FaceInfo, Source, Style};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};
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

/// Every installed drawable face, read the first time the full font list
/// is asked for.
fn everything() -> &'static Database {
    EVERYTHING.get_or_init(|| {
        let mut everything = Database::new();
        #[cfg(any(target_vendor = "apple", windows))]
        for path in super::os::all_files() {
            let _ = everything.load_font_file(&path);
        }
        #[cfg(target_os = "android")]
        everything.load_fonts_dir("/system/fonts");
        #[cfg(not(any(target_vendor = "apple", windows, target_os = "android")))]
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

/// Every installed family that can be chosen, by name, with whether it's
/// monospaced; `None` until the scan the first call starts has finished. A
/// family can be chosen when it draws printable ASCII, as the app's own text
/// is, and writes one of the user's languages.
pub fn families() -> Option<&'static [(&'static str, bool)]> {
    static FAMILIES: OnceLock<Vec<(&'static str, bool)>> = OnceLock::new();
    static SCAN: Once = Once::new();
    SCAN.call_once(|| {
        std::thread::spawn(|| {
            FAMILIES.get_or_init(|| {
                let languages = super::cjk::languages();
                let mut families: Vec<(&'static str, bool)> = vec![];
                let db = everything();
                for face in db.faces() {
                    let name = face.families[0].0.as_str();
                    // Apple's own UI faces, which it hides from font menus
                    let listed = |data: &[u8], index| {
                        draws_ascii(data, index) && writes_any(data, index, &languages)
                    };
                    if name.starts_with('.') || db.with_face_data(face.id, listed) != Some(true) {
                        continue;
                    }
                    match families.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, mono)) => *mono |= face.monospaced,
                        None => families.push((name, face.monospaced)),
                    }
                }
                families.sort_by_key(|(n, _)| n.to_lowercase());
                families
            });
        });
    });
    FAMILIES.get().map(Vec::as_slice)
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

/// Whether a face writes one of `languages`: it draws its letters, a font
/// declaring the scripts it was designed for names its script, and a CJK font
/// is one made for that language, as its code pages declare.
fn writes_any(data: &[u8], index: u32, languages: &[String]) -> bool {
    let Ok(face) = ttf_parser::Face::parse(data, index) else { return false };
    const CJK: u32 = 0b11111 << 17;
    let declared = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"OS/2"))
        .filter(|t| t.len() >= 82)
        .map_or(0, |t| u32::from_be_bytes([t[78], t[79], t[80], t[81]]));
    let designed = design_languages(&face);
    let writes = |language: &str| {
        let cjk = declared & CJK == 0 || declared & super::cjk::code_pages(language) != 0;
        let design = designed
            .as_ref()
            .is_none_or(|tags| designed_for(tags, language));
        cjk && design
            && letters(language)
                .iter()
                .all(|&c| face.glyph_index(c).is_some())
    };
    languages.iter().any(|l| writes(l)) || (languages.is_empty() && writes("en"))
}

/// The `dlng` entries of a face's `meta` table: the scripts (or languages)
/// it was designed for, like `Latn`, `el`, or `zh-Hans`.
fn design_languages(face: &ttf_parser::Face) -> Option<Vec<String>> {
    let t = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"meta"))?;
    let u32_at = |at: usize| Some(u32::from_be_bytes(t.get(at..at + 4)?.try_into().ok()?) as usize);
    for map in 0..u32_at(12)? {
        let at = 16 + map * 12;
        if t.get(at..at + 4)? == b"dlng" {
            let (off, len) = (u32_at(at + 4)?, u32_at(at + 8)?);
            let tags = std::str::from_utf8(t.get(off..off + len)?).ok()?;
            return Some(tags.split(',').map(|s| s.trim().to_owned()).collect());
        }
    }
    None
}

/// Whether one of `tags` names `language` or a script it's written in.
fn designed_for(tags: &[String], language: &str) -> bool {
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let scripts = scripts(&primary);
    tags.iter().any(|tag| {
        tag.split('-').any(|sub| {
            sub.eq_ignore_ascii_case(&primary)
                || scripts.iter().any(|s| sub.eq_ignore_ascii_case(s))
        })
    })
}

/// The ISO 15924 scripts `language` (a primary subtag) is written in.
fn scripts(language: &str) -> &'static [&'static str] {
    match language {
        "ru" | "uk" | "be" | "bg" | "sr" | "mk" | "kk" => &["Cyrl"],
        "el" => &["Grek"],
        "he" | "yi" => &["Hebr"],
        "ar" | "fa" | "ur" => &["Arab"],
        "hi" | "mr" | "ne" => &["Deva"],
        "th" => &["Thai"],
        "ja" => &["Jpan", "Hira", "Kana", "Hani"],
        "zh" | "yue" => &["Hans", "Hant", "Hani"],
        "ko" => &["Kore", "Hang"],
        _ => &["Latn"],
    }
}

/// A few letters text in `language` can't do without, beyond ASCII.
fn letters(language: &str) -> &'static [char] {
    let primary = language.split(['-', '_']).next().unwrap_or("");
    match primary.to_ascii_lowercase().as_str() {
        "ru" | "uk" | "be" | "bg" | "sr" | "mk" | "kk" => &['ж', 'я'],
        "el" => &['λ', 'ω'],
        "he" | "yi" => &['א', 'ש'],
        "ar" | "fa" | "ur" => &['ب', 'ع'],
        "hi" | "mr" | "ne" => &['क', 'ष'],
        "th" => &['ก', 'ษ'],
        "ja" => &['あ', 'ア'],
        "zh" | "yue" => &['中', '国'],
        "ko" => &['한', '글'],
        "pl" => &['ł', 'ś', 'ż'],
        "cs" => &['ř', 'ě'],
        "hu" => &['ő', 'ű'],
        "tr" | "az" => &['ğ', 'ş', 'ı'],
        "ro" => &['ș', 'ț'],
        "vi" => &['ơ', 'ư', 'ạ'],
        // the Latin-1 letters western European languages share
        _ => &['é', 'ñ', 'ü'],
    }
}

/// `family` as people know it: Apple's own fonts install under hidden names.
pub fn shown_name(family: &str) -> &str {
    match family {
        ".SF NS Mono" | ".SF UI Mono" => "SF Mono",
        ".SF NS" | ".SF UI" => "SF Pro",
        family => family,
    }
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

/// The chosen text family where it's installed, else the system's, where it
/// supplies one.
pub fn sans_family() -> Option<&'static str> {
    super::drawn_name(false).or_else(platform_sans)
}

/// The chosen monospace family where it's installed, else the system's,
/// where it supplies one.
pub fn mono_family() -> Option<&'static str> {
    super::drawn_name(true).or_else(platform_mono)
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
#[cfg(not(any(all(unix, not(target_vendor = "apple")), target_os = "android")))]
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
