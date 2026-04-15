use crate::async_on_wasm;
use crate::file_cache::{FileCache, FilesExt as _, ResolvedLink};
use crate::tab::markdown_editor::HttpClient;
use egui::{
    Align2, Color32, ColorImage, Context, CursorIcon, FontId, Id, OpenUrl, Pos2, Rect, Sense,
    Stroke, TextureId, Ui, UiBuilder, Vec2,
};
use epaint::RectShape;
use lb_rs::blocking::Lb;
use lb_rs::{Uuid, spawn};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock};

use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;

pub trait EmbedResolver: Send + Clone {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn can_resolve(&self, url: &str) -> bool;
    fn height(&self, url: &str, max_size: Vec2) -> f32;
    fn show(&mut self, url: &str, rect: Rect, ui: &mut Ui);
    fn last_modified(&self) -> u64;
}

impl EmbedResolver for () {
    fn can_resolve(&self, _: &str) -> bool {
        false
    }

    fn height(&self, _: &str, _: Vec2) -> f32 {
        0.
    }

    fn show(&mut self, _: &str, _: Rect, _: &mut Ui) {}

    fn last_modified(&self) -> u64 {
        0
    }
}

#[derive(Clone)]
pub struct ImageCache {
    current: HashMap<String, Arc<Mutex<ImageState>>>,
    version: Arc<Mutex<u64>>,
    previous: HashMap<String, Arc<Mutex<ImageState>>>,

    ctx: Context,
    client: HttpClient,
    core: Lb,
    file_id: Uuid,
    files: Arc<RwLock<FileCache>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ImageState {
    #[default]
    Loading,
    Loaded(TextureId),
    Failed(String),
}

impl ImageCache {
    pub fn new(
        ctx: Context, client: HttpClient, core: Lb, file_id: Uuid, files: Arc<RwLock<FileCache>>,
    ) -> Self {
        Self {
            current: HashMap::new(),
            version: Arc::new(Mutex::new(0)),
            previous: HashMap::new(),
            ctx,
            client,
            core,
            file_id,
            files,
        }
    }

    fn ensure_loaded(&mut self, url: &str) {
        if self.current.contains_key(url) {
            return;
        }

        let image_state: Arc<Mutex<ImageState>> = Default::default();
        let client = self.client.clone();
        let core = self.core.clone();
        let ctx = self.ctx.clone();
        let version = self.version.clone();
        let url_owned = url.to_string();

        let maybe_lb_id = {
            let guard = self.files.read().unwrap();
            let from_id = guard.get_by_id(self.file_id).map(|f| f.parent);
            from_id.and_then(|from_id| match guard.resolve_link(&url_owned, from_id)? {
                ResolvedLink::File(id) => Some(id),
                ResolvedLink::External(_) => None,
            })
        };

        let viewport_width = self.ctx.screen_rect().width();
        let pixels_per_point = self.ctx.pixels_per_point();

        self.current.insert(url_owned.clone(), image_state.clone());

        spawn!({
            let texture_manager = ctx.tex_manager();

            let texture_closure = async_on_wasm!({
                let image_bytes = if let Some(id) = maybe_lb_id {
                    core.read_document(id, false).map_err(|e| e.to_string())?
                } else {
                    if !url_owned.starts_with("http://") && !url_owned.starts_with("https://") {
                        return Err(format!("image not found: {url_owned}"));
                    }
                    let bytes;
                    #[cfg(target_arch = "wasm32")]
                    {
                        bytes = download_image(&client, &url_owned)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        bytes = download_image(&client, &url_owned).map_err(|e| e.to_string())?
                    }

                    bytes
                };

                let is_svg = if let Some(id) = maybe_lb_id {
                    let file = core.get_file_by_id(id).map_err(|e| e.to_string())?;
                    file.name.ends_with(".svg")
                } else {
                    url_owned.ends_with(".svg")
                };

                let image_bytes = if is_svg {
                    let tree = usvg::Tree::from_data(
                        &image_bytes,
                        &Default::default(),
                        &Default::default(),
                    )
                    .map_err(|e| e.to_string())?;

                    let (w, h, base_transform) = if maybe_lb_id.is_some() {
                        let bb = tree.root().abs_bounding_box();
                        (
                            bb.width(),
                            bb.height(),
                            Transform::identity().post_translate(-bb.left(), -bb.top()),
                        )
                    } else {
                        let size = tree.size();
                        (size.width(), size.height(), Transform::default())
                    };

                    let scale = (viewport_width / w).min(1.0) * pixels_per_point;
                    let pix_w = (w * scale) as u32;
                    let pix_h = (h * scale) as u32;
                    let transform = if scale < 1.0 {
                        base_transform.post_scale(scale, scale)
                    } else {
                        base_transform
                    };

                    let mut pix_map = Pixmap::new(pix_w, pix_h)
                        .ok_or("failed to create pixmap")
                        .map_err(|e| e.to_string())?;
                    resvg::render(&tree, transform, &mut pix_map.as_mut());
                    pix_map.encode_png().map_err(|e| e.to_string())?
                } else {
                    image_bytes
                };

                let image = decode_with_orientation(&image_bytes).map_err(|e| e.to_string())?;
                let size_pixels = [image.width() as usize, image.height() as usize];

                let egui_image = egui::ImageData::Color(
                    ColorImage::from_rgba_unmultiplied(size_pixels, &image.to_rgba8()).into(),
                );

                Ok(texture_manager
                    .write()
                    .alloc(url_owned, egui_image, Default::default()))
            });

            #[cfg(target_arch = "wasm32")]
            let texture_result = texture_closure().await;

            #[cfg(not(target_arch = "wasm32"))]
            let texture_result = texture_closure();

            match texture_result {
                Ok(texture_id) => {
                    *image_state.lock().unwrap() = ImageState::Loaded(texture_id);
                }
                Err(err) => {
                    *image_state.lock().unwrap() = ImageState::Failed(err);
                }
            }

            *version.lock().unwrap() += 1;
            ctx.request_repaint();
        });
    }

    fn image_size(&self, texture_size: Vec2, width: f32, max_height: f32, margin: f32) -> Vec2 {
        let image_max_size = Vec2::new(width, max_height) - Vec2::splat(margin);

        let width = width.min(texture_size.x).min(image_max_size.x);
        let height = (texture_size.y * width / texture_size.x).min(image_max_size.y);
        let width = texture_size.x * height / texture_size.y;

        Vec2::new(width, height)
    }
}

impl EmbedResolver for ImageCache {
    fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }

    fn end_frame(&mut self) {
        let texture_manager = self.ctx.tex_manager();
        for (_, eviction) in self.previous.drain() {
            if let ImageState::Loaded(eviction) = eviction.lock().unwrap().deref() {
                texture_manager.write().free(*eviction);
            }
        }

        if self
            .current
            .values()
            .any(|s| &ImageState::Loading == s.lock().unwrap().deref())
        {
            self.ctx
                .request_repaint_after(std::time::Duration::from_millis(8));
        }
    }

    fn can_resolve(&self, url: &str) -> bool {
        self.current.contains_key(url)
            || self.previous.contains_key(url)
            || url.starts_with("http://")
            || url.starts_with("https://")
            || {
                let guard = self.files.read().unwrap();
                let from_id = guard.get_by_id(self.file_id).map(|f| f.parent);
                from_id
                    .and_then(|from_id| guard.resolve_link(url, from_id))
                    .is_some()
            }
    }

    fn height(&self, url: &str, max_size: Vec2) -> f32 {
        if let Some(image_state) = self.current.get(url).or_else(|| self.previous.get(url)) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => {
                    self.image_size(Vec2::splat(200.), max_size.x, max_size.y, 0.)
                        .y
                }
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;
                    self.image_size(
                        Vec2::new(image_width as _, image_height as _),
                        max_size.x,
                        max_size.y,
                        0.,
                    )
                    .y
                }
                ImageState::Failed(_) => {
                    self.image_size(Vec2::splat(200.), max_size.x, max_size.y, 0.)
                        .y
                }
            }
        } else {
            0.
        }
    }

    fn show(&mut self, url: &str, rect: Rect, ui: &mut Ui) {
        // Reclaim from evictions or start loading
        if !self.current.contains_key(url) {
            if let Some(cached) = self.previous.remove(url) {
                self.current.insert(url.to_string(), cached);
            } else {
                self.ensure_loaded(url);
            }
        }

        let max_size = rect.size();
        let Some(image_state) = self.current.get(url) else { return };
        let image_state = image_state.lock().unwrap().deref().clone();

        match image_state {
            ImageState::Loading => {
                let icon = Icon::IMAGE;
                let caption = "Loading image...";
                let size = self.image_size(Vec2::splat(200.), max_size.x, max_size.y, 0.);
                let draw_rect = Rect::from_min_size(rect.min, Vec2::new(max_size.x, size.y));

                ui.scope_builder(UiBuilder::new().max_rect(draw_rect), |ui| {
                    let theme = self.ctx.get_lb_theme();
                    ui.painter().text(
                        draw_rect.center(),
                        Align2::CENTER_CENTER,
                        icon.icon,
                        FontId { size: 48.0, family: egui::FontFamily::Monospace },
                        theme.neutral_fg_secondary(),
                    );
                    ui.painter().text(
                        draw_rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                        Align2::CENTER_BOTTOM,
                        caption,
                        FontId::default(),
                        theme.neutral_fg_secondary(),
                    );
                    ui.painter().rect_stroke(
                        draw_rect,
                        2.,
                        Stroke { width: 1., color: theme.neutral_bg_tertiary() },
                        egui::epaint::StrokeKind::Inside,
                    );
                });
            }
            ImageState::Loaded(texture_id) => {
                let [image_width, image_height] =
                    self.ctx.tex_manager().read().meta(texture_id).unwrap().size;
                let size = self.image_size(
                    Vec2::new(image_width as _, image_height as _),
                    max_size.x,
                    max_size.y,
                    0.,
                );
                let padding = (max_size.x - size.x) / 2.0;
                let image_top_left = rect.min + Vec2::new(padding, 0.);
                let image_rect = Rect::from_min_size(image_top_left, size);

                let resp = ui.interact(image_rect, Id::new(texture_id), Sense::click());
                if resp.hovered() {
                    ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    ui.ctx()
                        .open_url(OpenUrl { url: url.into(), new_tab: true });
                }

                ui.scope_builder(UiBuilder::new().max_rect(image_rect), |ui| {
                    ui.painter().add(
                        RectShape::filled(image_rect, 2.0_f32, Color32::WHITE).with_texture(
                            texture_id,
                            Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        ),
                    );
                });
            }
            ImageState::Failed(message) => {
                let icon = Icon::NO_IMAGE;
                let caption = message.clone();
                let size = self.image_size(Vec2::splat(200.), max_size.x, max_size.y, 0.);
                let draw_rect = Rect::from_min_size(rect.min, Vec2::new(max_size.x, size.y));

                ui.scope_builder(UiBuilder::new().max_rect(draw_rect), |ui| {
                    let theme = self.ctx.get_lb_theme();
                    ui.painter().text(
                        draw_rect.center(),
                        Align2::CENTER_CENTER,
                        icon.icon,
                        FontId { size: 48.0, family: egui::FontFamily::Monospace },
                        theme.neutral_fg_secondary(),
                    );
                    ui.painter().text(
                        draw_rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                        Align2::CENTER_BOTTOM,
                        caption,
                        FontId::default(),
                        theme.neutral_fg_secondary(),
                    );
                    ui.painter().rect_stroke(
                        draw_rect,
                        2.,
                        Stroke { width: 1., color: theme.neutral_bg_tertiary() },
                        egui::epaint::StrokeKind::Inside,
                    );
                });
            }
        }
    }

    fn last_modified(&self) -> u64 {
        *self.version.lock().unwrap()
    }
}

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! async_on_wasm {
    ($block:expr) => {
        (async || -> Result<TextureId, String> { $block })
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! async_on_wasm {
    ($block:expr) => {
        (|| -> Result<TextureId, String> { $block })
    };
}

use image::{DynamicImage, ImageDecoder};
use std::io::Cursor;

pub fn decode_with_orientation(image_bytes: &[u8]) -> Result<DynamicImage, String> {
    let reader = image::ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;

    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);

    Ok(img)
}

#[cfg(not(target_arch = "wasm32"))]
fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send()?.bytes()?.to_vec();
    Ok(response)
}

#[cfg(target_arch = "wasm32")]
async fn download_image(client: &HttpClient, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send().await?.bytes().await?.to_vec();
    Ok(response)
}
