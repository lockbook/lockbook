use crate::tab;
use crate::tab::markdown_editor::embed::EmbedResolver;
use crate::tab::markdown_editor::theme::Theme;
use egui::{
    self, Align2, Color32, ColorImage, Context, CursorIcon, FontId, Id, OpenUrl, Pos2, Rect, Sense,
    Stroke, TextureId, Ui, Vec2,
};
use epaint::RectShape;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use reqwest::blocking::Client;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::instrument;

/// A read-through cache that fetches an image on the first call to
/// [`ImageCache::get`]. On a cache miss, a repaint is requested after a short
/// delay; the caller is expected to retry next frame until the image is loaded.
#[derive(Clone)]
pub struct ImageCache {
    images: HashMap<String, Arc<Mutex<ImageState>>>,
    used_this_frame: HashSet<String>, // images cached until first frame unused
    last_modified: Arc<Mutex<u64>>,

    // supporting resources
    ctx: Context,
    client: Client,
    lb: Lb,
    file_id: Uuid, // used as the base for relative links
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ImageState {
    #[default]
    Loading,
    Loaded(TextureId),
    Failed(String),
}

impl ImageCache {
    pub fn new(ctx: Context, client: Client, lb: Lb, file_id: Uuid) -> Self {
        Self {
            images: Default::default(),
            used_this_frame: Default::default(),
            last_modified: Default::default(),
            ctx,
            client,
            lb,
            file_id,
        }
    }

    pub fn get(&mut self, url: &str) -> ImageState {
        if let Some(image_state) = self.images.get(url) {
            image_state.lock().unwrap().deref().clone()
        } else {
            let image_state: Arc<Mutex<ImageState>> = Default::default();

            self.images.insert(url.into(), image_state.clone());

            // launch image fetch
            let self_clone = self.clone();
            let url_clone = url.to_string();
            let image_state_clone = image_state.clone();
            thread::spawn(move || self_clone.background_fetch(url_clone, image_state_clone));

            let result = image_state.lock().unwrap().deref().clone();
            result
        }
    }

    /// Runs on a background thread to fetch an image from lb_rs or the web. On
    /// completion, updates the cache, requests a new frame, and updates
    /// self.last_modified.
    #[instrument(level = "debug", skip(self, image_state), fields(thread = format!("{:?}", thread::current().id())))]
    fn background_fetch(&self, url: String, image_state: Arc<Mutex<ImageState>>) {
        let texture_manager = self.ctx.tex_manager();

        let texture_result = (|| -> Result<TextureId, String> {
            // use core for lb:// urls and relative paths
            let maybe_lb_id = match url.strip_prefix("lb://") {
                Some(id) => Some(Uuid::parse_str(id).map_err(|e| e.to_string())?),
                None => {
                    let parent_id = self
                        .lb
                        .get_file_by_id(self.file_id)
                        .map_err(|e| e.to_string())?
                        .parent;

                    tab::core_get_by_relative_path(&self.lb, parent_id, PathBuf::from(&url))
                        .map(|f| f.id)
                        .ok()
                }
            };

            let image_bytes = if let Some(id) = maybe_lb_id {
                self.lb
                    .read_document(id, false)
                    .map_err(|e| e.to_string())?
            } else {
                self.download_image(&url).map_err(|e| e.to_string())?
            };

            // convert lockbook drawings to images
            let image_bytes = if let Some(id) = maybe_lb_id {
                let file = self.lb.get_file_by_id(id).map_err(|e| e.to_string())?;
                if file.name.ends_with(".svg") {
                    // todo: check errors
                    let tree = usvg::Tree::from_data(
                        &image_bytes,
                        &Default::default(),
                        &Default::default(),
                    )
                    .map_err(|e| e.to_string())?;

                    let bounding_box = tree.root().abs_bounding_box();

                    // dimensions & transform chosen so that all svg content appears in the result
                    let mut pix_map =
                        Pixmap::new(bounding_box.width() as _, bounding_box.height() as _)
                            .ok_or("failed to create pixmap")
                            .map_err(|e| e.to_string())?;
                    let transform = Transform::identity()
                        .post_translate(-bounding_box.left(), -bounding_box.top());
                    resvg::render(&tree, transform, &mut pix_map.as_mut());
                    pix_map.encode_png().map_err(|e| e.to_string())?
                } else {
                    // leave non-drawings alone
                    image_bytes
                }
            } else {
                // leave non-lockbook images alone
                image_bytes
            };

            let image = image::load_from_memory(&image_bytes).map_err(|e| e.to_string())?;
            let size_pixels = [image.width() as usize, image.height() as usize];

            let egui_image = egui::ImageData::Color(
                ColorImage::from_rgba_unmultiplied(size_pixels, &image.to_rgba8()).into(),
            );

            Ok(texture_manager
                .write()
                .alloc(url, egui_image, Default::default()))
        })();

        match texture_result {
            Ok(texture_id) => {
                *image_state.lock().unwrap() = ImageState::Loaded(texture_id);
            }
            Err(err) => {
                *image_state.lock().unwrap() = ImageState::Failed(err);
            }
        }

        *self.last_modified.lock().unwrap() = self.ctx.frame_nr();

        // request a frame when the image is done loading
        self.ctx.request_repaint();
    }

    fn download_image(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
        let response = self.client.get(url).send()?.bytes()?.to_vec();
        Ok(response)
    }
}

impl Drop for ImageCache {
    fn drop(&mut self) {
        let texture_manager = self.ctx.tex_manager();
        for (_, eviction) in self.images.drain() {
            if let ImageState::Loaded(eviction) = eviction.lock().unwrap().deref() {
                texture_manager.write().free(*eviction);
            }
        }
    }
}

impl EmbedResolver for ImageCache {
    fn begin_frame(&mut self) {
        self.used_this_frame.clear();
    }

    fn end_frame(&mut self) {
        let texture_manager = self.ctx.tex_manager();
        for (_, eviction) in self
            .images
            .extract_if(|k, _| !self.used_this_frame.contains(k))
        {
            if let ImageState::Loaded(eviction) = eviction.lock().unwrap().deref() {
                texture_manager.write().free(*eviction);
            }
        }
    }

    fn can_resolve(&self, url: &str) -> bool {
        url.strip_prefix("http://").is_some()
            || url.strip_prefix("https://").is_some()
            || url.strip_prefix("lb://").is_some() // todo: or is relative path
    }

    fn height(&self, url: &str, max_size: Vec2) -> f32 {
        if let Some(image_state) = self.images.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => image_size(Vec2::splat(200.), max_size).y,
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;
                    let size = image_size(Vec2::new(image_width as _, image_height as _), max_size);

                    size.y
                }
                ImageState::Failed(_) => image_size(Vec2::splat(200.), max_size).y,
            }
        } else {
            0.
        }
    }

    fn show(&mut self, url: &str, rect: Rect, theme: &Theme, ui: &mut Ui) {
        self.used_this_frame.insert(url.to_string());

        let top_left = rect.left_top();
        let width = rect.size().x;

        if let Some(image_state) = self.images.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => {
                    let icon = "\u{e410}";
                    let caption = "Loading image...";

                    let size = image_size(Vec2::splat(200.), rect.size());
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: theme.bg().neutral_tertiary },
                        );
                    });
                }
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;

                    let size =
                        image_size(Vec2::new(image_width as _, image_height as _), rect.size());
                    let padding = (width - size.x) / 2.0;
                    let image_top_left = top_left + Vec2::new(padding, 0.);
                    let rect = Rect::from_min_size(image_top_left, size);

                    let resp = ui.interact(rect, Id::new(texture_id), Sense::click());
                    if resp.hovered() {
                        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        ui.output_mut(|o| o.open_url = Some(OpenUrl::new_tab(url)));
                    }

                    ui.allocate_ui_at_rect(rect, |ui| {
                        ui.painter().add(RectShape {
                            rect,
                            rounding: (2.).into(),
                            fill: Color32::WHITE,
                            stroke: Stroke::NONE,
                            blur_width: 0.0,
                            fill_texture_id: texture_id,
                            uv: Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        });
                    });
                }
                ImageState::Failed(message) => {
                    let icon = "\u{f116}";
                    let caption = format!("Could not show image: {message}");

                    let size = image_size(Vec2::splat(200.), rect.size());
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: theme.bg().neutral_tertiary },
                        );
                    });
                }
            }
        }
    }

    fn last_modified(&self) -> u64 {
        *self.last_modified.lock().unwrap().deref()
    }
}

fn image_size(texture_size: Vec2, max_size: Vec2) -> Vec2 {
    // make sure images can be viewed in full by capping their height and width to the viewport
    // todo: though great on mobile, images look too big on desktop

    let width_capped_size = Vec2::new(max_size.x, texture_size.y * max_size.x / texture_size.x);
    let height_capped_size = Vec2::new(texture_size.x * max_size.y / texture_size.y, max_size.y);

    if width_capped_size.length() < height_capped_size.length() {
        width_capped_size
    } else {
        height_capped_size
    }
}
