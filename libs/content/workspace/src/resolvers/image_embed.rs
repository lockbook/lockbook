//! Markdown adapter for the shared document image presentation.
use super::EmbedResolver;
pub use crate::widgets::document_image::{wiki_embed_key, wiki_embed_title};
use crate::widgets::{document_image::DocumentImage, image_cache::ImageCache};
use egui::{Context, CornerRadius, Rect, Ui, Vec2};
use lb_rs::Uuid;

pub struct ImageEmbedResolver(DocumentImage);
impl ImageEmbedResolver {
    pub fn new(images: ImageCache, file_id: Uuid) -> Self {
        Self(DocumentImage::new(images, file_id))
    }
}
impl EmbedResolver for ImageEmbedResolver {
    fn size(&self, url: &str) -> Vec2 {
        self.0.size(url)
    }
    fn is_loaded(&self, url: &str) -> bool {
        self.0.is_loaded(url)
    }
    fn withheld(&self, url: &str) -> bool {
        self.0.withheld(url)
    }
    fn take_pending(&self) -> bool {
        self.0.take_pending()
    }
    fn load_anyway(&self, url: &str) {
        self.0.load_anyway(url)
    }
    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: CornerRadius) {
        self.0.show(ui, url, rect, rounding)
    }
    fn prefetch(&self, url: &str) {
        self.0.prefetch(url)
    }
    fn seq(&self) -> u64 {
        self.0.seq()
    }
    fn copy_image(&self, url: &str, ctx: &Context) {
        self.0.copy_image(url, ctx)
    }
}
