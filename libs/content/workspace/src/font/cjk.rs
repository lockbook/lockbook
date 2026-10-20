//! CJK fonts come from the OS: they are tens of megabytes each, and every
//! platform but Linux ships them. Missing files leave CJK as tofu, as before.

use glyphon::cosmic_text::{self, PlatformFallback};
use glyphon::fontdb::{Database, FaceInfo, Language, Style, Weight};
use std::path::PathBuf;
use std::sync::OnceLock;
use unicode_script::Script;

const SC: &[&str] =
    &["PingFang SC", "Hiragino Sans GB", "Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei"];
const TC: &[&str] = &["PingFang TC", "Noto Sans CJK TC", "Noto Sans TC", "Microsoft JhengHei"];
const HK: &[&str] = &["PingFang HK", "Noto Sans CJK HK", "Noto Sans HK"];
const JA: &[&str] = &["Hiragino Sans", "Noto Sans CJK JP", "Noto Sans JP", "Yu Gothic"];
const KO: &[&str] = &["Apple SD Gothic Neo", "Noto Sans CJK KR", "Noto Sans KR", "Malgun Gothic"];
const OTHER: &[&str] = &["WenQuanYi Micro Hei", "WenQuanYi Zen Hei", "Droid Sans Fallback"];

const GROUPS: [&[&str]; 5] = [SC, TC, HK, JA, KO];
const SC_I: usize = 0;
const TC_I: usize = 1;
const HK_I: usize = 2;
const JA_I: usize = 3;
const KO_I: usize = 4;

/// Family names to try, the given group first and its nearest forms next.
fn order(first: usize) -> Vec<&'static str> {
    let groups = match first {
        TC_I => [TC_I, HK_I, SC_I, JA_I, KO_I],
        HK_I => [HK_I, TC_I, SC_I, JA_I, KO_I],
        JA_I => [JA_I, SC_I, TC_I, HK_I, KO_I],
        KO_I => [KO_I, SC_I, TC_I, HK_I, JA_I],
        _ => [SC_I, TC_I, HK_I, JA_I, KO_I],
    };
    groups
        .into_iter()
        .flat_map(|g| GROUPS[g].iter().copied())
        .chain(OTHER.iter().copied())
        .collect()
}

/// The user's preferred languages, from a host that knows them better than
/// `sys-locale`: on Android it reads only the system's first language, not
/// the list or the app's own choice.
static PREFERRED_LANGUAGES: OnceLock<Vec<String>> = OnceLock::new();

/// Call before the first font system is built.
pub fn set_preferred_languages(languages: Vec<String>) {
    let _ = PREFERRED_LANGUAGES.set(languages);
}

/// Han is unified across languages, so its glyph forms follow the first
/// CJK language the user prefers (Simplified Chinese when there is none).
fn han_group() -> usize {
    let group = match PREFERRED_LANGUAGES.get() {
        Some(languages) => languages.iter().find_map(|l| locale_group(l)),
        None => sys_locale::get_locales().find_map(|l| locale_group(&l)),
    };
    group.unwrap_or(SC_I)
}

fn locale_group(locale: &str) -> Option<usize> {
    let locale = locale.to_ascii_lowercase().replace('_', "-");
    let mut subtags = locale.split('-');
    let language = subtags.next()?;
    let rest: Vec<&str> = subtags.collect();
    let has = |tag| rest.contains(&tag);
    match language {
        "ja" => Some(JA_I),
        "ko" => Some(KO_I),
        "zh" | "yue" if has("hans") => Some(SC_I),
        "zh" | "yue" if has("hk") || has("mo") => Some(HK_I),
        "zh" | "yue" if has("hant") || has("tw") => Some(TC_I),
        "yue" => Some(HK_I),
        "zh" => Some(SC_I),
        _ => None,
    }
}

/// The system's CJK faces, found once per process.
fn found() -> &'static Database {
    static FOUND: OnceLock<Database> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut found = Database::new();
        for path in candidates() {
            if path.is_file() {
                let _ = found.load_font_file(&path);
            }
        }
        #[cfg(target_os = "ios")]
        found.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            lb_fonts::NOTO_SANS_SC_REGULAR,
        )));

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

/// Each family found, in fallback order, with its faces nearest regular and bold.
fn families() -> impl Iterator<Item = (&'static FaceInfo, &'static FaceInfo)> {
    order(han_group()).into_iter().filter_map(|family| {
        let faces: Vec<&FaceInfo> = found()
            .faces()
            .filter(|f| f.style == Style::Normal && f.families.iter().any(|(n, _)| n == family))
            .collect();
        let nearest = |w: u16| faces.iter().min_by_key(|f| f.weight.0.abs_diff(w)).copied();
        Some((nearest(400)?, nearest(700)?))
    })
}

/// Appends the system's CJK faces after everything already in `db`: a
/// regular and a bold per family, each also registered as italic, because
/// cosmic-text only falls back to exact style matches.
pub fn load(db: &mut Database) {
    for (regular, bold) in families() {
        for (face, weight) in [(regular, Weight::NORMAL), (bold, Weight::BOLD)] {
            for style in [Style::Normal, Style::Italic] {
                let mut info = face.clone();
                info.weight = weight;
                info.style = style;
                if weight == Weight::BOLD {
                    info.families
                        .push((BOLD_ALIAS.into(), Language::English_UnitedStates));
                }
                db.push_face_info(info);
            }
        }
    }
}

/// Marks the bold copies, which the last-resort fallback must not take:
/// they'd be its only exact match for bold italic, and outrank the bundled
/// fonts for bold symbols. Bold CJK text reaches them by family name.
const BOLD_ALIAS: &str = ".lockbook CJK bold";

/// Each family's regular face as a fallback for every egui text family.
/// The files are mapped, not read, so only the glyphs drawn cost memory.
#[cfg(not(target_family = "wasm"))]
pub fn register_egui(fonts: &mut epaint::text::FontDefinitions, scale: f32) {
    use epaint::text::{FontData, FontFamily, FontTweak};
    use glyphon::fontdb::Source;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::path::Path;

    static MAPPED: OnceLock<Vec<(String, &'static [u8], u32)>> = OnceLock::new();
    let mapped = MAPPED.get_or_init(|| {
        let mut files: HashMap<&Path, &'static [u8]> = HashMap::new();
        families()
            .filter_map(|(face, _)| {
                let path = match &face.source {
                    Source::File(path) => path,
                    Source::Binary(data) => {
                        let data: &'static _ = Box::leak(Box::new(data.clone()));
                        return Some((
                            face.post_script_name.clone(),
                            (**data).as_ref(),
                            face.index,
                        ));
                    }
                    _ => return None,
                };
                let bytes = match files.get(path.as_path()) {
                    Some(bytes) => *bytes,
                    None => {
                        let file = std::fs::File::open(path).ok()?;
                        // SAFETY: system font files don't change under a running app.
                        let map = unsafe { memmap2::Mmap::map(&file) }.ok()?;
                        let bytes: &'static [u8] = Box::leak(Box::new(map));
                        files.insert(path, bytes);
                        bytes
                    }
                };
                Some((face.post_script_name.clone(), bytes, face.index))
            })
            .collect()
    });

    for (name, bytes, index) in mapped {
        let data = FontData {
            font: Cow::Borrowed(bytes),
            index: *index,
            tweak: FontTweak { scale, ..Default::default() },
        };
        fonts.font_data.insert(name.clone(), data.into());
    }
    for (family, list) in fonts.families.iter_mut() {
        if !matches!(family, FontFamily::Name(n) if &**n == "phosphor" || &**n == "Icons") {
            list.extend(mapped.iter().map(|(name, ..)| name.clone()));
        }
    }
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = [
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();

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
    [
        "/System/Library/Fonts/Core/HiraginoKakuGothic.ttc",
        "/System/Library/Fonts/Core/AppleSDGothicNeo.ttc",
    ]
    .into_iter()
    .map(|p| PathBuf::from(format!("{root}{p}")))
    .collect()
}

#[cfg(target_os = "android")]
fn candidates() -> Vec<PathBuf> {
    ["/system/fonts/NotoSansCJK-Regular.ttc", "/system/fonts/NotoSansCJK-Bold.ttc"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
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
    ]
    .into_iter()
    .map(|f| fonts.join(f))
    .collect()
}

#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
fn candidates() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = vec!["/usr/share/fonts".into(), "/usr/local/share/fonts".into()];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        dirs.push(PathBuf::from(&home).join(".fonts"));
    }

    let mut paths = vec![];
    while let Some(dir) = dirs.pop() {
        for path in read_dir(dir) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let cjk = name.contains("CJK")
                || ["NotoSansSC", "NotoSansTC", "NotoSansHK", "NotoSansJP", "NotoSansKR"]
                    .iter()
                    .any(|p| name.starts_with(p))
                || name.starts_with("wqy")
                || name.starts_with("DroidSansFallback");
            if path.is_dir() {
                dirs.push(path);
            } else if cjk {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(not(any(unix, target_os = "windows")))]
fn candidates() -> Vec<PathBuf> {
    vec![]
}

#[cfg(any(
    target_os = "macos",
    all(unix, not(any(target_vendor = "apple", target_os = "android")))
))]
fn read_dir(dir: PathBuf) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
}

/// The platform's fallback lists, with CJK scripts pointed at the faces
/// [`load`] registered.
pub struct Fallback {
    platform: PlatformFallback,
    forbidden: Vec<&'static str>,
    han: Vec<&'static str>,
    bopomofo: Vec<&'static str>,
    ko: Vec<&'static str>,
}

impl Default for Fallback {
    fn default() -> Self {
        let mut forbidden = cosmic_text::Fallback::forbidden_fallback(&PlatformFallback).to_vec();
        forbidden.push(BOLD_ALIAS);
        Self {
            platform: PlatformFallback,
            forbidden,
            han: order(han_group()),
            bopomofo: order(TC_I),
            ko: order(KO_I),
        }
    }
}

impl cosmic_text::Fallback for Fallback {
    fn common_fallback(&self) -> &[&'static str] {
        self.platform.common_fallback()
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &self.forbidden
    }

    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        match script {
            // Kana follow Han so a Japanese line stays in one font.
            Script::Han | Script::Hiragana | Script::Katakana => &self.han,
            Script::Hangul => &self.ko,
            Script::Bopomofo => &self.bopomofo,
            _ => self.platform.script_fallback(script, locale),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_groups() {
        for (locale, group) in [
            ("en-US", None),
            ("ja-JP", Some(JA_I)),
            ("ko_KR", Some(KO_I)),
            ("zh-CN", Some(SC_I)),
            ("zh-Hans-SG", Some(SC_I)),
            ("zh-TW", Some(TC_I)),
            ("zh-Hant", Some(TC_I)),
            ("zh-Hant-HK", Some(HK_I)),
            ("zh_MO", Some(HK_I)),
            ("zh-Hans-HK", Some(SC_I)),
            ("yue-Hant-HK", Some(HK_I)),
            ("kok-IN", None),
        ] {
            assert_eq!(locale_group(locale), group, "{locale}");
        }
    }
}
