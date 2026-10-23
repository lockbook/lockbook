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

/// The system's drawable faces, found once per process.
pub fn found() -> &'static Database {
    static FOUND: OnceLock<Database> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut found = Database::new();
        // fontconfig's directories, wherever the distribution puts them
        #[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
        found.load_system_fonts();
        for path in candidates() {
            if path.is_dir() {
                found.load_fonts_dir(&path);
            } else if path.is_file() {
                let _ = found.load_font_file(&path);
            }
        }
        #[cfg(target_os = "ios")]
        found.load_font_source(Source::Binary(Arc::new(lb_fonts::NOTO_SANS_SC_REGULAR)));

        // Swash can't draw Apple's `hvgl` outlines (as iOS 26's PingFang has), and
        // a face it shapes but can't draw leaves invisible text.
        let undrawable: Vec<_> = found
            .faces()
            .filter(|f| found.with_face_data(f.id, has_outlines) != Some(true))
            .map(|f| f.id)
            .collect();
        for id in undrawable {
            found.remove_face(id);
        }
        found
    })
}

fn has_outlines(data: &[u8], index: u32) -> bool {
    ttf_parser::RawFace::parse(data, index).is_ok_and(|face| {
        [b"glyf", b"CFF ", b"CFF2"]
            .iter()
            .any(|tag| face.table(ttf_parser::Tag::from_bytes(tag)).is_some())
    })
}

/// The faces of `family`, in the order found.
pub fn faces(family: &str) -> impl Iterator<Item = &'static FaceInfo> + '_ {
    found()
        .faces()
        .filter(move |f| f.families.iter().any(|(n, _)| n == family))
}

/// The first of `families` the system has.
fn first_present(families: &[&'static str]) -> Option<&'static str> {
    families
        .iter()
        .copied()
        .find(|family| faces(family).next().is_some())
}

/// The system's text family, where it supplies one.
pub fn sans_family() -> Option<&'static str> {
    first_present(SANS)
}

/// The system's monospace family, where it supplies one.
pub fn mono_family() -> Option<&'static str> {
    first_present(MONO)
}

/// Appends the system's faces after everything in `db`: on Linux those its
/// fallback lists name, for whatever the bundled fonts lack; elsewhere text,
/// script, and symbol faces.
pub fn load(db: &mut Database) {
    #[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
    load_fallbacks(db);
    #[cfg(not(all(unix, not(any(target_vendor = "apple", target_os = "android")))))]
    load_families(db);
}

/// Loads the families cosmic-text's Linux lists name, so a font system reads
/// a few dozen faces rather than every one installed.
#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
fn load_fallbacks(db: &mut Database) {
    use glyphon::cosmic_text::{Fallback, PlatformFallback};
    use glyphon::fontdb::Weight;

    let family = |f: &FaceInfo| f.families[0].0.clone();
    let platform = &PlatformFallback;
    let scripts = FALLBACK_SCRIPTS
        .iter()
        .flat_map(|&script| platform.script_fallback(script, ""));
    let names = scripts.chain(platform.common_fallback()).copied();
    for name in names {
        // bundled already, or drawn in color Swash can't
        if name == "Noto Color Emoji" || db.faces().any(|f| family(f) == name) {
            continue;
        }
        // monospaced faces would join the monospace fallback, which takes any
        // that covers a word ahead of the bundled script fonts
        for face in faces(name).filter(|f| !f.monospaced) {
            db.push_face_info(shared(face));
        }
    }

    // fallback takes only faces of the weight asked for, so bold text
    // reaches those without a bold face through a bold copy
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

/// The scripts whose Linux fallback fonts are loaded. Han, kana, and Hangul
/// are [`super::cjk`]'s.
#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
const FALLBACK_SCRIPTS: &[Script] = &[
    Script::Adlam,
    Script::Arabic,
    Script::Armenian,
    Script::Bengali,
    Script::Cherokee,
    Script::Devanagari,
    Script::Ethiopic,
    Script::Georgian,
    Script::Gujarati,
    Script::Gurmukhi,
    Script::Hebrew,
    Script::Javanese,
    Script::Kannada,
    Script::Khmer,
    Script::Lao,
    Script::Malayalam,
    Script::Mongolian,
    Script::Myanmar,
    Script::Oriya,
    Script::Sinhala,
    Script::Syriac,
    Script::Tamil,
    Script::Telugu,
    Script::Thaana,
    Script::Thai,
    Script::Tibetan,
    Script::Tifinagh,
    Script::Vai,
];

#[cfg(not(all(unix, not(any(target_vendor = "apple", target_os = "android")))))]
fn load_families(db: &mut Database) {
    let (sans, mono) = (sans_family(), mono_family());
    if let Some(sans) = sans {
        db.set_sans_serif_family(sans);
    }
    if let Some(mono) = mono {
        db.set_monospace_family(mono);
    }

    // The last resort ahead of the script faces, so where no list names a
    // font, it wins the remaining faces; the script lists reach theirs anyway.
    let scripts = SCRIPTS
        .iter()
        .flat_map(|(_, families)| families.iter().copied());
    let families = sans
        .into_iter()
        .chain(mono)
        .chain(LAST_RESORT.iter().copied())
        .chain(scripts)
        .chain(SYMBOLS.iter().copied());
    let mut loaded = vec![];
    for family in families {
        if !loaded.contains(&family) {
            loaded.push(family);
            for face in faces(family) {
                db.push_face_info(shared(face));
            }
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

/// Families loaded but kept out of every fallback list, so they draw only
/// what the bundled fonts, emoji included, lack.
pub fn last_resort_families() -> &'static [&'static str] {
    LAST_RESORT
}

/// The face of `family` nearest `weight` in `style`, falling back to normal.
pub fn nearest(family: &str, weight: u16, style: Style) -> Option<&'static FaceInfo> {
    let in_style = |s: Style| {
        faces(family)
            .filter(move |f| f.style == s)
            .min_by_key(|f| f.weight.0.abs_diff(weight))
    };
    in_style(style).or_else(|| in_style(Style::Normal))
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
    let mut mapped = MAPPED.get_or_init(Default::default).lock().unwrap();
    if let Some(data) = mapped.get(path) {
        return Some(data.clone());
    }
    let data: Blob = Arc::new(map(path)?);
    mapped.insert(path.clone(), data.clone());
    Some(data)
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

/// What the system draws each script with. Han, kana, and Hangul are [`super::cjk`]'s.
#[cfg(target_vendor = "apple")]
const SCRIPTS: &[(Script, &[&str])] = &[
    (Script::Arabic, &[".SF Arabic", "Geeza Pro"]),
    (Script::Armenian, &[".SF Armenian", "Noto Sans Armenian"]),
    (Script::Bengali, &["Kohinoor Bangla", ".SF Bangla"]),
    (Script::Cherokee, &["Galvji", "Plantagenet Cherokee"]),
    (Script::Devanagari, &["Kohinoor Devanagari", ".SF Devanagari"]),
    (Script::Ethiopic, &["Kefa III", "Kefa"]),
    (Script::Georgian, &[".SF Georgian"]),
    (Script::Gujarati, &["Kohinoor Gujarati", ".SF Gujarati"]),
    (Script::Gurmukhi, &["Mukta Mahee", ".SF Gurmukhi"]),
    (Script::Hebrew, &[".SF Hebrew", "Arial Hebrew"]),
    (Script::Kannada, &["Noto Sans Kannada", ".SF Kannada"]),
    (Script::Khmer, &["Khmer Sangam MN"]),
    (Script::Lao, &["Lao Sangam MN"]),
    (Script::Malayalam, &["Malayalam Sangam MN", ".SF Malayalam"]),
    (Script::Mongolian, &["Noto Sans Mongolian"]),
    (Script::Myanmar, &["Noto Sans Myanmar", "Myanmar Sangam MN"]),
    (Script::Oriya, &["Noto Sans Oriya", ".SF Odia"]),
    (Script::Sinhala, &["Sinhala Sangam MN"]),
    (Script::Syriac, &["Noto Sans Syriac"]),
    (Script::Tamil, &["Tamil Sangam MN", ".SF Tamil"]),
    (Script::Telugu, &["Kohinoor Telugu", ".SF Telugu"]),
    (Script::Thaana, &["Noto Sans Thaana"]),
    (Script::Thai, &["Thonburi"]),
    (Script::Tibetan, &["Kailasa"]),
];
#[cfg(target_vendor = "apple")]
const SYMBOLS: &[&str] = &["Apple Symbols", "Menlo"];

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

/// The bundled Noto follows each system font, for systems stripped of it.
#[cfg(target_os = "windows")]
const SCRIPTS: &[(Script, &[&str])] = &[
    (Script::Adlam, &["Ebrima"]),
    (Script::Arabic, &["Segoe UI", "Noto Sans Arabic"]),
    (Script::Armenian, &["Segoe UI", "Sylfaen"]),
    (Script::Bengali, &["Nirmala UI", "Noto Sans Bengali"]),
    (Script::Canadian_Aboriginal, &["Gadugi"]),
    (Script::Cherokee, &["Gadugi"]),
    (Script::Devanagari, &["Nirmala UI", "Noto Sans Devanagari"]),
    (Script::Ethiopic, &["Ebrima"]),
    (Script::Georgian, &["Segoe UI", "Sylfaen"]),
    (Script::Gujarati, &["Nirmala UI", "Noto Sans Gujarati"]),
    (Script::Gurmukhi, &["Nirmala UI", "Noto Sans Gurmukhi"]),
    (Script::Hebrew, &["Segoe UI", "Noto Sans Hebrew"]),
    (Script::Javanese, &["Javanese Text"]),
    (Script::Kannada, &["Nirmala UI", "Noto Sans Kannada"]),
    (Script::Khmer, &["Leelawadee UI"]),
    (Script::Lao, &["Leelawadee UI"]),
    (Script::Malayalam, &["Nirmala UI", "Noto Sans Malayalam"]),
    (Script::Mongolian, &["Mongolian Baiti"]),
    (Script::Myanmar, &["Myanmar Text", "Noto Sans Myanmar"]),
    (Script::Nko, &["Ebrima"]),
    (Script::Oriya, &["Nirmala UI"]),
    (Script::Sinhala, &["Nirmala UI"]),
    (Script::Syriac, &["Segoe UI Historic"]),
    (Script::Tamil, &["Nirmala UI", "Noto Sans Tamil"]),
    (Script::Telugu, &["Nirmala UI", "Noto Sans Telugu"]),
    (Script::Thaana, &["MV Boli"]),
    (Script::Thai, &["Leelawadee UI", "Noto Sans Thai"]),
    (Script::Tibetan, &["Microsoft Himalaya"]),
    (Script::Tifinagh, &["Ebrima"]),
    (Script::Vai, &["Ebrima"]),
    (Script::Yi, &["Microsoft Yi Baiti"]),
];

// Segoe UI Symbol has monochrome forms of many emoji; behind Twemoji, it only
// fills the symbols Twemoji lacks.
#[cfg(target_os = "windows")]
const LAST_RESORT: &[&str] = &["Segoe UI Symbol"];
#[cfg(not(target_os = "windows"))]
const LAST_RESORT: &[&str] = &[];

#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
const SCRIPTS: &[(Script, &[&str])] = &[];
/// The bundled text and emoji faces, ahead of the system's for what a
/// script's fonts lack, as before the system's were loaded: DejaVu Sans
/// would take symbols Noto Sans Mono draws, and draw emoji in monochrome.
#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
const SYMBOLS: &[&str] = &["Noto Sans", "Noto Sans Mono", "Twemoji Mozilla"];
#[cfg(not(unix))]
const SYMBOLS: &[&str] = &[];

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/System/Library/Fonts")];

    // PingFang is a downloaded asset under a hashed directory.
    for assets in read_dir("/System/Library/AssetsV2".into()) {
        let is_font = assets.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .starts_with("com_apple_MobileAsset_Font")
        });
        if is_font {
            for asset in read_dir(assets) {
                paths.push(asset.join("AssetData/PingFang.ttc"));
            }
        }
    }
    paths
}

#[cfg(target_os = "ios")]
fn candidates() -> Vec<PathBuf> {
    // Simulator processes see the host's /System; the runtime's is here.
    let root = std::env::var("IPHONE_SIMULATOR_ROOT").unwrap_or_default();
    vec![PathBuf::from(format!("{root}/System/Library/Fonts/Core"))]
}

#[cfg(target_os = "android")]
fn candidates() -> Vec<PathBuf> {
    vec!["/system/fonts".into()]
}

#[cfg(target_os = "windows")]
fn candidates() -> Vec<PathBuf> {
    let fonts =
        PathBuf::from(std::env::var("WINDIR").unwrap_or("C:\\Windows".into())).join("Fonts");
    [
        "msyh.ttc",
        "msyhbd.ttc",
        "msjh.ttc",
        "msjhbd.ttc",
        "YuGothR.ttc",
        "YuGothB.ttc",
        "malgun.ttf",
        "malgunbd.ttf",
        "segoeui.ttf",
        "segoeuib.ttf",
        "segoeuii.ttf",
        "segoeuiz.ttf",
        "CascadiaMono.ttf",
        "consola.ttf",
        "consolab.ttf",
        "consolai.ttf",
        "consolaz.ttf",
        "ebrima.ttf",
        "ebrimabd.ttf",
        "gadugi.ttf",
        "gadugib.ttf",
        "himalaya.ttf",
        "javatext.ttf",
        "LeelawUI.ttf",
        "LeelaUIb.ttf",
        "mmrtext.ttf",
        "mmrtextb.ttf",
        "monbaiti.ttf",
        "msyi.ttf",
        "mvboli.ttf",
        "Nirmala.ttc",
        // Windows 10's Nirmala UI, before it became a collection
        "Nirmala.ttf",
        "NirmalaB.ttf",
        "seguihis.ttf",
        "seguisym.ttf",
        "sylfaen.ttf",
    ]
    .into_iter()
    .map(|f| fonts.join(f))
    .collect()
}

#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
fn candidates() -> Vec<PathBuf> {
    vec![]
}

#[cfg(target_os = "macos")]
fn read_dir(dir: PathBuf) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
}
