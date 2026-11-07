//! CJK fonts come from the OS: they are tens of megabytes each, and every
//! platform but Linux ships them. Missing ones leave CJK as tofu, as before.

use glyphon::fontdb::{Database, FaceInfo, Language, Style, Weight};
use std::sync::OnceLock;

const SC: &[&str] = &["Noto Sans CJK SC", "Noto Sans SC", "Source Han Sans SC"];
const TC: &[&str] = &["Noto Sans CJK TC", "Noto Sans TC", "Source Han Sans TC"];
const HK: &[&str] = &["Noto Sans CJK HK", "Noto Sans HK", "Source Han Sans HC"];
const JA: &[&str] = &["Noto Sans CJK JP", "Noto Sans JP", "Source Han Sans"];
const KO: &[&str] = &["Noto Sans CJK KR", "Noto Sans KR", "Source Han Sans K"];
const OTHER: &[&str] = &["WenQuanYi Micro Hei", "WenQuanYi Zen Hei", "Droid Sans Fallback"];

const GROUPS: [&[&str]; 5] = [SC, TC, HK, JA, KO];
const SC_I: usize = 0;
const TC_I: usize = 1;
const HK_I: usize = 2;
const JA_I: usize = 3;
const KO_I: usize = 4;

/// Han and kana's fallback families, in the user's preferred forms.
pub(super) fn han_order() -> &'static [&'static str] {
    static ORDER: OnceLock<Vec<&'static str>> = OnceLock::new();
    ORDER.get_or_init(|| order(han_group()))
}

/// Bopomofo's fallback families, Traditional forms first.
#[cfg(not(any(target_vendor = "apple", windows)))]
pub(super) fn bopomofo_order() -> &'static [&'static str] {
    static ORDER: OnceLock<Vec<&'static str>> = OnceLock::new();
    ORDER.get_or_init(|| order(TC_I))
}

/// Hangul's fallback families, Korean forms first.
#[cfg(not(any(target_vendor = "apple", windows)))]
pub(super) fn hangul_order() -> &'static [&'static str] {
    static ORDER: OnceLock<Vec<&'static str>> = OnceLock::new();
    ORDER.get_or_init(|| order(KO_I))
}

/// The language whose forms Han and kana are asked in: the first CJK one the
/// user prefers.
pub(super) fn han_language() -> &'static str {
    match han_group() {
        TC_I => "zh-Hant",
        HK_I => "zh-HK",
        JA_I => "ja",
        KO_I => "ko",
        _ => "zh-Hans",
    }
}

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
    languages()
        .iter()
        .find_map(|l| locale_group(l))
        .unwrap_or(SC_I)
}

/// The languages the user reads, most preferred first.
pub(super) fn languages() -> Vec<String> {
    match PREFERRED_LANGUAGES.get() {
        Some(languages) => languages.clone(),
        None => sys_locale::get_locales().collect(),
    }
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

/// Whether `family` is one of the CJK fallback families.
pub(super) fn is_cjk(family: &str) -> bool {
    GROUPS
        .iter()
        .chain([&OTHER])
        .any(|group| group.contains(&family))
}

/// Appends a CJK family's `faces` to `db`: a regular and a bold, as script
/// fallback only takes exact weights of static faces.
pub(super) fn add(db: &mut Database, faces: &[FaceInfo]) {
    let upright: Vec<&FaceInfo> = faces.iter().filter(|f| f.style == Style::Normal).collect();
    let nearest = |w: u16| {
        upright
            .iter()
            .min_by_key(|f| f.weight.0.abs_diff(w))
            .copied()
    };
    let (Some(regular), Some(bold)) = (nearest(400), nearest(700)) else { return };
    for (face, weight) in [(regular, Weight::NORMAL), (bold, Weight::BOLD)] {
        let mut info = face.clone();
        info.weight = weight;
        if weight == Weight::BOLD {
            info.families
                .push((BOLD_ALIAS.into(), Language::English_UnitedStates));
        }
        db.push_face_info(info);
    }
}

/// Marks the bold copies, which the last-resort fallback must not take:
/// they'd outrank the other fonts for bold symbols. Bold CJK text reaches
/// them by family name.
pub(super) const BOLD_ALIAS: &str = ".lockbook CJK bold";

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
