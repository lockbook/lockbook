//! The files the system's font service has for a family, and for every
//! family it has. A font installed for one user is found the same as one
//! installed for everyone.

use std::path::PathBuf;

/// The files of `family`'s faces.
#[cfg(target_vendor = "apple")]
pub fn family_files(family: &str) -> Vec<PathBuf> {
    use objc2_core_foundation::{CFArray, CFDictionary, CFString, CFType};
    use objc2_core_text::{CTFontCollection, CTFontDescriptor, kCTFontFamilyNameAttribute};
    let key: &CFString = unsafe { kCTFontFamilyNameAttribute };
    let family = CFString::from_str(family);
    let value: &CFType = family.as_ref();
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&[key], &[value]);
    let query = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };
    let queries = CFArray::from_retained_objects(&[query]);
    let collection =
        unsafe { CTFontCollection::with_font_descriptors(Some(queries.as_opaque()), None) };
    let found = unsafe { collection.matching_font_descriptors() };
    found.map(descriptor_files).unwrap_or_default()
}

/// The files of every face the system has.
#[cfg(target_vendor = "apple")]
pub fn all_files() -> Vec<PathBuf> {
    use objc2_core_text::CTFontCollection;
    let collection = unsafe { CTFontCollection::from_available_fonts(None) };
    unsafe { collection.matching_font_descriptors() }
        .map(descriptor_files)
        .unwrap_or_default()
}

/// The system's interface text and monospace fonts, upright and italic,
/// which go by hidden names a family lookup doesn't find. The monospace one
/// has no file of its own to name, so it comes as the bytes Core Text draws
/// it from.
#[cfg(target_os = "macos")]
pub fn interface_fonts() -> Vec<glyphon::fontdb::Source> {
    use objc2_app_kit::{NSFont, NSFontWeightRegular};
    use objc2_core_text::{CTFont, CTFontUIFontType};
    let text = unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, None) };
    let mono = NSFont::monospacedSystemFontOfSize_weight(0.0, unsafe { NSFontWeightRegular });
    // an NSFont is the CTFont it wraps
    let mono: &CTFont = unsafe { &*(objc2::rc::Retained::as_ptr(&mono) as *const CTFont) };
    let mono = Some(objc2_core_foundation::CFRetained::from(mono));
    let mut fonts = vec![];
    for font in [text, mono].into_iter().flatten() {
        let italic = italic(&font);
        for font in [Some(font), italic].into_iter().flatten() {
            fonts.extend(source(&font));
        }
    }
    fonts
}

/// `font`'s italic, where it has one.
#[cfg(target_os = "macos")]
fn italic(
    font: &objc2_core_text::CTFont,
) -> Option<objc2_core_foundation::CFRetained<objc2_core_text::CTFont>> {
    use objc2_core_text::CTFontSymbolicTraits as Traits;
    unsafe {
        font.copy_with_symbolic_traits(
            0.0,
            std::ptr::null(),
            Traits::TraitItalic,
            Traits::TraitItalic,
        )
    }
}

/// Where `font` is read from: its file, else the bytes Core Text draws it from.
#[cfg(target_os = "macos")]
fn source(font: &objc2_core_text::CTFont) -> Option<glyphon::fontdb::Source> {
    use glyphon::fontdb::Source;
    font_file(font)
        .map(Source::File)
        .or_else(|| font_bytes(font).map(|bytes| Source::Binary(std::sync::Arc::new(bytes))))
}

/// The family the system draws `c` in when the interface font lacks it, for
/// text in `language`; none where the interface font draws it.
#[cfg(target_vendor = "apple")]
pub fn fallback_family(c: char, language: &str) -> Option<String> {
    let (base, font) = fallback_font(c, language, false)?;
    let family = unsafe { font.family_name() }.to_string();
    (family != unsafe { base.family_name() }.to_string()).then_some(family)
}

/// The font [`fallback_family`] names, by its PostScript name, and where to read it.
#[cfg(target_os = "macos")]
pub fn fallback_source(c: char, language: &str) -> Option<(String, glyphon::fontdb::Source)> {
    let (_, font) = fallback_font(c, language, true)?;
    let name = unsafe { font.post_script_name() }.to_string();
    Some((name, source(&font)?))
}

/// The font Core Text draws `c` in, else the next it would cascade to that
/// Swash can draw; `thorough` also searches every installed font made for
/// `language`, which takes tens of milliseconds, so only off the UI thread.
#[cfg(target_vendor = "apple")]
fn fallback_font(
    c: char, language: &str, thorough: bool,
) -> Option<(
    objc2_core_foundation::CFRetained<objc2_core_text::CTFont>,
    objc2_core_foundation::CFRetained<objc2_core_text::CTFont>,
)> {
    use objc2_core_foundation::{CFArray, CFRange, CFRetained, CFString};
    use objc2_core_text::{CTFont, CTFontUIFontType};
    let base = unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, None) }?;
    let text = CFString::from_str(&c.to_string());
    let range = CFRange { location: 0, length: text.length() };
    let language = CFString::from_str(language);
    let font = unsafe { base.for_string_with_language(&text, range, Some(&language)) };
    if !hvgl(&font) {
        return Some((base, font));
    }
    // Swash can't draw hvgl outlines: the next font the system would cascade to
    let languages =
        objc2_core_foundation::CFArray::from_retained_objects(std::slice::from_ref(&language));
    let cascade = unsafe { base.default_cascade_list_for_languages(Some(languages.as_opaque())) }?;
    let cascade: CFRetained<CFArray<objc2_core_text::CTFontDescriptor>> =
        unsafe { CFRetained::cast_unchecked(cascade) };
    // Han's forms differ by language, so a CJK font must be made for the one asked
    let cjk = ["zh", "ja", "ko"]
        .iter()
        .any(|l| language.to_string().starts_with(l));
    let usable = |font: &CTFont| {
        !hvgl(font)
            && !last_resort(font)
            && draws(font, c)
            && (!cjk || supports(font, &language.to_string()))
    };
    let fonts = |descriptors: CFRetained<CFArray<objc2_core_text::CTFontDescriptor>>| {
        descriptors
            .iter()
            .map(|descriptor| unsafe {
                CTFont::with_font_descriptor(&descriptor, 0.0, std::ptr::null())
            })
            .find(|font| usable(font))
    };
    // where every font the cascade has for it is hvgl, any installed one made
    // for the language
    let font =
        fonts(cascade).or_else(|| thorough.then(|| fonts(made_for(&language)?)).flatten())?;
    Some((base, font))
}

/// The installed fonts made for `language`.
#[cfg(target_vendor = "apple")]
fn made_for(
    language: &objc2_core_foundation::CFString,
) -> Option<
    objc2_core_foundation::CFRetained<
        objc2_core_foundation::CFArray<objc2_core_text::CTFontDescriptor>,
    >,
> {
    use objc2_core_foundation::{CFArray, CFDictionary, CFRetained, CFString, CFType};
    use objc2_core_text::{CTFontCollection, CTFontDescriptor, kCTFontLanguagesAttribute};
    let languages = CFArray::from_objects(&[language]);
    let key: &CFString = unsafe { kCTFontLanguagesAttribute };
    let value: &CFType = languages.as_ref();
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&[key], &[value]);
    let query = unsafe { CTFontDescriptor::with_attributes(attributes.as_opaque()) };
    let queries = CFArray::from_retained_objects(&[query]);
    let collection =
        unsafe { CTFontCollection::with_font_descriptors(Some(queries.as_opaque()), None) };
    let found = unsafe { collection.matching_font_descriptors() }?;
    Some(unsafe { CFRetained::cast_unchecked(found) })
}

/// Whether Swash can't draw `font`: it has no outlines, or Apple's `hvgl` ones.
#[cfg(target_vendor = "apple")]
fn hvgl(font: &objc2_core_text::CTFont) -> bool {
    let has = |tag: &[u8; 4]| unsafe { font.has_table(u32::from_be_bytes(*tag)) };
    has(b"hvgl") || ![b"glyf", b"CFF ", b"CFF2"].iter().any(|tag| has(tag))
}

/// Whether `font` is the one that claims every character to draw a
/// placeholder for it.
#[cfg(target_vendor = "apple")]
fn last_resort(font: &objc2_core_text::CTFont) -> bool {
    unsafe { font.post_script_name() }.to_string() == "LastResort"
}

/// Whether `font` was made for `language`.
#[cfg(target_vendor = "apple")]
fn supports(font: &objc2_core_text::CTFont, language: &str) -> bool {
    use objc2_core_foundation::{CFArray, CFRetained, CFString};
    let languages: CFRetained<CFArray<CFString>> =
        unsafe { CFRetained::cast_unchecked(font.supported_languages()) };
    languages.iter().any(|l| l.to_string() == language)
}

/// Whether `font` has a glyph for `c`.
#[cfg(target_vendor = "apple")]
fn draws(font: &objc2_core_text::CTFont, c: char) -> bool {
    unsafe { font.character_set() }.is_long_character_member(c as u32)
}

/// `font` as an OpenType file, assembled from the tables Core Text has for it.
#[cfg(target_os = "macos")]
fn font_bytes(font: &objc2_core_text::CTFont) -> Option<Vec<u8>> {
    use objc2_core_text::CTFontTableOptions;
    let tags = unsafe { font.available_tables(CTFontTableOptions::empty()) }?;
    // the array holds the tags themselves, not objects
    let tags: Vec<u32> = (0..tags.count())
        .map(|i| unsafe { tags.value_at_index(i) } as usize as u32)
        .collect();
    let tables: Vec<(u32, Vec<u8>)> = tags
        .into_iter()
        .filter_map(|tag| {
            let data = unsafe { font.table(tag, CTFontTableOptions::empty()) }?;
            Some((tag, data.to_vec()))
        })
        .collect();
    let cff = tables.iter().any(|(tag, _)| tag.to_be_bytes() == *b"CFF ");
    Some(sfnt(if cff { 0x4F54_544F } else { 0x0001_0000 }, tables))
}

/// An OpenType file of `tables`, as `version` names its outlines.
#[cfg(target_os = "macos")]
fn sfnt(version: u32, mut tables: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|(tag, _)| *tag);
    let count = tables.len() as u16;
    let pow = 1u16 << (15 - count.max(1).leading_zeros());
    let mut out = vec![];
    out.extend(version.to_be_bytes());
    out.extend(count.to_be_bytes());
    out.extend((pow * 16).to_be_bytes());
    out.extend((pow.trailing_zeros() as u16).to_be_bytes());
    out.extend((count * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in &tables {
        let sum = data
            .chunks(4)
            .map(|c| {
                let mut word = [0; 4];
                word[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(word)
            })
            .fold(0u32, u32::wrapping_add);
        out.extend(tag.to_be_bytes());
        out.extend(sum.to_be_bytes());
        out.extend((offset as u32).to_be_bytes());
        out.extend((data.len() as u32).to_be_bytes());
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        out.extend(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    out
}

#[cfg(target_os = "macos")]
fn font_file(font: &objc2_core_text::CTFont) -> Option<PathBuf> {
    use objc2_core_foundation::CFURL;
    use objc2_core_text::kCTFontURLAttribute;
    unsafe { font.attribute(kCTFontURLAttribute) }?
        .downcast::<CFURL>()
        .ok()?
        .to_file_path()
}

#[cfg(target_vendor = "apple")]
fn descriptor_files(
    found: objc2_core_foundation::CFRetained<objc2_core_foundation::CFArray>,
) -> Vec<PathBuf> {
    use objc2_core_foundation::{CFArray, CFRetained, CFURL};
    use objc2_core_text::{CTFontDescriptor, kCTFontURLAttribute};
    // the array holds font descriptors
    let found: CFRetained<CFArray<CTFontDescriptor>> = unsafe { CFRetained::cast_unchecked(found) };
    let mut files: Vec<PathBuf> = found
        .iter()
        .filter_map(|descriptor| unsafe { descriptor.attribute(kCTFontURLAttribute) })
        .filter_map(|url| url.downcast::<CFURL>().ok()?.to_file_path())
        .collect();
    files.sort();
    files.dedup();
    files
}

/// The files of `family`'s faces.
#[cfg(windows)]
pub fn family_files(family: &str) -> Vec<PathBuf> {
    use windows::core::{BOOL, HSTRING};
    let Some(collection) = system_collection() else { return vec![] };
    let (mut index, mut exists) = (0u32, BOOL(0));
    let found =
        unsafe { collection.FindFamilyName(&HSTRING::from(family), &mut index, &mut exists) };
    if found.is_err() || !exists.as_bool() {
        return vec![];
    }
    let mut files = unsafe { collection.GetFontFamily(index) }
        .map(|family| family_paths(&family))
        .unwrap_or_default();
    files.sort();
    files.dedup();
    files
}

/// The files of every face the system has.
#[cfg(windows)]
pub fn all_files() -> Vec<PathBuf> {
    let Some(collection) = system_collection() else { return vec![] };
    let count = unsafe { collection.GetFontFamilyCount() };
    let mut files: Vec<PathBuf> = (0..count)
        .filter_map(|i| unsafe { collection.GetFontFamily(i) }.ok())
        .flat_map(|family| family_paths(&family))
        .collect();
    files.sort();
    files.dedup();
    files
}

#[cfg(windows)]
fn system_collection() -> Option<windows::Win32::Graphics::DirectWrite::IDWriteFontCollection> {
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
    };
    let factory: IDWriteFactory =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
    let mut collection = None;
    unsafe { factory.GetSystemFontCollection(&mut collection, false) }.ok()?;
    collection
}

/// The local files `family`'s faces are read from.
#[cfg(windows)]
fn family_paths(family: &windows::Win32::Graphics::DirectWrite::IDWriteFontFamily) -> Vec<PathBuf> {
    use windows::Win32::Graphics::DirectWrite::IDWriteLocalFontFileLoader;
    use windows::core::Interface as _;
    let mut paths = vec![];
    for i in 0..unsafe { family.GetFontCount() } {
        let Ok(face) = (unsafe { family.GetFont(i).and_then(|font| font.CreateFontFace()) }) else {
            continue;
        };
        let mut count = 0u32;
        if unsafe { face.GetFiles(&mut count, None) }.is_err() {
            continue;
        }
        let mut files = vec![None; count as usize];
        if unsafe { face.GetFiles(&mut count, Some(files.as_mut_ptr())) }.is_err() {
            continue;
        }
        for file in files.into_iter().flatten() {
            let (mut key, mut key_len) = (std::ptr::null_mut(), 0u32);
            if unsafe { file.GetReferenceKey(&mut key, &mut key_len) }.is_err() {
                continue;
            }
            let Ok(local) = unsafe { file.GetLoader() }
                .and_then(|loader| loader.cast::<IDWriteLocalFontFileLoader>())
            else {
                continue;
            };
            let Ok(len) = (unsafe { local.GetFilePathLengthFromKey(key, key_len) }) else {
                continue;
            };
            let mut path = vec![0u16; len as usize + 1];
            if unsafe { local.GetFilePathFromKey(key, key_len, &mut path) }.is_ok() {
                paths.push(PathBuf::from(String::from_utf16_lossy(&path[..len as usize])));
            }
        }
    }
    paths
}

/// The family the system draws `c` in when the interface font lacks it, for
/// text in `language`; none where the interface font draws it.
#[cfg(windows)]
pub fn fallback_family(c: char, language: &str) -> Option<String> {
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_NORMAL, DWriteCreateFactory, IDWriteFactory2, IDWriteTextAnalysisSource,
    };
    use windows::core::{HSTRING, w};
    let factory: IDWriteFactory2 =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
    let fallback = unsafe { factory.GetSystemFontFallback() }.ok()?;
    let collection = system_collection()?;
    let text: Vec<u16> = c.to_string().encode_utf16().collect();
    let len = text.len() as u32;
    let source: IDWriteTextAnalysisSource =
        TextSource { text, locale: HSTRING::from(language) }.into();
    let (mut mapped, mut font, mut scale) = (0u32, None, 0f32);
    unsafe {
        fallback.MapCharacters(
            &source,
            0,
            len,
            &collection,
            w!("Segoe UI"),
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            &mut mapped,
            &mut font,
            &mut scale,
        )
    }
    .ok()?;
    let names = unsafe {
        font?
            .GetFontFamily()
            .and_then(|family| family.GetFamilyNames())
    }
    .ok()?;
    let family = localized(&names)?;
    (family != "Segoe UI").then_some(family)
}

/// `names` in English, else its first.
#[cfg(windows)]
fn localized(
    names: &windows::Win32::Graphics::DirectWrite::IDWriteLocalizedStrings,
) -> Option<String> {
    use windows::core::{BOOL, w};
    let (mut index, mut exists) = (0u32, BOOL(0));
    let _ = unsafe { names.FindLocaleName(w!("en-us"), &mut index, &mut exists) };
    if !exists.as_bool() {
        index = 0;
    }
    let len = unsafe { names.GetStringLength(index) }.ok()?;
    let mut name = vec![0u16; len as usize + 1];
    unsafe { names.GetString(index, &mut name) }.ok()?;
    Some(String::from_utf16_lossy(&name[..len as usize]))
}

/// One character of text, in one language, for DirectWrite to find a font for.
#[cfg(windows)]
#[windows::core::implement(windows::Win32::Graphics::DirectWrite::IDWriteTextAnalysisSource)]
struct TextSource {
    text: Vec<u16>,
    locale: windows::core::HSTRING,
}

#[cfg(windows)]
impl windows::Win32::Graphics::DirectWrite::IDWriteTextAnalysisSource_Impl for TextSource_Impl {
    fn GetTextAtPosition(
        &self, position: u32, text: *mut *mut u16, length: *mut u32,
    ) -> windows::core::Result<()> {
        let at = (position as usize).min(self.text.len());
        unsafe {
            *text = self.text[at..].as_ptr() as *mut u16;
            *length = (self.text.len() - at) as u32;
        }
        Ok(())
    }

    fn GetTextBeforePosition(
        &self, position: u32, text: *mut *mut u16, length: *mut u32,
    ) -> windows::core::Result<()> {
        let at = (position as usize).min(self.text.len());
        unsafe {
            *text = self.text.as_ptr() as *mut u16;
            *length = at as u32;
        }
        Ok(())
    }

    fn GetParagraphReadingDirection(
        &self,
    ) -> windows::Win32::Graphics::DirectWrite::DWRITE_READING_DIRECTION {
        windows::Win32::Graphics::DirectWrite::DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
    }

    fn GetLocaleName(
        &self, position: u32, length: *mut u32, locale: *mut *mut u16,
    ) -> windows::core::Result<()> {
        unsafe {
            *length = (self.text.len() as u32).saturating_sub(position);
            *locale = self.locale.as_ptr() as *mut u16;
        }
        Ok(())
    }

    fn GetNumberSubstitution(
        &self, position: u32, length: *mut u32,
        substitution: windows::core::OutRef<
            windows::Win32::Graphics::DirectWrite::IDWriteNumberSubstitution,
        >,
    ) -> windows::core::Result<()> {
        unsafe { *length = (self.text.len() as u32).saturating_sub(position) };
        substitution.write(None)
    }
}
