use glyphon::fontdb::{Database, Source};
use std::sync::Arc;

pub fn load(db: &mut Database) {
    db.set_sans_serif_family("Noto Sans");
    db.set_monospace_family("Noto Sans Mono");

    for font in lb_fonts::NOTO {
        db.load_font_source(Source::Binary(Arc::new(font) as _));
    }
    for font in lb_fonts::SYMBOLS {
        db.load_font_source(Source::Binary(Arc::new(font) as _));
    }

    #[cfg(target_vendor = "apple")]
    {
        db.set_sans_serif_family("SF Pro Text");
        db.set_monospace_family("SF Mono");

        for font in lb_fonts::SF {
            db.load_font_source(Source::Binary(Arc::new(font) as _));
        }
    }
}
