use crate::{core_err_unexpected, CoreError};
use image::codecs::bmp::BmpEncoder;
use image::codecs::farbfeld::FarbfeldEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::pnm::PnmEncoder;
use image::codecs::tga::TgaEncoder;
use image::ColorType;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
use raqote::{
    DrawOptions, DrawTarget, LineCap, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle,
};
use serde::Deserialize;
use serde_json::error::Category;
use std::array::IntoIter;
use std::collections::HashMap;
use std::io::BufWriter;
use std::iter::FromIterator;

#[derive(Deserialize)]
pub enum SupportedImageFormats {
    Png,
    Jpeg,
    Pnm,
    Tga,
    Farbfeld,
    Bmp,
}

pub fn parse_drawing(drawing_bytes: &[u8]) -> Result<Drawing, CoreError> {
    match serde_json::from_slice::<Drawing>(drawing_bytes) {
        Ok(d) => Ok(d),
        Err(e) => match e.classify() {
            Category::Io => Err(CoreError::Unexpected(String::from("json io"))),
            Category::Syntax | Category::Data | Category::Eof => Err(CoreError::DrawingInvalid),
        },
    }
}

pub fn export_drawing(
    drawing_bytes: &[u8],
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, CoreError> {
    let drawing = parse_drawing(drawing_bytes)?;

    let theme = match render_theme {
        Some(theme) => theme,
        None => match drawing.theme {
            None => HashMap::<_, _>::from_iter(IntoIter::new([
                (
                    ColorAlias::White,
                    ColorRGB {
                        r: 0xFF,
                        g: 0xFF,
                        b: 0xFF,
                    },
                ),
                (
                    ColorAlias::Black,
                    ColorRGB {
                        r: 0x00,
                        g: 0x00,
                        b: 0x00,
                    },
                ),
                (
                    ColorAlias::Red,
                    ColorRGB {
                        r: 0xFF,
                        g: 0x00,
                        b: 0x00,
                    },
                ),
                (
                    ColorAlias::Green,
                    ColorRGB {
                        r: 0x00,
                        g: 0xFF,
                        b: 0x00,
                    },
                ),
                (
                    ColorAlias::Yellow,
                    ColorRGB {
                        r: 0xFF,
                        g: 0xFF,
                        b: 0x00,
                    },
                ),
                (
                    ColorAlias::Blue,
                    ColorRGB {
                        r: 0x00,
                        g: 0x00,
                        b: 0xFF,
                    },
                ),
                (
                    ColorAlias::Magenta,
                    ColorRGB {
                        r: 0xFF,
                        g: 0x00,
                        b: 0xFF,
                    },
                ),
                (
                    ColorAlias::Cyan,
                    ColorRGB {
                        r: 0x00,
                        g: 0xFF,
                        b: 0xFF,
                    },
                ),
            ])),
            Some(theme) => theme,
        },
    };

    let (width, height) = get_drawing_bounds(drawing.strokes.as_slice());

    let mut draw_target = DrawTarget::new(width as i32, height as i32);

    for stroke in drawing.strokes {
        let color_rgb = theme
            .get(&stroke.color)
            .ok_or_else(|| CoreError::Unexpected(String::from("unable to get color from alias")))?;

        if stroke.points_x.len() != stroke.points_y.len()
            || stroke.points_y.len() != stroke.points_girth.len()
        {
            return Err(CoreError::Unexpected(String::from(
                "unequal points and girth metrics",
            )));
        }

        if stroke.alpha > 1.0 || stroke.alpha < 0.0 {
            return Err(CoreError::Unexpected(String::from("invalid alpha value")));
        }

        for point_index in 0..stroke.points_x.len() - 1 {
            let mut pb = PathBuilder::new();
            let x1 = stroke
                .points_x
                .get(point_index)
                .ok_or_else(|| {
                    CoreError::Unexpected(String::from("unable to get color from alias"))
                })?
                .to_owned();
            let y1 = stroke
                .points_y
                .get(point_index)
                .ok_or_else(|| {
                    CoreError::Unexpected(String::from("unable to get color from alias"))
                })?
                .to_owned();
            let x2 = stroke
                .points_x
                .get(point_index + 1)
                .ok_or_else(|| {
                    CoreError::Unexpected(String::from("unable to get color from alias"))
                })?
                .to_owned();
            let y2 = stroke
                .points_y
                .get(point_index + 1)
                .ok_or_else(|| {
                    CoreError::Unexpected(String::from("unable to get color from alias"))
                })?
                .to_owned();

            pb.move_to(x1, y1);
            pb.line_to(x2, y2);

            pb.close();
            let path = pb.finish();

            draw_target.stroke(
                &path,
                &Source::Solid(SolidSource {
                    r: color_rgb.r,
                    g: color_rgb.g,
                    b: color_rgb.b,
                    a: (stroke.alpha * 255.0) as u8,
                }),
                &StrokeStyle {
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                    width: stroke
                        .points_girth
                        .get(point_index)
                        .ok_or_else(|| {
                            CoreError::Unexpected(String::from("unable to get stroke girth"))
                        })?
                        .to_owned(),
                    miter_limit: 10.0,
                    dash_array: Vec::new(),
                    dash_offset: 0.0,
                },
                &DrawOptions::new(),
            );
        }
    }

    let mut buffer = Vec::<u8>::new();
    let mut buf_writer = BufWriter::new(&mut buffer);

    let mut drawing_bytes: Vec<u8> = Vec::new();

    for pixel in draw_target.into_vec().iter() {
        let (r, g, b, a) = u32_byte_to_u8_bytes(pixel.to_owned());

        drawing_bytes.push(r);
        drawing_bytes.push(g);
        drawing_bytes.push(b);
        drawing_bytes.push(a);
    }

    match format {
        SupportedImageFormats::Png => PngEncoder::new(&mut buf_writer).encode(
            drawing_bytes.as_slice(),
            width,
            height,
            ColorType::Rgba8,
        ),
        SupportedImageFormats::Pnm => PnmEncoder::new(&mut buf_writer).encode(
            drawing_bytes.as_slice(),
            width,
            height,
            ColorType::Rgba8,
        ),
        SupportedImageFormats::Jpeg => JpegEncoder::new(&mut buf_writer).encode(
            drawing_bytes.as_slice(),
            width,
            height,
            ColorType::Rgba8,
        ),
        SupportedImageFormats::Tga => TgaEncoder::new(&mut buf_writer).encode(
            drawing_bytes.as_slice(),
            width,
            height,
            ColorType::Rgba8,
        ),
        SupportedImageFormats::Farbfeld => {
            FarbfeldEncoder::new(&mut buf_writer).encode(drawing_bytes.as_slice(), width, height)
        }
        SupportedImageFormats::Bmp => BmpEncoder::new(&mut buf_writer).encode(
            drawing_bytes.as_slice(),
            width,
            height,
            ColorType::Rgba8,
        ),
    }
    .map_err(core_err_unexpected)?;

    std::mem::drop(buf_writer);

    Ok(buffer)
}

fn u32_byte_to_u8_bytes(u32_byte: u32) -> (u8, u8, u8, u8) {
    let mut byte_1 = (u32_byte >> 16) & 0xffu32;
    let mut byte_2 = (u32_byte >> 8) & 0xffu32;
    let mut byte_3 = u32_byte & 0xffu32;
    let byte_4 = (u32_byte >> 24) & 0xffu32;

    if byte_4 > 0u32 {
        byte_1 = byte_1 * 255u32 / byte_4;
        byte_2 = byte_2 * 255u32 / byte_4;
        byte_3 = byte_3 * 255u32 / byte_4;
    }

    (byte_1 as u8, byte_2 as u8, byte_3 as u8, byte_4 as u8)
}

fn get_drawing_bounds(strokes: &[Stroke]) -> (u32, u32) {
    let stroke_to_max_x = |stroke: &Stroke| {
        stroke
            .points_x
            .iter()
            .zip(stroke.points_girth.clone())
            .map(|(x, girth)| x + girth)
            .map(|num| num as u32)
            .max()
            .unwrap_or(0)
    };

    let stroke_to_max_y = |stroke: &Stroke| {
        stroke
            .points_y
            .iter()
            .zip(stroke.points_girth.clone())
            .map(|(y, girth)| y + girth)
            .map(|num| num as u32)
            .max()
            .unwrap_or(0)
    };

    let max_x_and_girth = strokes
        .iter()
        .map(|stroke| stroke_to_max_x(stroke))
        .max()
        .unwrap_or(0);

    let max_y_and_girth = strokes
        .iter()
        .map(|stroke| stroke_to_max_y(stroke))
        .max()
        .unwrap_or(0);

    (max_x_and_girth + 20, max_y_and_girth + 20)
}

#[cfg(test)]
mod unit_tests {
    use crate::service::drawing_service;
    use crate::service::drawing_service::SupportedImageFormats;
    use lockbook_models::drawing::{ColorAlias, Drawing, Stroke};

    #[test]
    fn parse_drawing_invalid() {
        assert!(drawing_service::parse_drawing(b"not a valid drawing").is_err());
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

        assert!(drawing_service::parse_drawing(&drawing_bytes).is_ok());
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

        assert_eq!(
            drawing_service::get_drawing_bounds(drawing.strokes.as_slice()),
            (20, 20)
        );
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

        assert_eq!(
            drawing_service::get_drawing_bounds(drawing.strokes.as_slice()),
            (121, 121)
        );
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

        assert_eq!(
            drawing_service::get_drawing_bounds(drawing.strokes.as_slice()),
            (2021, 2021)
        );
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

        let result = drawing_service::export_drawing(
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

        let result = drawing_service::export_drawing(
            &serde_json::to_vec(&drawing).unwrap(),
            SupportedImageFormats::Png,
            None,
        );
        assert!(result.is_err());
    }
}
