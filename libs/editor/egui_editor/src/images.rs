use crate::element::Url;
use crate::layouts::{Annotation, Layouts};
use egui::{ColorImage, TextureId, Ui};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub struct ImageCache {
    pub map: HashMap<Url, TextureId>,
}

pub fn calc(
    layouts: &Layouts, prior_cache: &ImageCache, client: &reqwest::blocking::Client, ui: &Ui,
) -> ImageCache {
    let mut result = ImageCache::default();

    let mut prior_cache = prior_cache.clone();
    let texture_manager = ui.ctx().tex_manager();
    for layout in &layouts.layouts {
        if let Some(Annotation::Image(_, url, title)) = layout.annotation.clone() {
            if let Some(cached) = prior_cache.map.remove(&url) {
                // re-use image from previous cache
                result.map.insert(url, cached);
            } else {
                // download image
                let image_bytes = match download_image(client, &url) {
                    Ok(image_bytes) => image_bytes,
                    Err(_) => continue,
                };
                let image = match image::load_from_memory(&image_bytes) {
                    Ok(image) => image,
                    Err(_) => continue,
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

                result.map.insert(url, texture_id);
            }
        }
    }

    for (_, eviction) in prior_cache.map.drain() {
        texture_manager.write().free(eviction)
    }

    result
}

fn download_image(
    client: &reqwest::blocking::Client, url: &str,
) -> Result<Vec<u8>, reqwest::Error> {
    let response = client.get(url).send()?.bytes()?.to_vec();
    Ok(response)
}
