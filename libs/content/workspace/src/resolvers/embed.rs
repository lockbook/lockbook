use egui::{CornerRadius, Rect, Ui, Vec2};

/// Resolves embedded content (images, etc.) for the markdown editor.
pub trait EmbedResolver {
    /// Returns the size of the content at the url.
    fn size(&self, url: &str) -> Vec2;

    /// Whether `url` has finished loading (so `show` paints it, not a
    /// placeholder). Lets callers defer tiny embeds — e.g. a favicon — until ready.
    fn is_loaded(&self, url: &str) -> bool;

    /// Shows the content in the provided Ui at the provided Rect, with the
    /// given corner rounding (e.g. a card hero rounds its top corners only).
    fn show(&self, ui: &mut Ui, url: &str, rect: Rect, rounding: CornerRadius);

    /// Called per-frame while content may be shown soon so loading can start
    /// before it's time to show.
    fn warm(&self, url: &str);

    /// Increment this to signal when any return value could change.
    fn seq(&self) -> u64;
}

impl EmbedResolver for () {
    fn size(&self, _url: &str) -> Vec2 {
        Vec2::splat(200.)
    }
    fn is_loaded(&self, _url: &str) -> bool {
        false
    }
    fn show(&self, _ui: &mut Ui, _url: &str, _rect: Rect, _rounding: CornerRadius) {}
    fn warm(&self, _url: &str) {}
    fn seq(&self) -> u64 {
        0
    }
}
