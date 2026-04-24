use crate::file_cache::{FilesExt as _, ResolvedLink};
use crate::tab::markdown_editor::HttpClient;
use comrak::nodes::{AstNode, NodeLink, NodeValue};
use egui::{ColorImage, Context, TextureId, Ui};
use lb_rs::blocking::Lb;
use lb_rs::{Uuid, spawn};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};
use std::collections::HashMap;
use std::ops::Deref;

use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct ImageCache {
    pub map: HashMap<String, Arc<Mutex<ImageState>>>,
    pub updated: Arc<Mutex<bool>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ImageState {
    #[default]
    Loading,
    Loaded(TextureId),
    Failed(String),
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

pub fn calc<'ast>(
    root: &'ast AstNode<'ast>, prior_cache: &ImageCache, client: &HttpClient, core: &Lb,
    file_id: Uuid, files: &std::sync::Arc<std::sync::RwLock<crate::file_cache::FileCache>>,
    ui: &Ui,
) -> ImageCache {
    let mut result = ImageCache::default();
    let mut prior_cache = prior_cache.clone();
    result.updated = prior_cache.updated;

    for node in root.descendants() {
        if let NodeValue::Image(node_link) = &node.data.borrow().value {
            let NodeLink { url, .. } = &**node_link;

            if result.map.contains_key(url) {
                // the second removal of the same image from the prior cache is always a cache miss and causes performance issues
                // we need to remove cache hits from the prior cache to avoid freeing them from the texture manager
                continue;
            }

            let cached = prior_cache.map.remove(url);

            // For local (non-http) images we may have previously cached a failure because the
            // file tree cache wasn't refreshed yet (e.g. paste-imported image + inline render in
            // the same frame). If the link resolves now, retry instead of pinning the failure
            // forever.
            let use_cached = if let Some(cached) = &cached {
                let is_local = !url.starts_with("http://") && !url.starts_with("https://");
                if is_local && matches!(*cached.lock().unwrap(), ImageState::Failed(_)) {
                    let guard = files.read().unwrap();
                    let from_id = guard.get_by_id(file_id).map(|f| f.parent);
                    let resolves_now = from_id
                        .and_then(|from_id| guard.resolve_link(url, from_id))
                        .is_some_and(|l| matches!(l, ResolvedLink::File(_)));
                    !resolves_now
                } else {
                    true
                }
            } else {
                false
            };

            if use_cached {
                // re-use image from previous cache (even it if failed to load)
                result.map.insert(url.clone(), cached.unwrap());
            } else {
                let url = url.clone();
                let image_state: Arc<Mutex<ImageState>> = Default::default();
                let client = client.clone();
                let core = core.clone();
                let ctx = ui.ctx().clone();
                let updated = result.updated.clone();

                let maybe_lb_id = {
                    let guard = files.read().unwrap();
                    let from_id = guard.get_by_id(file_id).map(|f| f.parent);
                    from_id.and_then(|from_id| match guard.resolve_link(&url, from_id)? {
                        ResolvedLink::File(id) => Some(id),
                        ResolvedLink::External(_) => None,
                    })
                };

                let viewport_width = ui.available_width();
                let pixels_per_point = ui.ctx().pixels_per_point();

                result.map.insert(url.clone(), image_state.clone());
                // fetch image
                spawn!({
                    let texture_manager = ctx.tex_manager();

                    let texture_closure = async_on_wasm!({
                        let image_bytes = if let Some(id) = maybe_lb_id {
                            core.read_document(id, false).map_err(|e| e.to_string())?
                        } else {
                            if !url.starts_with("http://") && !url.starts_with("https://") {
                                return Err(format!("image not found: {url}"));
                            }
                            let bytes;
                            #[cfg(target_arch = "wasm32")]
                            {
                                bytes = download_image(&client, &url)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                bytes = download_image(&client, &url).map_err(|e| e.to_string())?
                            }

                            bytes
                        };

                        // convert svgs to rasterized images
                        let is_svg = if let Some(id) = maybe_lb_id {
                            let file = core.get_file_by_id(id).map_err(|e| e.to_string())?;
                            file.name.ends_with(".svg")
                        } else {
                            url.ends_with(".svg")
                        };

                        let image_bytes = if is_svg {
                            let tree = usvg::Tree::from_data(
                                &image_bytes,
                                &Default::default(),
                                &Default::default(),
                            )
                            .map_err(|e| e.to_string())?;

                            let (w, h, base_transform) = if maybe_lb_id.is_some() {
                                // lockbook drawings don't have meaningful dimensions,
                                // use bounding box to capture all content
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

                            // cap to viewport width, then scale up for DPI
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

                        let image =
                            decode_with_orientation(&image_bytes).map_err(|e| e.to_string())?;
                        let size_pixels = [image.width() as usize, image.height() as usize];

                        let egui_image = egui::ImageData::Color(
                            ColorImage::from_rgba_unmultiplied(size_pixels, &image.to_rgba8())
                                .into(),
                        );

                        Ok(texture_manager
                            .write()
                            .alloc(url, egui_image, Default::default()))
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

                    *updated.lock().unwrap() = true;

                    // request a frame when the image is done loading
                    ctx.request_repaint();
                });
            }
        }
    }

    let texture_manager = ui.ctx().tex_manager();
    for (_, eviction) in prior_cache.map.drain() {
        if let ImageState::Loaded(eviction) = eviction.lock().unwrap().deref() {
            texture_manager.write().free(*eviction);
        }
    }

    result
}

use image::{DynamicImage, ImageDecoder};
use std::io::Cursor;

pub fn decode_with_orientation(image_bytes: &[u8]) -> Result<DynamicImage, String> {
    // Create a reader so we can access the decoder (and its metadata)
    let reader = image::ImageReader::new(Cursor::new(image_bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;

    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;

    // Read orientation (usually from Exif; if not present, this is NoTransforms)
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;

    // Decode pixels
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;

    // Apply rotation/flip in-place
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

impl ImageCache {
    pub fn any_loading(&self) -> bool {
        self.map
            .values()
            .any(|state| &ImageState::Loading == state.lock().unwrap().deref())
    }

    pub fn free(&mut self, ctx: &Context) {
        let texture_manager = ctx.tex_manager();
        for (_, eviction) in self.map.drain() {
            if let ImageState::Loaded(eviction) = eviction.lock().unwrap().deref() {
                texture_manager.write().free(*eviction);
            }
        }
    }
}
