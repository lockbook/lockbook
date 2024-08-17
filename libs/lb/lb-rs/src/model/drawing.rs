use std::collections::HashMap;
use std::io::BufWriter;
use std::iter::FromIterator;

use crate::shared::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
use image::codecs::bmp::BmpEncoder;
use image::codecs::farbfeld::FarbfeldEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::pnm::PnmEncoder;
use image::codecs::tga::TgaEncoder;
use image::{ColorType, ImageEncoder};
use raqote::{
    DrawOptions, DrawTarget, LineCap, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle,
};
use serde::Deserialize;
use serde_json::error::Category;

use crate::model::errors::core_err_unexpected;
use crate::{CoreError, LbResult};

pub fn validate(drawing: &Drawing) -> LbResult<()> {
    if drawing.scale <= 0.0 {
        return Err(CoreError::DrawingInvalid.into());
    }

    for stroke in &drawing.strokes {
        if stroke.points_x.len() != stroke.points_y.len()
            || stroke.points_y.len() != stroke.points_girth.len()
        {
            return Err(CoreError::DrawingInvalid.into());
        }

        if stroke.alpha > 1.0 || stroke.alpha < 0.0 {
            return Err(CoreError::DrawingInvalid.into());
        }
    }

    Ok(())
}

pub fn parse_drawing(drawing_bytes: &[u8]) -> LbResult<Drawing> {
    // represent an empty string as an empty drawing, rather than returning an error
    if drawing_bytes.is_empty() {
        return Ok(Drawing::default());
    }
    let drawing =
        serde_json::from_slice::<Drawing>(drawing_bytes).map_err(|err| match err.classify() {
            Category::Io => CoreError::Unexpected(String::from("json io")),
            Category::Syntax | Category::Data | Category::Eof => CoreError::DrawingInvalid,
        })?;
    validate(&drawing)?;
    Ok(drawing)
}

#[derive(Deserialize, Debug)]
pub enum SupportedImageFormats {
    Png,
    Jpeg,
    Pnm,
    Tga,
    Farbfeld,
    Bmp,
}

impl std::str::FromStr for SupportedImageFormats {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "bmp" => Ok(Self::Bmp),
            "tga" => Ok(Self::Tga),
            "pnm" => Ok(Self::Pnm),
            "farbfeld" => Ok(Self::Farbfeld),
            unsupp => Err(format!("unsupported image format '{}'", unsupp)),
        }
    }
}

pub fn export_drawing(
    drawing_bytes: &[u8], format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> LbResult<Vec<u8>> {
    let drawing = parse_drawing(drawing_bytes)?;

    let theme = match render_theme {
        Some(theme) => theme,
        None => match drawing.theme {
            None => HashMap::<_, _>::from_iter(IntoIterator::into_iter([
                (ColorAlias::White, ColorRGB { r: 0xFF, g: 0xFF, b: 0xFF }),
                (ColorAlias::Black, ColorRGB { r: 0x00, g: 0x00, b: 0x00 }),
                (ColorAlias::Red, ColorRGB { r: 0xFF, g: 0x00, b: 0x00 }),
                (ColorAlias::Green, ColorRGB { r: 0x00, g: 0xFF, b: 0x00 }),
                (ColorAlias::Yellow, ColorRGB { r: 0xFF, g: 0xFF, b: 0x00 }),
                (ColorAlias::Blue, ColorRGB { r: 0x00, g: 0x00, b: 0xFF }),
                (ColorAlias::Magenta, ColorRGB { r: 0xFF, g: 0x00, b: 0xFF }),
                (ColorAlias::Cyan, ColorRGB { r: 0x00, g: 0xFF, b: 0xFF }),
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

        for point_index in 0..stroke.points_x.len() - 1 {
            let mut pb = PathBuilder::new();
            let x1 = stroke
                .points_x
                .get(point_index)
                .ok_or(CoreError::DrawingInvalid)?
                .to_owned();
            let y1 = stroke
                .points_y
                .get(point_index)
                .ok_or(CoreError::DrawingInvalid)?
                .to_owned();
            let x2 = stroke
                .points_x
                .get(point_index + 1)
                .ok_or(CoreError::DrawingInvalid)?
                .to_owned();
            let y2 = stroke
                .points_y
                .get(point_index + 1)
                .ok_or(CoreError::DrawingInvalid)?
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
                        .ok_or(CoreError::DrawingInvalid)?
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
        SupportedImageFormats::Png => PngEncoder::new(&mut buf_writer).write_image(
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

pub fn get_drawing_bounds(strokes: &[Stroke]) -> (u32, u32) {
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

    let max_x_and_girth = strokes.iter().map(stroke_to_max_x).max().unwrap_or(0);

    let max_y_and_girth = strokes.iter().map(stroke_to_max_y).max().unwrap_or(0);

    (max_x_and_girth + 20, max_y_and_girth + 20)
}
