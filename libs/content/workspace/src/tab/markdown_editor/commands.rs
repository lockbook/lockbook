//! Editor actions a platform delivers as commands instead of pointer
//! events: taps on its touch targets, and a long-press reorder, decided by the
//! platform's own gesture system.

use super::input::{Location, Region};
use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::input::{Event, Gesture};
use super::widget::block::drag::{BlockDrag, BlockDragAction, DragUnit, TouchReorder};
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

    /// Takes a horizontal pan that began at `pos` for the table, code block,
    /// or diagram wider than the note there, if any: the platform then
    /// drives it with [`Self::row_pan_by`]. False while a lifted item or row
    /// is dragged.
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

    /// Whether a long press at `pos` can lift something (see
    /// [`MdRender::can_lift`]). The platform checks the keyboard: with it up,
    /// a long press is text selection.
    pub fn reorder_can_start(&self, pos: Pos2) -> bool {
        let r = &self.renderer;
        r.touch_mode && !r.readonly && r.interactive && r.can_lift(pos)
    }

    /// Lift the image or list item under `pos`; the platform then drives
    /// the drag.
    pub fn reorder_start(&mut self, pos: Pos2) -> bool {
        if !self.lift(pos) {
            return false;
        }
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

    /// Drop at `pos`, or put the image or item back if `cancelled`.
    pub fn reorder_end(&mut self, pos: Pos2, cancelled: bool) {
        if !self.touch_reorder_driven {
            return;
        }
        if cancelled {
            self.in_progress_block_drag = None;
            self.in_progress_image_drag = None;
            self.renderer.block_drag_action = None;
        } else {
            self.renderer.block_drag_action = Some(BlockDragAction::Released(pos));
            if let Some(drag) = &mut self.in_progress_image_drag {
                drag.pointer = pos;
            }
        }
        self.touch_reorder = TouchReorder::Idle;
        self.touch_reorder_driven = false;
        self.renderer.ctx.request_repaint();
    }

    /// A standalone field (the chat composer) scrolls its overflow; a note
    /// scrolls its page.
    pub fn apply_gesture(&mut self, gesture: Gesture) {
        if self.standalone {
            if let Gesture::Scroll { dy } | Gesture::Crawl { dy } = gesture {
                self.overflow_scroll_by(-dy);
            }
            return;
        }
        match gesture {
            Gesture::Scroll { dy } => self.scroll_area.gesture_scroll(-dy),
            Gesture::Fling { vy } => self.scroll_area.gesture_fling(-vy),
            Gesture::Crawl { dy } => self.crawl_by(-dy),
            Gesture::ScrollbarBegin => self.scroll_area.gesture_scrollbar_begin(),
            Gesture::ScrollbarDrag { dy } => self.scroll_area.gesture_scrollbar_drag(dy),
            Gesture::RowPan { dx } => self.row_pan_by(dx),
            Gesture::RowPanEnd { vx } => self.row_pan_end(vx),
            Gesture::ReorderMove { pos } => self.reorder_move(pos),
            Gesture::ReorderEnd { pos, cancelled } => self.reorder_end(pos, cancelled),
        }
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

    /// Follows a drag's finger or pointer down: its downward travel adds up,
    /// whichever way it moves between, and starts over with the next drag.
    pub(crate) fn track_drag_travel(&mut self, ui: &egui::Ui) {
        // armed, a lift's first frame counts before its drag is taken up
        let armed = matches!(self.touch_reorder, TouchReorder::Armed { .. });
        let dragging =
            armed || self.in_progress_block_drag.is_some() || self.in_progress_image_drag.is_some();
        if !dragging {
            self.drag_travel = (0.0, None);
            return;
        }
        let Some(y) = self.drag_pointer(ui).map(|pointer| pointer.y) else { return };
        let (travel, last) = self.drag_travel;
        let down = last.map_or(0.0, |last| (y - last).max(0.0));
        self.drag_travel = (travel + down, Some(y));
    }

    /// How far above the finger or pointer a drop aims, holding something
    /// grabbed `grab` below its top: nothing at the pickup, then as much as it
    /// has traveled down, up to just above what it carries, as iOS's loupe
    /// lags, so the floating copy never hides where it lands. A drop where
    /// the thing was picked up lands there.
    pub(crate) fn drag_lift(&self, grab: f32) -> f32 {
        self.drag_travel.0.min(grab + self.renderer.drop_lift())
    }

    /// Where a block drag's pointer counts as, for its drop: where it puts
    /// the floating card's top [`Self::drag_lift`] above the finger. A
    /// column drops along its row.
    pub(crate) fn block_drop_pointer(&self, drag: &BlockDrag, pointer: Pos2) -> Pos2 {
        let lifts = match drag.unit {
            DragUnit::Item | DragUnit::Row => self.renderer.touch_mode,
            DragUnit::Column(_) => false,
        };
        match lifts {
            true => {
                pointer + egui::vec2(0.0, drag.grab_offset.y - self.drag_lift(drag.grab_offset.y))
            }
            false => pointer,
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
