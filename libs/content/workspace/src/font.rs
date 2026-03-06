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

pub fn load(db: &mut Database) {
    #[cfg(target_vendor = "apple")]
    {
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::SF_PRO_REGULAR) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::SF_MONO_REGULAR) as _));

        // SF Pro Text Bold reports family "SF Pro Text", not "SF Pro".
        // Load it manually so fontdb sees it as the bold face of the "SF Pro" family.
        let ids = db.load_font_source(Source::Binary(Arc::new(lb_fonts::SF_PRO_TEXT_BOLD) as _));
        for id in ids {
            if let Some(face) = db.face(id) {
                let mut info = FaceInfo {
                    families: vec![(SANS.to_string(), fontdb::Language::English_UnitedStates)],
                    ..face.clone()
                };
                info.id = fontdb::ID::dummy();
                db.push_face_info(info);
                db.remove_face(id);
            }
        }
    }

    #[cfg(not(target_vendor = "apple"))]
    {
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::PT_SANS_REGULAR) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::PT_SANS_BOLD) as _));
        db.load_font_source(Source::Binary(Arc::new(lb_fonts::JETBRAINS_MONO) as _));
    }

    db.load_font_source(Source::Binary(Arc::new(lb_fonts::NERD_FONTS_MONO_SYMBOLS) as _));
    db.load_font_source(Source::Binary(Arc::new(lb_fonts::TWITTER_COLOR_EMOJI_15_1) as _));
}
