//! CJK fonts come from the OS: they are tens of megabytes each, and every
//! platform but Linux ships them. Missing ones leave CJK as tofu, as before.

use super::system;
use glyphon::fontdb::{Database, FaceInfo, Language, Style, Weight};
use std::sync::OnceLock;

const SC: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans GB",
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "Microsoft YaHei",
];
const TC: &[&str] = &[
    "PingFang TC",
    "Noto Sans CJK TC",
    "Noto Sans TC",
    "Source Han Sans TC",
    "Microsoft JhengHei",
];
const HK: &[&str] = &["PingFang HK", "Noto Sans CJK HK", "Noto Sans HK", "Source Han Sans HC"];
const JA: &[&str] =
    &["Hiragino Sans", "Noto Sans CJK JP", "Noto Sans JP", "Source Han Sans", "Yu Gothic"];
const KO: &[&str] = &[
    "Apple SD Gothic Neo",
    "Noto Sans CJK KR",
    "Noto Sans KR",
    "Source Han Sans K",
    "Malgun Gothic",
];
const OTHER: &[&str] = &["WenQuanYi Micro Hei", "WenQuanYi Zen Hei", "Droid Sans Fallback"];

const GROUPS: [&[&str]; 5] = [SC, TC, HK, JA, KO];
const SC_I: usize = 0;
const TC_I: usize = 1;
const HK_I: usize = 2;
const JA_I: usize = 3;
const KO_I: usize = 4;

/// Han and kana's fallback families, in the user's preferred forms.
pub(super) fn han_order() -> Vec<&'static str> {
    order(han_group())
}

/// Bopomofo's fallback families, Traditional forms first.
pub(super) fn bopomofo_order() -> Vec<&'static str> {
    order(TC_I)
}

/// Hangul's fallback families, Korean forms first.
pub(super) fn hangul_order() -> Vec<&'static str> {
    order(KO_I)
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

/// Each family found, in fallback order, with its faces nearest regular and bold.
fn families() -> impl Iterator<Item = (&'static FaceInfo, &'static FaceInfo)> {
    order(han_group()).into_iter().filter_map(|family| {
        let faces: Vec<&FaceInfo> = system::faces(family)
            .filter(|f| f.style == Style::Normal)
            .collect();
        let nearest = |w: u16| faces.iter().min_by_key(|f| f.weight.0.abs_diff(w)).copied();
        Some((nearest(400)?, nearest(700)?))
    })
}

/// Appends the system's CJK faces after everything already in `db`: a
/// regular and a bold per family, as script fallback only takes exact
/// weights of static faces.
pub fn load(db: &mut Database) {
    for (regular, bold) in families() {
        for (face, weight) in [(regular, Weight::NORMAL), (bold, Weight::BOLD)] {
            let mut info = system::shared(face);
            info.weight = weight;
            if weight == Weight::BOLD {
                info.families
                    .push((BOLD_ALIAS.into(), Language::English_UnitedStates));
            }
            db.push_face_info(info);
        }
    }
}

/// Marks the bold copies, which the last-resort fallback must not take:
/// they'd outrank the other fonts for bold symbols. Bold CJK text reaches
/// them by family name.
pub(super) const BOLD_ALIAS: &str = ".lockbook CJK bold";

/// Each family's regular face as a fallback for every egui text family.
pub fn register_egui(fonts: &mut epaint::text::FontDefinitions, scale: f32) {
    use epaint::text::{FontData, FontFamily, FontTweak};

    let regulars: Vec<_> = families()
        .filter_map(|(face, _)| {
            Some((face.post_script_name.clone(), system::face_data(face)?, face.index))
        })
        .collect();
    for (name, data, index) in &regulars {
        let data = FontData::from_blob(data.clone(), *index)
            .tweak(FontTweak { scale, ..Default::default() });
        fonts.font_data.insert(name.clone(), data.into());
    }
    for (family, list) in fonts.families.iter_mut() {
        if !matches!(family, FontFamily::Name(n) if &**n == "phosphor" || &**n == "Icons") {
            list.extend(regulars.iter().map(|(name, ..)| name.clone()));
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
