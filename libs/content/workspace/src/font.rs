use glyphon::fontdb::{self, Database, FaceInfo, Source};
use std::sync::Arc;

#[cfg(target_vendor = "apple")]
pub const SANS: &str = "SF Pro";
#[cfg(not(target_vendor = "apple"))]
pub const SANS: &str = "PT Sans";

#[cfg(target_vendor = "apple")]
pub const MONO: &str = "SF Mono";
#[cfg(not(target_vendor = "apple"))]
pub const MONO: &str = "JetBrains Mono";

// SF Pro's font files ship with different font family names
// this fn corrects the names so that they are recognized as one family
fn load_as_family(db: &mut Database, family: &str, data: &'static [u8]) {
    let ids = db.load_font_source(Source::Binary(Arc::new(data) as _));
    for id in ids {
        if let Some(face) = db.face(id) {
            let mut info = FaceInfo {
                families: vec![(family.to_string(), fontdb::Language::English_UnitedStates)],
                ..face.clone()
            };
            info.id = fontdb::ID::dummy();
            db.push_face_info(info);
            db.remove_face(id);
        }
    }
}

pub fn load(db: &mut Database) {
    #[cfg(target_vendor = "apple")]
    {
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::SF_PRO_REGULAR) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::SF_MONO_REGULAR) as _));
        load_as_family(db, SANS, lb_fonts::SF_PRO_TEXT_BOLD);
        load_as_family(db, SANS, lb_fonts::SF_PRO_ITALIC);
    }

    #[cfg(not(target_vendor = "apple"))]
    {
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::PT_SANS_REGULAR) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::PT_SANS_BOLD) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::PT_SANS_ITALIC) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::JETBRAINS_MONO) as _));
    }

    db.load_font_source(Source::Binary(Arc::new(lb_fonts::NERD_FONTS_MONO_SYMBOLS) as _));
    db.load_font_source(Source::Binary(Arc::new(lb_fonts::TWEMOJI_MOZILLA) as _));
}
