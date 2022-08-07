use lockbook_core::model::drawing;
use lockbook_core::model::drawing::SupportedImageFormats;
use lockbook_shared::drawing::{ColorAlias, Drawing, Stroke};

#[test]
fn parse_drawing_invalid() {
    assert!(drawing::parse_drawing(b"not a valid drawing").is_err());
}

#[test]
fn parse_drawing_valid() {
    let drawing = Drawing {
        scale: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10f32, 50f32, 60f32],
            points_y: vec![10f32, 50f32, 1000f32],
            points_girth: vec![5f32, 7f32],
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
        scale: 0.0,
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
        scale: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![100f32],
            points_y: vec![100f32],
            points_girth: vec![1f32],
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
        scale: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![2000f32],
            points_y: vec![2000f32],
            points_girth: vec![1f32],
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
        scale: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10f32, 50f32, 60f32],
            points_y: vec![10f32, 50f32, 1000f32],
            points_girth: vec![5f32, 7f32, 91f32],
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
        scale: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        strokes: vec![Stroke {
            points_x: vec![10f32, 50f32, 60f32],
            points_y: vec![10f32, 50f32, 1000f32],
            points_girth: vec![5f32, 7f32],
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
