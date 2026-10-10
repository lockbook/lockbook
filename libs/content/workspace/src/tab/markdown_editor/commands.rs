//! Editor actions a platform delivers as commands instead of pointer
//! events: taps on its touch targets, and a long-press reorder, decided by the
//! platform's own gesture system.

use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::input::Event;
use super::widget::block::drag::{BlockDragAction, TouchReorder};
use super::{MdEdit, TouchTarget};

impl MdEdit {
    /// The touch target painted under `pos` last frame: of several, the one
    /// painted last, which is on top.
    pub fn touch_target_at(&self, pos: Pos2) -> Option<&TouchTarget> {
        self.renderer
            .touch_targets
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, target)| target)
    }

    /// Tap the target under `pos`. Its event is queued as a click's would
    /// be, except a selection, which is the platform's to write: that range
    /// is returned instead.
    pub fn tap(&mut self, pos: Pos2) -> Option<(Grapheme, Grapheme)> {
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
