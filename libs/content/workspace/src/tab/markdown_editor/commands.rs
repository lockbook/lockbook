//! Editor actions a platform delivers as commands instead of pointer
//! events: taps on its touch targets, and a long-press reorder, decided by the
//! platform's own gesture system.

use super::input::{Location, Region};
use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::input::Event;
use super::widget::block::drag::{BlockDragAction, TouchReorder};
use super::{MdEdit, TouchTarget};

impl MdEdit {
    /// The touch target painted under `pos` last frame.
    pub fn touch_target_at(&self, pos: Pos2) -> Option<&TouchTarget> {
        self.renderer
            .touch_targets
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, target)| target)
    }

    /// Tap the target under `pos`. Its event is queued as a click's would
    /// be, except a selection, which is the platform's to write: that range
    /// is returned instead.
    pub fn tap(&mut self, pos: Pos2) -> Option<(Grapheme, Grapheme)> {
        // a tap that edits a table — on a cell its row lacks, or below a
        // trailing table — is the next frame's to make; the host's own
        // caret placement comes first and the edit's selection after it
        let region = Region::Location(Location::Pos(pos));
        if self.table_select_edits(region, &mut Vec::new()) {
            self.renderer.render_events.push(Event::Select { region });
            self.renderer.ctx.request_repaint();
            return None;
        }
        let Some(TouchTarget::Tap(event)) = self.touch_target_at(pos).cloned() else {
            return None;
        };
        self.renderer.ctx.request_repaint();
        match event {
            Event::Select { region } => Some(self.region_to_range(region)),
            event => {
                self.renderer.render_events.push(event);
                None
            }
        }
    }

    /// Takes a horizontal pan that began at `pos` for the table or code
    /// block wider than the note there, if any: the platform then drives it
    /// with [`Self::row_pan_by`]. False while a lifted item or row is
    /// dragged.
    pub fn row_pan_begin(&mut self, pos: Pos2) -> bool {
        let dragging = self.in_progress_block_drag.is_some()
            || !matches!(self.touch_reorder, TouchReorder::Idle);
        let scroller = self.renderer.row_scroller_at(pos).filter(|_| !dragging);
        self.platform_row_pan = scroller;
        scroller.is_some()
    }

    /// Pans the block taken by [`Self::row_pan_begin`] by `dx` points of
    /// finger travel. The frame clamps it to the block's width.
    pub fn row_pan_by(&mut self, dx: f32) {
        let scroll = self
            .platform_row_pan
            .and_then(|key| self.renderer.row_scrolls.get_mut(&key));
        if let Some(scroll) = scroll.filter(|_| dx.is_finite()) {
            scroll.offset -= dx;
            self.renderer.ctx.request_repaint();
        }
    }

    /// The pan ended at `vx` points per second: the block coasts from it.
    pub fn row_pan_end(&mut self, vx: f32) {
        if let Some(key) = self.platform_row_pan.take() {
            if let Some(scroll) = self.renderer.row_scrolls.get_mut(&key) {
                scroll.velocity = if vx.is_finite() { -vx } else { 0.0 };
                self.renderer.ctx.request_repaint();
            }
        }
    }

    /// Whether a long press at `pos` can lift a list item. The platform
    /// checks the keyboard: with it up, a long press is text selection.
    pub fn reorder_can_start(&self, pos: Pos2) -> bool {
        let r = &self.renderer;
        r.touch_mode && !r.readonly && r.interactive && r.touch_reorder_target(pos).is_some()
    }

    /// Lift the list item under `pos`; the platform then drives the drag.
    pub fn reorder_start(&mut self, pos: Pos2) -> bool {
        let Some(drag) = self.renderer.touch_reorder_target(pos) else { return false };
        self.renderer.block_drag_action = Some(BlockDragAction::Started(drag));
        self.touch_reorder = TouchReorder::Armed { last: pos };
        self.touch_reorder_driven = true;
        self.renderer.ctx.request_repaint();
        true
    }

    pub fn reorder_move(&mut self, pos: Pos2) {
        if self.touch_reorder_driven {
            self.touch_reorder = TouchReorder::Armed { last: pos };
            self.renderer.ctx.request_repaint();
        }
    }

    /// Drop at `pos`, or put the item back if `cancelled`.
    pub fn reorder_end(&mut self, pos: Pos2, cancelled: bool) {
        if !self.touch_reorder_driven {
            return;
        }
        if cancelled {
            self.in_progress_block_drag = None;
            self.renderer.block_drag_action = None;
        } else {
            self.renderer.block_drag_action = Some(BlockDragAction::Released(pos));
        }
        self.touch_reorder = TouchReorder::Idle;
        self.touch_reorder_driven = false;
        self.renderer.ctx.request_repaint();
    }

    /// A crawl at the viewport's edge scrolls like a pan.
    pub fn crawl_by(&mut self, precise_pixels: f32) {
        self.scroll_area.gesture_scroll(precise_pixels);
    }

    /// Scroll a standalone field (the chat composer) whose content outgrew
    /// its rect; the frame clamps the far end.
    pub fn overflow_scroll_by(&mut self, precise_pixels: f32) {
        if precise_pixels.is_finite() {
            self.overflow_scroll = (self.overflow_scroll + precise_pixels).max(0.0);
        }
    }

    /// The live pointer of a block drag: the platform's while it drives
    /// the reorder, else egui's.
    pub(crate) fn drag_pointer(&self, ui: &egui::Ui) -> Option<Pos2> {
        match self.touch_reorder {
            TouchReorder::Armed { last } if self.touch_reorder_driven => Some(last),
            _ => ui.input(|i| i.pointer.latest_pos()),
        }
    }
}
