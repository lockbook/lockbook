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
        use glyphon::Weight;

        db.set_sans_serif_family("SF Pro Text");
        db.set_monospace_family("SF Mono");

        for font in lb_fonts::SF {
            let source = Source::Binary(Arc::new(font) as _);
            let ids = db.load_font_source(source);

            // SF Pro Text "Bold" has usWeightClass=600 (semibold). Re-register
            // it as 700 so fontdb resolves Weight::BOLD to SF instead of
            // falling back to Noto Sans Bold.
            for id in ids {
                if let Some(face) = db.face(id) {
                    if face.weight == Weight::SEMIBOLD {
                        let mut patched = face.clone();
                        patched.weight = Weight::BOLD;
                        db.remove_face(id);
                        db.push_face_info(patched);
                    }
                }
            }
        }
    }
}
