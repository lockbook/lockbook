//! Read-only markdown label — wraps [`MdRender`] for rendering arbitrary
//! markdown snippets as passive UI content (chat messages, toolbar previews,
//! search previews). No cursor, no input processing, no widgets.
//!
//! The label owns an [`MdRender`] so its layout / syntax-highlight caches
//! persist across frames. The caller sets the content per call — the buffer
//! is replaced each time, which invalidates the text-change cache inside
//! `LayoutCache`, but that's inherent to label-style use.

use comrak::Arena;
use egui::{Pos2, Rect, Ui, UiBuilder, Vec2};
use lb_rs::model::text::buffer::Buffer;

use crate::TextBufferArea;

use super::MdRender;

pub struct MdLabel {
    pub renderer: MdRender,
}

impl MdLabel {
    pub fn new(ctx: egui::Context) -> Self {
        Self { renderer: MdRender::empty(ctx) }
    }

    /// Parse `md` at `width` and return the rendered height. Call before
    /// `show` to measure; the AST parse is cheap but not memoized.
    pub fn height(&mut self, md: &str, width: f32) -> f32 {
        self.prepare(md, width);
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);
        self.renderer.height(root, &[root])
    }

    /// Render `md` into `ui` at the current cursor position, wrapping at
    /// `width`. Advances `ui`'s cursor past the rendered block. Returns the
    /// shaped text areas for the caller to submit via `GlyphonRendererCallback`.
    pub fn show(&mut self, ui: &mut Ui, md: &str, width: f32) -> Vec<TextBufferArea> {
        let top_left = ui.min_rect().min;
        let (text_areas, rect) = self.paint_at(ui, md, top_left, width);
        ui.advance_cursor_after_rect(rect);
        text_areas
    }

    /// Render at an absolute `top_left` without advancing the caller's layout
    /// cursor — for manual-layout contexts (e.g. a chat transcript that
    /// positions each bubble via a running y coordinate). Returns the shaped
    /// text areas and the rendered rect.
    pub fn paint_at(
        &mut self, ui: &mut Ui, md: &str, top_left: Pos2, width: f32,
    ) -> (Vec<TextBufferArea>, Rect) {
        self.renderer.dark_mode = ui.style().visuals.dark_mode;
        self.prepare(md, width);
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);

        let height = self.renderer.height(root, &[root]);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        self.renderer.galleys.galleys.clear();
        self.renderer.bounds.wrap_lines.clear();
        self.renderer.text_areas.clear();

        // Scoped ui for clipping; the scope's cursor side-effects stay local.
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            self.renderer.show_block(ui, root, top_left, &[root]);
        });

        (std::mem::take(&mut self.renderer.text_areas), rect)
    }

    fn prepare(&mut self, md: &str, width: f32) {
        self.renderer.width = width;
        self.renderer.buffer = Buffer::from(md);
        self.renderer.layout_cache.invalidate_text_change();
    }

    /// Widest galley rect from the last render — for callers that size a
    /// container to fit the rendered content (e.g. chat bubble width).
    /// Returns 0 when nothing has rendered yet.
    pub fn rendered_width(&self) -> f32 {
        self.renderer
            .galleys
            .galleys
            .iter()
            .map(|g| g.rect.width())
            .fold(0.0_f32, f32::max)
    }
}
