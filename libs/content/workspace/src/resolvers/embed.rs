use std::collections::HashMap;

use egui::{Rect, Ui, Vec2};

/// Resolves embedded content (images, etc.) for the markdown editor.
pub trait EmbedResolver {
    /// Returns the size of the content at the url.
    fn size(&self, url: &str) -> Vec2;

    /// Shows the content in the provided Ui at the provided Rect.
    fn show(&self, ui: &mut Ui, url: &str, rect: Rect);

    /// Called per-frame while content may be shown soon so loading can start
    /// before it's time to show.
    fn warm(&self, url: &str);

    /// Increment this to signal when any return value could change.
    fn seq(&self) -> u64;

    /// Temporary hack that supports persisting image dimensions.
    fn image_dims(&self) -> HashMap<String, [f32; 2]>;
}

impl EmbedResolver for () {
    fn size(&self, _url: &str) -> Vec2 {
        Vec2::splat(200.)
    }
    fn show(&self, _ui: &mut Ui, _url: &str, _rect: Rect) {}
    fn warm(&self, _url: &str) {}
    fn seq(&self) -> u64 {
        0
    }
    fn image_dims(&self) -> HashMap<String, [f32; 2]> {
        HashMap::new()
    }
}
