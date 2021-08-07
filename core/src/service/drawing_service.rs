use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::file_repo;
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
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use uuid::Uuid;

#[derive(Deserialize)]
pub enum SupportedImageFormats {
    Png,
    Jpeg,
    Pnm,
    Tga,
    Farbfeld,
    Bmp,
}

macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}

pub fn save_drawing(config: &Config, id: Uuid, drawing_bytes: &[u8]) -> Result<(), CoreError> {
    let drawing_string = String::from(String::from_utf8_lossy(&drawing_bytes));

    // validate drawing
    match serde_json::from_str::<Drawing>(drawing_string.as_str()) {
        Ok(_) => {}
        Err(e) => match e.classify() {
            Category::Io => return Err(CoreError::Unexpected(String::from("json io"))),
            Category::Syntax | Category::Data | Category::Eof => {
                return Err(CoreError::DrawingInvalid);
            }
        },
    };

    let metadata = file_repo::get_metadata(config, RepoSource::Local, id)?;
    file_repo::insert_document(config, RepoSource::Local, &metadata, drawing_bytes)
}

pub fn get_drawing(config: &Config, id: Uuid) -> Result<Drawing, CoreError> {
    let drawing_bytes = file_repo::get_document(config, RepoSource::Local, id)?;
    let drawing_string = String::from(String::from_utf8_lossy(&drawing_bytes));

    match serde_json::from_str::<Drawing>(drawing_string.as_str()) {
        Ok(d) => Ok(d),
        Err(e) => match e.classify() {
            Category::Io => Err(CoreError::Unexpected(String::from("json io"))),
            Category::Syntax | Category::Data | Category::Eof => Err(CoreError::DrawingInvalid),
        },
    }
}

pub fn export_drawing(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, CoreError> {
    let drawing = get_drawing(config, id)?;

    let theme = match render_theme {
        Some(theme) => theme,
        None => match drawing.theme {
            None => hashmap![
                ColorAlias::White => ColorRGB{r: 0xFF, g: 0xFF, b: 0xFF},
                ColorAlias::Black => ColorRGB{r: 0x00, g: 0x00, b: 0x00},
                ColorAlias::Red => ColorRGB{r: 0xFF, g: 0x00, b: 0x00},
                ColorAlias::Green => ColorRGB{r: 0x00, g: 0xFF, b: 0x00},
                ColorAlias::Yellow => ColorRGB{r: 0xFF, g: 0xFF, b: 0x00},
                ColorAlias::Blue => ColorRGB{r: 0x00, g: 0x00, b: 0xFF},
                ColorAlias::Magenta => ColorRGB{r: 0xFF, g: 0x00, b: 0xFF},
                ColorAlias::Cyan => ColorRGB{r: 0x00, g: 0xFF, b: 0xFF}
            ],
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

pub fn export_drawing_to_disk(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    location: String,
) -> Result<(), CoreError> {
    let drawing_bytes = export_drawing(config, id, format, render_theme)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?;

    file.write_all(drawing_bytes.as_slice())
        .map_err(CoreError::from)
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
    use crate::model::repo::RepoSource;
    use crate::model::state::temp_config;
    use crate::repo::{account_repo, file_repo};
    use crate::service::drawing_service::SupportedImageFormats;
    use crate::service::{drawing_service, file_encryption_service, file_service};
    use lockbook_crypto::pubkey;
    use lockbook_models::account::Account;
    use lockbook_models::drawing::{ColorAlias, Drawing, Stroke};
    use lockbook_models::file_metadata::FileType::{Document, Folder};

    #[test]
    fn test_drawing_bounds() {
        let empty_drawing = Drawing {
            scale: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
            strokes: vec![],
            theme: None,
        };

        assert_eq!(
            drawing_service::get_drawing_bounds(empty_drawing.strokes.as_slice()),
            (20, 20)
        );

        let small_drawing = Drawing {
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
            drawing_service::get_drawing_bounds(small_drawing.strokes.as_slice()),
            (121, 121)
        );

        let large_drawing = Drawing {
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
            drawing_service::get_drawing_bounds(large_drawing.strokes.as_slice()),
            (2021, 2021)
        );
    }

    #[test]
    fn test_create_png_sanity_check() {
        let config = &temp_config();

        let keys = pubkey::generate_key();
        let account = Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: keys,
        };

        account_repo::insert(config, &account).unwrap();
        let root = file_service::create_root(&account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &root).unwrap();

        let folder = file_service::create(Folder, root.id, "folder", &account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &folder).unwrap();
        let document = file_service::create(Document, root.id, "doc", &account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &document).unwrap();

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

        file_repo::insert_document(
            config,
            RepoSource::Local,
            &document,
            &serde_json::to_vec(&drawing).unwrap(),
        )
        .unwrap();

        drawing_service::export_drawing(config, document.id, SupportedImageFormats::Png, None)
            .unwrap();
    }

    #[test]
    fn test_create_png_unequal_points_data_sanity_check() {
        let config = &temp_config();

        let keys = pubkey::generate_key();
        let account = Account {
            username: String::from("username"),
            api_url: "ftp://uranus.net".to_string(),
            private_key: keys,
        };

        account_repo::insert(config, &account).unwrap();
        let root = file_service::create_root(&account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &root).unwrap();

        let folder = file_service::create(Folder, root.id, "folder", &account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &folder).unwrap();
        let document = file_service::create(Document, root.id, "doc", &account.username);
        file_repo::insert_metadata(config, RepoSource::Local, &document).unwrap();

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

        file_repo::insert_document(
            config,
            RepoSource::Local,
            &document,
            &serde_json::to_vec(&drawing).unwrap(),
        )
        .unwrap();

        drawing_service::export_drawing(config, document.id, SupportedImageFormats::Png, None)
            .unwrap_err();
    }
}
