use egui::{Rect, Ui, Vec2};

use crate::tab::markdown_editor::theme::Theme;

/// Initialize with an EmbedResolver to draw embedded content based on a URL.
/// The width for the embed will determined by the editor as the editor's width
/// minus the indentation for Markdown blocks that the embed is nested in.
pub trait EmbedResolver {
    /// Called at the beginning of the frame (a hook in case you need it)
    fn begin_frame(&mut self) {}

    /// Called at the end of the frame (a hook in case you need it)
    fn end_frame(&mut self) {}

    /// Can this url be resolved by this resolver?
    fn can_resolve(&self, url: &str) -> bool;

    /// How tall will the embed be for the given `url` with `max_size` space
    /// available? Used to place subsequent elements in the document; supports
    /// only rendering what's visible in the scroll view. The result is cached
    /// until the document or window size changes. The result must not exceed
    /// `max_size.y`.
    fn height(&self, url: &str, max_size: Vec2) -> f32;

    /// Show the embed. Just draw your embed in the rect; the Ui's cursor will
    /// not be in any particular state and any effect on the Ui's cursor will be
    /// ignored.
    fn show(&mut self, url: &str, rect: Rect, theme: &Theme, ui: &mut Ui);

    /// When did the state of the resolver last change in a way that affects how
    /// embeds should be layed out? Signals that layout should change e.g. when
    /// an image has completed loading. The particular value doesn't matter as
    /// long as it always goes up when the layout should change.
    fn last_modified(&self) -> u64;
}

impl EmbedResolver for () {
    fn can_resolve(&self, _: &str) -> bool {
        false
    }

    fn height(&self, _: &str, _: Vec2) -> f32 {
        0.
    }

    fn show(&mut self, _: &str, _: Rect, _: &Theme, _: &mut Ui) {}

    fn last_modified(&self) -> u64 {
        0
    }
}
