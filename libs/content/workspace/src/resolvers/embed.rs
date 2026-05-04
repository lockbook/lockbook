use std::collections::HashMap;

use egui::{Rect, Ui, Vec2};

/// Resolves embedded content (images, etc.) for the markdown editor.
/// The editor calls `size` during layout and `show` during rendering.
/// `()` is the no-op implementation.
pub trait EmbedResolver {
    fn size(&self, url: &str) -> Vec2;
    fn show(&self, ui: &mut Ui, url: &str, rect: Rect);
    fn warm(&self, url: &str);
    fn last_modified(&self) -> u64;
    fn image_dims(&self) -> HashMap<String, [f32; 2]>;
}

impl EmbedResolver for () {
    fn size(&self, _url: &str) -> Vec2 {
        Vec2::splat(200.)
    }
    fn show(&self, _ui: &mut Ui, _url: &str, _rect: Rect) {}
    fn warm(&self, _url: &str) {}
    fn last_modified(&self) -> u64 {
        0
    }
    fn image_dims(&self) -> HashMap<String, [f32; 2]> {
        HashMap::new()
    }
}
