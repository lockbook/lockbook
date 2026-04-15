use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use glyphon::fontdb::{Database, Source};
use std::sync::Arc;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    let (sans, mono, bold, base_scale) = if cfg!(target_vendor = "apple") {
        (lb_fonts::SF_PRO_TEXT_REGULAR, lb_fonts::SF_MONO_REGULAR, lb_fonts::SF_PRO_TEXT_BOLD, 0.9)
    } else {
        (
            lb_fonts::NOTO_SANS_REGULAR,
            lb_fonts::NOTO_SANS_MONO_REGULAR,
            lb_fonts::NOTO_SANS_BOLD,
            1.,
        )
    };

    let mono_scale = 0.9 * base_scale;
    let mono_y_offset_factor = 0.1;
    let mono_baseline_offset_factor = -0.1;

    let super_y_offset_factor = -1. / 4.;
    let sub_y_offset_factor = 1. / 4.;
    let super_scale = 3. / 4.;
    let sub_scale = 3. / 4.;

    fonts.font_data.insert(
        "sans".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(sans)
        }
        .into(),
    );
    fonts.font_data.insert("mono".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: mono_y_offset_factor,
                scale: mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });
    fonts.font_data.insert(
        "bold".to_string(),
        FontData {
            tweak: FontTweak { scale: base_scale, ..FontTweak::default() },
            ..FontData::from_static(bold)
        }
        .into(),
    );

    fonts.font_data.insert("sans_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }
        .into()
    });
    fonts.font_data.insert("bold_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor,
                scale: super_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }
        .into()
    });
    fonts.font_data.insert("mono_super".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: super_y_offset_factor + mono_y_offset_factor,
                scale: super_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });

    fonts.font_data.insert("sans_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(sans)
        }
        .into()
    });
    fonts.font_data.insert("bold_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor,
                scale: sub_scale * base_scale,
                ..Default::default()
            },
            ..FontData::from_static(bold)
        }
        .into()
    });
    fonts.font_data.insert("mono_sub".into(), {
        FontData {
            tweak: FontTweak {
                y_offset_factor: sub_y_offset_factor + mono_y_offset_factor,
                scale: sub_scale * mono_scale,
                baseline_offset_factor: mono_baseline_offset_factor,
                ..Default::default()
            },
            ..FontData::from_static(mono)
        }
        .into()
    });

    fonts.font_data.insert("icons".into(), {
        FontData {
            tweak: FontTweak { y_offset: -0.1, scale: mono_scale, ..Default::default() },
            ..FontData::from_static(lb_fonts::NERD_FONTS_MONO_SYMBOLS)
        }
        .into()
    });

    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Bold")), vec!["bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSuper")), vec!["sans_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSuper")), vec!["bold_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSuper")), vec!["mono_super".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("SansSub")), vec!["sans_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("BoldSub")), vec!["bold_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("MonoSub")), vec!["mono_sub".into()]);
    fonts
        .families
        .insert(FontFamily::Name(Arc::from("Icons")), vec!["icons".into()]);

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "sans".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "mono".to_string());

    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .push("icons".to_owned());
}

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
