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
    /// before it's time to show. **This makes an outbound request for `url`** —
    /// it's the egress point reached from otherwise pure-looking layout code.
    fn prefetch(&self, url: &str);

    /// Increment this to signal when any return value could change.
    fn seq(&self) -> u64;

    /// Whether `url` waits for the reader to ask before it shows: it reaches
    /// a file past the note's scope, as a yellow link does.
    fn withheld(&self, _url: &str) -> bool {
        false
    }

    /// The reader asked to see a withheld `url`.
    fn load_anyway(&self, _url: &str) {}

    /// Put the image at `url` on the clipboard, once its pixels are read.
    fn copy_image(&self, _url: &str, _ctx: &egui::Context) {}
    /// Whether anything shown since the last call is still loading.
    fn take_pending(&self) -> bool {
        false
    }
}

impl EmbedResolver for () {
    fn size(&self, _url: &str) -> Vec2 {
        Vec2::splat(200.)
    }
    fn is_loaded(&self, _url: &str) -> bool {
        false
    }
    fn show(&self, _ui: &mut Ui, _url: &str, _rect: Rect, _rounding: CornerRadius) {}
    fn prefetch(&self, _url: &str) {}
    fn seq(&self) -> u64 {
        0
    }
}
