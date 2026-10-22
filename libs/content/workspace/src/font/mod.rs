pub mod cjk;
pub mod system;

use epaint::text::FontData;
use glyphon::cosmic_text::{self, PlatformFallback};
use glyphon::fontdb::{Database, Source};
use std::sync::Arc;
use unicode_script::Script;

/// Loads the editor's fonts: the bundled symbols, then text and script faces
/// from the system where it has them, else bundled, then the system's CJK.
pub fn load(db: &mut Database) {
    #[cfg(not(any(target_vendor = "apple", target_os = "android")))]
    {
        db.set_sans_serif_family("Noto Sans");
        db.set_monospace_family("Noto Sans Mono");
        for font in lb_fonts::NOTO {
            db.load_font_source(Source::Binary(Arc::new(font) as _));
        }
    }
    for font in lb_fonts::SYMBOLS {
        db.load_font_source(Source::Binary(Arc::new(font) as _));
    }
    system::load(db);
    cjk::load(db);
}

/// The fonts egui draws text with, and the scale that matches them to the editor's.
pub struct TextFonts {
    pub sans: FontData,
    pub bold: FontData,
    pub italic: FontData,
    pub mono: FontData,
    pub scale: f32,
}

#[cfg(any(target_vendor = "apple", target_os = "android"))]
pub fn text_fonts() -> TextFonts {
    let [sans, bold, italic] = system_sans().expect("system text fonts");
    TextFonts {
        sans,
        bold,
        italic,
        mono: system_mono().expect("system monospace font"),
        scale: if cfg!(target_vendor = "apple") { 0.9 } else { 1. },
    }
}

/// The system's text fonts where it has them, else the bundled Noto.
#[cfg(not(any(target_vendor = "apple", target_os = "android")))]
pub fn text_fonts() -> TextFonts {
    let [sans, bold, italic] = system_sans().unwrap_or_else(|| {
        [lb_fonts::NOTO_SANS_REGULAR, lb_fonts::NOTO_SANS_BOLD, lb_fonts::NOTO_SANS_ITALIC]
            .map(FontData::from_static)
    });
    let mono =
        system_mono().unwrap_or_else(|| FontData::from_static(lb_fonts::NOTO_SANS_MONO_REGULAR));
    TextFonts { sans, bold, italic, mono, scale: 1. }
}

/// The system sans's regular, bold, and italic.
fn system_sans() -> Option<[FontData; 3]> {
    use glyphon::fontdb::Style;
    let sans = system::sans_family()?;
    Some([
        system_font(sans, 400, Style::Normal)?,
        system_font(sans, 700, Style::Normal)?,
        system_font(sans, 400, Style::Italic)?,
    ])
}

fn system_mono() -> Option<FontData> {
    system_font(system::mono_family()?, 400, glyphon::fontdb::Style::Normal)
}

fn system_font(family: &str, weight: u16, style: glyphon::fontdb::Style) -> Option<FontData> {
    use epaint::text::{FontTweak, VariationCoords};
    use glyphon::fontdb::Style;

    let face = system::nearest(family, weight, style)?;
    let mut coords = VariationCoords::new([(b"wght", weight as f32)]);
    if style == Style::Italic && face.style != Style::Italic {
        coords.push(b"ital", 1.);
    }
    // SF's text optical size, as its static Text cut had
    if cfg!(target_vendor = "apple") {
        coords.push(b"opsz", 17.);
    }
    let data = system::face_data(face)?;
    Some(FontData::from_blob(data, face.index).tweak(FontTweak { coords, ..Default::default() }))
}

/// Spaces what the system sans draws at `points` as SF's text optical sizes
/// would: the editor can't select optical size, so it gets SF's display cut,
/// which is tighter below 28pt.
pub fn track<'a>(attrs: glyphon::Attrs<'a>, grapheme: &str, points: f32) -> glyphon::Attrs<'a> {
    use unicode_script::UnicodeScript as _;
    if !cfg!(target_vendor = "apple") || attrs.family != glyphon::Family::SansSerif {
        return attrs;
    }
    let drawn_by_sans = grapheme.chars().all(|c| {
        (c as u32) < 0x2E80
            && matches!(
                c.script(),
                Script::Latin
                    | Script::Greek
                    | Script::Cyrillic
                    | Script::Common
                    | Script::Inherited
            )
    });
    if drawn_by_sans { attrs.letter_spacing(sf_text_spacing(points)) } else { attrs }
}

/// How much wider SF's text cut sets than its display cut, in em, by size.
fn sf_text_spacing(points: f32) -> f32 {
    const MEASURED: [(f32, f32); 7] = [
        (17., 0.0606),
        (19., 0.0551),
        (20., 0.0463),
        (22., 0.0287),
        (24., 0.011),
        (26., 0.0055),
        (28., 0.),
    ];
    let (first, last) = (MEASURED[0], MEASURED[MEASURED.len() - 1]);
    if points <= first.0 {
        return first.1;
    }
    MEASURED
        .windows(2)
        .find(|w| points <= w[1].0)
        .map(|w| w[0].1 + (w[1].1 - w[0].1) * (points - w[0].0) / (w[1].0 - w[0].0))
        .unwrap_or(last.1)
}

/// The platform's fallback lists, with the scripts the system draws pointed
/// at the faces [`load`] registered.
pub struct Fallback {
    platform: PlatformFallback,
    common: Vec<&'static str>,
    forbidden: Vec<&'static str>,
    han: Vec<&'static str>,
    bopomofo: Vec<&'static str>,
    hangul: Vec<&'static str>,
}

impl Default for Fallback {
    fn default() -> Self {
        let platform = PlatformFallback;
        let common = [system::symbol_families(), cosmic_text::Fallback::common_fallback(&platform)]
            .concat()
            .into_iter()
            .filter(|f| !system::last_resort_families().contains(f))
            .collect();
        let mut forbidden = cosmic_text::Fallback::forbidden_fallback(&platform).to_vec();
        forbidden.push(cjk::BOLD_ALIAS);
        Self {
            platform,
            common,
            forbidden,
            han: cjk::han_order(),
            bopomofo: cjk::bopomofo_order(),
            hangul: cjk::hangul_order(),
        }
    }
}

impl cosmic_text::Fallback for Fallback {
    fn common_fallback(&self) -> &[&'static str] {
        &self.common
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &self.forbidden
    }

    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        match script {
            // Kana follow Han so a Japanese line stays in one font.
            Script::Han | Script::Hiragana | Script::Katakana => &self.han,
            Script::Hangul => &self.hangul,
            Script::Bopomofo => &self.bopomofo,
            _ => match system::script_families(script) {
                Some(families) => families,
                None => self.platform.script_fallback(script, locale),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
    use std::collections::BTreeSet;

    /// Characters people type every day, and a word in each script the
    /// bundled fonts used to cover; none may render as tofu.
    const MUST_RENDER: &str = "⌘ ⌥ ⇧ ⌃ ⎋ ⏎ ⌫ ⇥ ← ↑ → ↓ ↔ ⇒ ✓ ✗ ✔ ★ ☆ • ◦ ■ □ ● ○ \
        ± × ÷ ≠ ≤ ≥ ≈ ∞ ∑ √ € £ ¥ ₹ ₩ — – … “ ” ‘ ’ « » ¿ ¡ § ¶ © ® ™ ° \
        ─ │ ┌ ┐ └ ┘ ½ ² ³ Ωμέγα Жизнь café 😀 👍🏽 🎉 \
        مرحبا שלום नमस्ते বাংলা ગુજરાતી ਪੰਜਾਬੀ ಕನ್ನಡ മലയാളം မြန်မာ தமிழ் తెలుగు สวัสดี";

    /// A word in scripts only Linux's system fonts draw, where it has them.
    const SYSTEM_SCRIPTS: &str = "Հայերեն አማርኛ ქართული ភាសាខ្មែរ ລາວ සිංහල བོད་ཡིག";

    /// What the bundled fonts lack: they carry no symbol font. Linux draws
    /// these with the system's fonts where it has them.
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
    const KNOWN_GAPS: &str = "⌘⌥⇧⌃⎋⏎⌫⇥✓✗★☆∑";

    const LINUX: bool = cfg!(all(unix, not(any(target_vendor = "apple", target_os = "android"))));

    fn tofu(fs: &mut FontSystem, text: &str, attrs: Attrs) -> BTreeSet<char> {
        let mut buf = Buffer::new(fs, Metrics::new(16., 20.));
        buf.set_text(text, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);
        buf.layout_runs()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .filter(|g| g.glyph_id == 0)
                    .flat_map(|g| run.text[g.start..g.end].chars())
            })
            .collect()
    }

    /// Scripts and symbols the bundled fonts lack, which Windows ships fonts for.
    #[cfg(target_os = "windows")]
    const WINDOWS_DRAWS: &str = "ភាសាខ្មែរ ລາວ ქართული Հայերեն አማርኛ සිංහල བོད་ཡིག \
        ↵ ⌦ ⇤ ⇪ ⌤ ⎇ ⇞ ⇟ ✘ ☐ ☒ ✦ ✧ ✪ ⟹ ➜ ➔ ↻ ↺ ✎ ⋯ ⋮ ⚑ ⚐ ⌀";
    #[cfg(not(target_os = "windows"))]
    const WINDOWS_DRAWS: &str = "";

    /// The characters of `text` no face in `fs` draws.
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
    fn undrawable(fs: &FontSystem, text: &str) -> BTreeSet<char> {
        let mut missing: BTreeSet<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        let db = fs.db();
        for face in db.faces() {
            db.with_face_data(face.id, |data, index| {
                if let Ok(face) = ttf_parser::Face::parse(data, index) {
                    missing.retain(|&c| face.glyph_index(c).is_none());
                }
            });
        }
        missing
    }

    #[test]
    fn everyday_characters_render() {
        let mut fs = crate::widgets::glyphon_render::new_font_system();
        let text = if LINUX {
            format!("{MUST_RENDER} {SYSTEM_SCRIPTS}")
        } else {
            format!("{MUST_RENDER} {WINDOWS_DRAWS}")
        };
        // what only the system's fonts draw may be missing where it lacks them
        #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
        let allowed = if LINUX {
            undrawable(&fs, &format!("{KNOWN_GAPS} {SYSTEM_SCRIPTS}"))
        } else {
            KNOWN_GAPS.chars().collect()
        };
        #[cfg(any(target_vendor = "apple", target_os = "android", target_os = "windows"))]
        let allowed = BTreeSet::new();
        for (name, attrs) in [
            ("sans", Attrs::new().family(Family::SansSerif)),
            ("bold", Attrs::new().family(Family::SansSerif).weight(Weight::BOLD)),
            ("mono", Attrs::new().family(Family::Monospace)),
        ] {
            let new: String = tofu(&mut fs, &text, attrs).difference(&allowed).collect();
            assert!(new.is_empty(), "{name} renders tofu for {new}");
        }
    }
}
