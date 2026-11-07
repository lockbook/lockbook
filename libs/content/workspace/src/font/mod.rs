pub mod cjk;
pub mod loader;
#[cfg(any(target_vendor = "apple", windows))]
pub mod os;
pub mod system;

use epaint::text::FontData;
use glyphon::cosmic_text::{self, PlatformFallback};
use glyphon::fontdb::{Database, FaceInfo, Source};
use loader::{Family, Kind, Load};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use unicode_script::Script;

static CHOSEN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Adds the fallback families text asked for once they're read off the UI
/// thread, redrawing `ctx`'s notes; until then text draws as before. Called
/// each frame.
pub fn poll(ctx: &egui::Context) {
    settle_interface(ctx);
    let added = settle_fallbacks();
    if loader::busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    if added {
        CHOSEN_SEQ.fetch_add(1, Ordering::Release);
        crate::widgets::glyphon_render::reload_font_system(ctx);
    }
}

/// How much smaller than its size the interface draws its text, which egui
/// sizes differently from the editor.
pub fn interface_scale() -> f32 {
    if cfg!(target_vendor = "apple") { 0.9 } else { 1. }
}

/// A family to fall back to, as [`loader::get`] reads it: by name, or as
/// the one the system draws a character in.
type Want = (String, Kind);

/// What text asked to fall back to, in the order asked: note text's, and
/// the interface's, which [`poll`] adds to egui; and how many of note
/// text's were resolved, and read, as of the last [`poll`].
struct Fallbacks {
    notes: Vec<Want>,
    interface: Vec<Want>,
    resolved: usize,
    read: usize,
}

static FALLBACKS: Mutex<Fallbacks> =
    Mutex::new(Fallbacks { notes: vec![], interface: vec![], resolved: 0, read: 0 });

fn want(list: &mut Vec<Want>, want: Want) {
    if !list.contains(&want) {
        loader::get(&want.0, want.1);
        list.push(want);
    }
}

/// Starts reading `families` for note text to fall back to, once each.
fn want_fallbacks(families: &[&'static str]) {
    let mut fallbacks = FALLBACKS.lock().unwrap();
    for &family in families {
        // drawn in color Swash can't
        if family != "Noto Color Emoji" {
            want(&mut fallbacks.notes, (family.to_owned(), Kind::Script));
        }
    }
}

/// What the loader reads as the family the system draws `c` in.
fn system_fallback(c: char) -> Want {
    (format!("{c}\t{}", fallback_language(c)), Kind::Fallback)
}

/// The language the system picks `c`'s font for: CJK by the user's
/// preferred forms, everything else by their first language.
fn fallback_language(c: char) -> String {
    use unicode_script::UnicodeScript as _;
    match c.script() {
        Script::Han | Script::Hiragana | Script::Katakana => cjk::han_language().into(),
        Script::Hangul => "ko".into(),
        Script::Bopomofo => "zh-Hant".into(),
        _ => cjk::languages()
            .first()
            .cloned()
            .unwrap_or_else(|| "en".into()),
    }
}

/// Starts reading the families the system draws `chars` in, which note
/// text just laid out with no font drawing them; they show as tofu until
/// [`poll`] adds them. Only where the system answers, macOS and Windows.
pub(crate) fn missing(chars: impl Iterator<Item = char>) {
    if cfg!(any(target_vendor = "apple", windows)) {
        let mut fallbacks = FALLBACKS.lock().unwrap();
        for c in chars {
            want(&mut fallbacks.notes, system_fallback(c));
        }
    }
}

/// The families of `wants` read so far, and how many are resolved.
fn read(wants: &[Want]) -> (Vec<Family>, usize) {
    let mut read = vec![];
    let mut resolved = 0;
    for (name, kind) in wants {
        match loader::get(name, *kind) {
            Load::Ready(family) => {
                read.push(family);
                resolved += 1;
            }
            Load::Missing => resolved += 1,
            Load::Loading => {}
        }
    }
    (read, resolved)
}

/// Note text's fallback families read so far.
fn read_fallbacks() -> Vec<Family> {
    let notes = FALLBACKS.lock().unwrap().notes.clone();
    read(&notes).0
}

/// Whether more of note text's fallback families were read since the last
/// call, once all asked for are resolved, so a batch redraws once.
fn settle_fallbacks() -> bool {
    let notes = {
        let fallbacks = FALLBACKS.lock().unwrap();
        if fallbacks.resolved == fallbacks.notes.len() {
            return false;
        }
        fallbacks.notes.clone()
    };
    let (read, resolved) = read(&notes);
    if resolved < notes.len() {
        return false;
    }
    let mut fallbacks = FALLBACKS.lock().unwrap();
    let added = read.len() > fallbacks.read;
    fallbacks.read = read.len();
    fallbacks.resolved = resolved;
    added
}

/// Starts reading a font for interface text egui finds no font for; [`poll`]
/// adds it once it's read. The system names one where it answers; elsewhere
/// the CJK families, which are what the bundled fonts lack.
struct InterfaceFallback;

impl epaint::text::FontProvider for InterfaceFallback {
    fn font_for(
        &self, request: &epaint::text::FallbackRequest<'_>,
    ) -> Option<epaint::text::FontInsert> {
        let c = request.cluster.chars().next()?;
        let mut fallbacks = FALLBACKS.lock().unwrap();
        if cfg!(any(target_vendor = "apple", windows)) {
            want(&mut fallbacks.interface, system_fallback(c));
        } else {
            for family in cjk::han_order() {
                want(&mut fallbacks.interface, ((*family).to_owned(), Kind::Script));
            }
        }
        None
    }
}

/// Gives `ctx` the [`InterfaceFallback`], and adds the families it asked
/// for, once read, behind every text family of `ctx`'s interface.
fn settle_interface(ctx: &egui::Context) {
    type Added = std::collections::HashSet<&'static str>;
    let id = egui::Id::unique("interface fallbacks");
    let Some(mut added) = ctx.data(|d| d.get_temp::<Added>(id)) else {
        ctx.add_font_provider(Arc::new(InterfaceFallback));
        ctx.data_mut(|d| d.insert_temp(id, Added::new()));
        return;
    };
    let wants = FALLBACKS.lock().unwrap().interface.clone();
    let (read, _) = read(&wants);
    let new: Vec<Family> = read
        .into_iter()
        .filter(|family| added.insert(family.name))
        .collect();
    if new.is_empty() {
        return;
    }
    let families: Vec<epaint::text::FontFamily> = ctx.fonts(|f| {
        f.definitions()
            .families
            .keys()
            .filter(|family| {
                !matches!(family, epaint::text::FontFamily::Name(n) if &**n == "phosphor" || &**n == "Icons")
            })
            .cloned()
            .collect()
    });
    for family in new {
        if let Some(font) = egui_font(&family.faces, &families) {
            ctx.add_font(font);
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, added));
}

/// A family's regular face as a fallback for each of egui's `families`.
fn egui_font(
    faces: &[FaceInfo], families: &[epaint::text::FontFamily],
) -> Option<epaint::text::FontInsert> {
    use epaint::text::{FontInsert, FontPriority, FontTweak, InsertFontFamily};
    use glyphon::fontdb::Style;
    let face = faces
        .iter()
        .filter(|f| f.style == Style::Normal)
        .min_by_key(|f| f.weight.0.abs_diff(400))?;
    let data = FontData::from_blob(system::face_data(face)?, face.index)
        .tweak(FontTweak { scale: interface_scale(), ..Default::default() });
    let families = families
        .iter()
        .map(|family| InsertFontFamily { family: family.clone(), priority: FontPriority::Lowest })
        .collect();
    Some(FontInsert::new(&face.post_script_name, data, families))
}

/// The height of `letter` (`x`, or `d` for its ascenders) in a family's
/// regular face among `faces`, in em, else its declared lowercase (or
/// capital) height. Reads the font file, so only [`loader`] calls it.
fn letter_height(faces: &[FaceInfo], letter: char) -> Option<f32> {
    use glyphon::fontdb::Style;
    let face = faces
        .iter()
        .min_by_key(|f| (f.style != Style::Normal, f.weight.0.abs_diff(400)))?;
    face_letter_height((*system::face_data(face)?).as_ref(), face.index, letter)
}

fn face_letter_height(data: &[u8], index: u32, letter: char) -> Option<f32> {
    let face = ttf_parser::Face::parse(data, index).ok()?;
    let drawn = face
        .glyph_index(letter)
        .and_then(|g| face.glyph_bounding_box(g))
        .map(|b| b.y_max);
    let declared = if letter == 'x' { face.x_height() } else { face.capital_height() };
    let height = drawn.or(declared).filter(|h| *h > 0)?;
    Some(height as f32 / face.units_per_em() as f32)
}

/// Changes with each [`poll`] that redraws in new families, so text measured
/// before it can be told apart.
pub fn chosen_seq() -> u64 {
    CHOSEN_SEQ.load(Ordering::Acquire)
}

/// Adds the fallback families read so far to `db`.
pub fn refresh(db: &mut Database) {
    for family in read_fallbacks() {
        add_fallback(db, &family);
    }
}

/// Adds a fallback `family`'s faces to `db` where it lacks them.
fn add_fallback(db: &mut Database, family: &Family) {
    if has_family(db, family.name) {
        return;
    }
    if cjk::is_cjk(family.name) {
        return cjk::add(db, &family.faces);
    }
    for face in family.faces.iter() {
        // monospaced faces would join the monospace fallback, which takes
        // any that covers a word ahead of the bundled script fonts
        if face.monospaced
            && cfg!(all(unix, not(any(target_vendor = "apple", target_os = "android"))))
        {
            continue;
        }
        db.push_face_info(face.clone());
    }
}

fn has_family(db: &Database, family: &str) -> bool {
    db.faces()
        .any(|f| f.families.iter().any(|(n, _)| n == family))
}

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
    refresh(db);
}

/// The fonts egui draws text with and the scale that matches them to the
/// editor's.
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
        scale: interface_scale(),
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
    let sans = system::platform_sans()?;
    Some([
        system_font(sans, 400, Style::Normal)?,
        system_font(sans, 700, Style::Normal)?,
        system_font(sans, 400, Style::Italic)?,
    ])
}

fn system_mono() -> Option<FontData> {
    system_font(system::platform_mono()?, 400, glyphon::fontdb::Style::Normal)
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
    if cfg!(target_vendor = "apple") && family.starts_with(".SF") {
        coords.push(b"opsz", 17.);
    }
    let data = system::face_data(face)?;
    Some(FontData::from_blob(data, face.index).tweak(FontTweak { coords, ..Default::default() }))
}

/// Spaces what the system sans draws at `points` as CoreText's system font
/// would: the editor can't select optical size or apply SF's size-specific
/// tracking, so it gets SF's display cut, which is tighter.
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

/// How much wider CoreText sets SF than its display cut does, in em, by size.
fn sf_text_spacing(points: f32) -> f32 {
    const MEASURED: [(f32, f32); 16] = [
        (11., 0.0654),
        (12., 0.0595),
        (13., 0.0537),
        (14., 0.0488),
        (15., 0.0439),
        (16., 0.0400),
        (17., 0.0341),
        (18., 0.0323),
        (19., 0.0306),
        (20., 0.0227),
        (21., 0.0193),
        (22., 0.0158),
        (23., 0.0143),
        (24., 0.0128),
        (28., 0.0125),
        (34., 0.0101),
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
    #[cfg(not(any(target_vendor = "apple", windows)))]
    platform: PlatformFallback,
    common: Vec<&'static str>,
    forbidden: Vec<&'static str>,
}

impl Default for Fallback {
    fn default() -> Self {
        let platform = PlatformFallback;
        // where the system answers per script, its common fonts would only
        // outrank the bundled emoji
        let common: Vec<&'static str> = if cfg!(any(target_vendor = "apple", windows)) {
            vec![]
        } else {
            [system::symbol_families(), cosmic_text::Fallback::common_fallback(&platform)].concat()
        };
        let mut forbidden = cosmic_text::Fallback::forbidden_fallback(&platform).to_vec();
        forbidden.push(cjk::BOLD_ALIAS);
        // asked for once each font system lists them
        want_fallbacks(&common);
        Self {
            #[cfg(not(any(target_vendor = "apple", windows)))]
            platform,
            common,
            forbidden,
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

    /// Asked the first time a font system lays out text in `script`, which
    /// starts reading its families: the one the system names where it
    /// answers, else the lists below.
    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        #[cfg(any(target_vendor = "apple", windows))]
        let families = {
            let _ = locale;
            system_script_families(script)
        };
        #[cfg(not(any(target_vendor = "apple", windows)))]
        let families = match script {
            // Kana follow Han so a Japanese line stays in one font.
            Script::Han | Script::Hiragana | Script::Katakana => cjk::han_order(),
            Script::Hangul => cjk::hangul_order(),
            Script::Bopomofo => cjk::bopomofo_order(),
            _ => match system::script_families(script) {
                Some(families) => families,
                None => self.platform.script_fallback(script, locale),
            },
        };
        want_fallbacks(families);
        families
    }
}

/// The family the system draws `script` in, found once per script from a
/// letter written in it, which the loader then reads.
#[cfg(any(target_vendor = "apple", windows))]
fn system_script_families(script: Script) -> &'static [&'static str] {
    use std::collections::HashMap;
    static FOUND: Mutex<Option<HashMap<Script, &'static [&'static str]>>> = Mutex::new(None);
    if let Some(found) = FOUND.lock().unwrap().as_ref().and_then(|f| f.get(&script)) {
        return found;
    }
    let family = letter_in(script).and_then(|c| {
        let family = os::fallback_family(c, &fallback_language(c))?;
        let mut fallbacks = FALLBACKS.lock().unwrap();
        want(&mut fallbacks.notes, system_fallback(c));
        Some(family)
    });
    let families: &'static [&'static str] = match family {
        Some(family) => Box::leak(vec![&*Box::leak(family.into_boxed_str())].into_boxed_slice()),
        None => &[],
    };
    FOUND
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(script, families);
    families
}

/// The first letter Unicode assigns to `script`.
#[cfg(any(target_vendor = "apple", windows))]
fn letter_in(script: Script) -> Option<char> {
    use unicode_script::UnicodeScript as _;
    (0x80..0x3_0000)
        .filter_map(char::from_u32)
        .find(|c| c.is_alphabetic() && c.script() == script)
}

/// Waits until every fallback family asked for so far has been read.
#[cfg(test)]
fn wait_for_fallbacks() {
    let start = std::time::Instant::now();
    loop {
        let notes = FALLBACKS.lock().unwrap().notes.clone();
        if read(&notes).1 == notes.len() {
            return;
        }
        assert!(start.elapsed().as_secs() < 60, "fallback families never read");
        std::thread::sleep(std::time::Duration::from_millis(5));
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

    /// The characters of `text` that draw as tofu once the fallback families
    /// its first layout asked for are read and added.
    fn tofu(fs: &mut FontSystem, text: &str, attrs: Attrs) -> BTreeSet<char> {
        let mut buf = Buffer::new(fs, Metrics::new(16., 20.));
        buf.set_text(text, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);
        // as the app's shaping cache reports what no font drew
        let missing: Vec<char> = buf
            .layout_runs()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .filter(|g| g.glyph_id == 0)
                    .flat_map(|g| run.text[g.start..g.end].chars())
            })
            .collect();
        super::missing(missing.into_iter());
        super::wait_for_fallbacks();
        super::refresh(fs.db_mut());
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

    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
    struct NoPath;
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
    impl ttf_parser::OutlineBuilder for NoPath {
        fn move_to(&mut self, _: f32, _: f32) {}
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }

    /// Arrows in prose sit on the math axis the `=` beside them marks.
    #[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "windows")))]
    #[test]
    fn arrows_sit_on_the_text_axis() {
        let mut fs = crate::widgets::glyphon_render::new_font_system();
        let mut buf = Buffer::new(&mut fs, Metrics::new(16., 20.));
        let attrs = Attrs::new().family(Family::SansSerif);
        buf.set_text("=→", &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(&mut fs, false);
        let run = buf.layout_runs().next().unwrap();
        // vertical center of each glyph, in ems
        let centers: Vec<f32> = run
            .glyphs
            .iter()
            .map(|g| {
                fs.db()
                    .with_face_data(g.font_id, |data, index| {
                        let face = ttf_parser::Face::parse(data, index).unwrap();
                        let bbox = face
                            .outline_glyph(ttf_parser::GlyphId(g.glyph_id), &mut NoPath)
                            .unwrap();
                        (bbox.y_min + bbox.y_max) as f32 / 2. / face.units_per_em() as f32
                    })
                    .unwrap()
            })
            .collect();
        // Noto Sans Math's arrow sits 0.01 em under Noto Sans's `=`; Noto Sans Mono's, 0.19
        assert!((centers[0] - centers[1]).abs() < 0.1, "`=` and `→` centers: {centers:?}");
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
        // the layout that asks for every fallback family the text needs
        let _ = tofu(&mut fs, &text, Attrs::new().family(Family::SansSerif));
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
