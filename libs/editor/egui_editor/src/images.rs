use crate::ast::Ast;
use crate::element::{InlineNode, MarkdownNode, Url};
use egui::{ColorImage, TextureId, Ui};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub struct ImageCache {
    pub map: HashMap<Url, TextureId>,
}

pub fn calc(
    ast: &Ast, prior_cache: &ImageCache, client: &reqwest::blocking::Client, ui: &Ui,
) -> ImageCache {
    let mut result = ImageCache::default();

    let mut prior_cache = prior_cache.clone();
    let texture_manager = ui.ctx().tex_manager();
    for node in &ast.nodes {
        if let MarkdownNode::Inline(InlineNode::Image(_, url, title)) = &node.node_type {
            let (url, title) = (url.clone(), title.clone());
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
