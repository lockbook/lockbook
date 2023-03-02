use lockbook_core::model::drawing;
use lockbook_core::{ColorAlias, Drawing, Stroke, SupportedImageFormats};
use test_utils::test_core_with_account;

#[test]
fn parse_drawing_invalid() {
    assert!(drawing::parse_drawing(b"not a valid drawing").is_err());
}

#[test]
fn parse_drawing_invalid_data() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10.0, 50.0, 60.0],
            points_y: vec![10.0, 50.0, 1000.0],
            points_girth: vec![5.0, 7.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };
    let drawing_bytes = serde_json::to_vec(&drawing).unwrap();
    drawing::parse_drawing(&drawing_bytes).unwrap_err();
}

#[test]
fn parse_drawing_valid() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10.0, 50.0, 60.0],
            points_y: vec![10.0, 50.0, 1000.0],
            points_girth: vec![5.0, 7.0, 9.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };
    let drawing_bytes = serde_json::to_vec(&drawing).unwrap();

    assert!(drawing::parse_drawing(&drawing_bytes).is_ok());
}

#[test]
fn get_drawing_bounds_empty() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![],
        theme: None,
    };

    assert_eq!(drawing::get_drawing_bounds(drawing.strokes.as_slice()), (20, 20));
}

#[test]
fn get_drawing_bounds_small() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![100.0],
            points_y: vec![100.0],
            points_girth: vec![1.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };

    assert_eq!(drawing::get_drawing_bounds(drawing.strokes.as_slice()), (121, 121));
}

#[test]
fn get_drawing_bounds_large() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![2000.0],
            points_y: vec![2000.0],
            points_girth: vec![1.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };

    assert_eq!(drawing::get_drawing_bounds(drawing.strokes.as_slice()), (2021, 2021));
}

#[test]
fn export_drawing_valid() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10.0, 50.0, 60.0],
            points_y: vec![10.0, 50.0, 1000.0],
            points_girth: vec![5.0, 7.0, 91.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };

    let result = drawing::export_drawing(
        &serde_json::to_vec(&drawing).unwrap(),
        SupportedImageFormats::Png,
        None,
    );
    assert!(result.is_ok());
}

#[test]
fn export_drawing_invalid() {
    let drawing = Drawing {
        scale: 1.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10.0, 50.0, 60.0],
            points_y: vec![10.0, 50.0, 1000.0],
            points_girth: vec![5.0, 7.0],
            color: ColorAlias::Black,
            alpha: 0.0,
        }],
        theme: None,
    };

    let result = drawing::export_drawing(
        &serde_json::to_vec(&drawing).unwrap(),
        SupportedImageFormats::Png,
        None,
    );
    assert!(result.is_err());
}

#[test]
fn get_drawing() {
    let core = test_core_with_account();
    let drawing = core.create_at_path("/drawing.draw").unwrap();
    core.get_drawing(drawing.id).unwrap();
}

#[test]
fn save_drawing() {
    let core = test_core_with_account();
    let drawing = core.create_at_path("/drawing.draw").unwrap();
    core.save_drawing(
        drawing.id,
        &Drawing {
            scale: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            strokes: vec![Stroke {
                points_x: vec![10.0, 50.0, 60.0],
                points_y: vec![10.0, 50.0, 1000.0],
                points_girth: vec![5.0, 7.0, 9.0],
                color: ColorAlias::Black,
                alpha: 0.0,
            }],
            theme: None,
        },
    )
    .unwrap();
    core.get_drawing(drawing.id).unwrap();
}

#[test]
fn save_drawing_invalid() {
    let core = test_core_with_account();
    let drawing = core.create_at_path("/drawing.draw").unwrap();
    core.save_drawing(
        drawing.id,
        &Drawing {
            scale: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            strokes: vec![Stroke {
                points_x: vec![10.0, 50.0, 60.0],
                points_y: vec![10.0, 50.0, 1000.0],
                points_girth: vec![5.0, 7.0],
                color: ColorAlias::Black,
                alpha: 0.0,
            }],
            theme: None,
        },
    )
    .unwrap_err();
}

#[test]
fn export_drawing() {
    let core = test_core_with_account();
    let drawing = core.create_at_path("/drawing.draw").unwrap();
    core.save_drawing(
        drawing.id,
        &Drawing {
            scale: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            strokes: vec![Stroke {
                points_x: vec![10.0, 50.0, 60.0],
                points_y: vec![10.0, 50.0, 1000.0],
                points_girth: vec![5.0, 7.0, 9.0],
                color: ColorAlias::Black,
                alpha: 0.0,
            }],
            theme: None,
        },
    )
    .unwrap();
    core.export_drawing(drawing.id, SupportedImageFormats::Png, None)
        .unwrap();
}
