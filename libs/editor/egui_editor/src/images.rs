use crate::ast::Ast;
use crate::style::{InlineNode, MarkdownNode, Url};
use egui::{ColorImage, TextureId, Ui};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct ImageCache {
    pub map: HashMap<Url, Arc<Mutex<ImageState>>>,
}

#[derive(Clone, Debug, Default)]
pub enum ImageState {
    #[default]
    Loading,
    Loaded(TextureId),
    Failed,
}

pub fn calc(
    ast: &Ast, prior_cache: &ImageCache, client: &reqwest::blocking::Client, ui: &Ui,
) -> ImageCache {
    let mut result = ImageCache::default();

    let mut prior_cache = prior_cache.clone();
    for node in &ast.nodes {
        if let MarkdownNode::Inline(InlineNode::Image(_, url, title)) = &node.node_type {
            let (url, title) = (url.clone(), title.clone());

            if result.map.contains_key(&url) {
                // the second removal of the same image from the prior cache is always a cache miss and causes performance issues
                // we need to remove cache hits from the prior cache to avoid freeing them from the texture manager
                continue;
            }

            if let Some(cached) = prior_cache.map.remove(&url) {
                // re-use image from previous cache (even it if failed to load)
                result.map.insert(url, cached);
            } else {
                let url = url.clone();
                let image_state: Arc<Mutex<ImageState>> = Default::default();
                let client = client.clone();
                let ctx = ui.ctx().clone();

                result.map.insert(url.clone(), image_state.clone());

                // download image
                thread::spawn(move || {
                    let texture_manager = ctx.tex_manager();
                    let image_bytes = match download_image(&client, &url) {
                        Ok(image_bytes) => image_bytes,
                        Err(_) => {
                            *image_state.lock().unwrap() = ImageState::Failed;
                            ctx.request_repaint();
                            return;
                        }
                    };
                    let image = match image::load_from_memory(&image_bytes) {
                        Ok(image) => image,
                        Err(_) => {
                            *image_state.lock().unwrap() = ImageState::Failed;
                            ctx.request_repaint();
                            return;
                        }
                    };
                    let size_pixels = [image.width() as usize, image.height() as usize];

                    let egui_image = egui::ImageData::Color(ColorImage::from_rgba_unmultiplied(
                        size_pixels,
                        &image.to_rgba8(),
                    ));
                    let texture_id =
                        texture_manager
                            .write()
                            .alloc(title, egui_image, Default::default());

                    *image_state.lock().unwrap() = ImageState::Loaded(texture_id);

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

fn download_image(
    client: &reqwest::blocking::Client, url: &str,
) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send()?.bytes()?.to_vec();
    Ok(response)
}
