//! Tables wider than the note scroll along their rows: a wheel over one, a
//! touch drag that sets out sideways, or the platform's own pan recognizer
//! moves it, and it coasts after a fling. It follows what the user is shown:
//! a find or preview match, or the end of a selection the user moved. An
//! edge with more beyond it fades out.

use egui::{Color32, Pos2, Rangef, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::style::EDGE_FADE;
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::theme::palette_v2::ThemeExt as _;

/// A block that scrolls along its rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowScroller {
    /// A table, by its place among the tables.
    Table(usize),
}

/// Where a block wider than the note is scrolled to along its rows.
#[derive(Clone, Copy, Debug, Default)]
pub struct RowScroll {
    pub offset: f32,
    /// How fast it is coasting after a touch pan, in points a second of
    /// offset, decaying frame by frame.
    pub velocity: f32,
    /// The pan's speed while a finger drives it, which it coasts from.
    pub tracked: f32,
    /// The find and preview matches it last scrolled to show.
    pub shown: [Option<(Grapheme, Grapheme)>; 2],
}

impl MdRender {
    /// Forgets what the last frame painted of scrolled blocks and the
    /// chrome over them.
    pub fn clear_row_scroll_frame(&mut self) {
        self.row_scrollers.clear();
        self.edge_fades.clear();
    }

    /// How far `key`, `content` wide and showing in `viewport`, is scrolled
    /// along its rows: where a wheel over it or a touch drag has left it, or
    /// where it shows the span `reveal` gives (in content coordinates) for an
    /// offset the user is shown — a match, or the end of a selection the user
    /// moved (`true`).
    pub fn row_scroll(
        &mut self, ui: &Ui, key: RowScroller, viewport: Rect, content: f32,
        reveal: impl Fn(&Self, Grapheme, bool) -> Option<Rangef>,
    ) -> f32 {
        let scrolled = self.row_scrolls.get(&key).copied();
        let mut scroll = scrolled.unwrap_or_default();

        if ui.rect_contains_pointer(viewport) {
            let wheel = ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.x));
            scroll.offset -= wheel;
            if wheel != 0.0 {
                scroll.velocity = 0.0;
            }
        }
        // coasting after a pan, slowing as the page does
        if scroll.velocity.abs() > 1.0 {
            const DECAY_PER_SEC: f32 = 4.0;
            let dt = ui.input(|i| i.stable_dt).max(0.0001);
            scroll.offset += scroll.velocity * dt;
            scroll.velocity *= (-DECAY_PER_SEC * dt).exp();
            ui.ctx().request_repaint();
        } else {
            scroll.velocity = 0.0;
        }

        let shown = [self.find_current_match, self.preview_match];
        let span = if shown != scroll.shown {
            let shown = shown.iter().flatten().next();
            shown.and_then(|shown| reveal(self, shown.start(), false))
        } else if self.caret_moved || scrolled.is_none() {
            reveal(self, self.buffer.current.selection.1, true)
        } else {
            None
        };
        scroll.shown = shown;
        if let Some(span) = span {
            if span.min < scroll.offset {
                scroll.offset = span.min;
            } else if span.max > scroll.offset + viewport.width() {
                scroll.offset = span.max - viewport.width();
            }
            scroll.velocity = 0.0;
        }

        let clamped = scroll
            .offset
            .clamp(0.0, (content - viewport.width()).max(0.0));
        // an edge stops a coast
        if clamped != scroll.offset {
            scroll.velocity = 0.0;
        }
        scroll.offset = clamped;
        self.row_scrolls.insert(key, scroll);
        scroll.offset
    }

    /// How far `key` is scrolled along its rows.
    pub fn row_scrolled(&self, key: RowScroller) -> f32 {
        self.row_scrolls
            .get(&key)
            .map_or(0.0, |scroll| scroll.offset)
    }

    /// The chrome of `rect`, scrolled `scroll` of `content` and showing
    /// within `clip`: its rows fade out at an edge with more beyond it —
    /// painted over the text, once it is — each band `(top, bottom, color)`
    /// in its own color, and a thumb along the bottom border says there is
    /// more.
    pub fn row_scroll_chrome(
        &mut self, ui: &Ui, rect: Rect, clip: Rect, scroll: f32, content: f32,
        bands: &[(f32, f32, Color32)],
    ) {
        let (width, fade) = (rect.width(), EDGE_FADE.pts());
        let edges = [
            (scroll > 0.5, rect.left(), rect.left() + fade),
            (scroll + width < content - 0.5, rect.right() - fade, rect.right()),
        ];
        for (more, x0, x1) in edges {
            for (y0, y1, color) in bands.iter().filter(|_| more) {
                let fade = Rect::from_x_y_ranges(x0..=x1, *y0..=*y1);
                let (left, right) = if x0 == rect.left() {
                    (*color, Color32::TRANSPARENT)
                } else {
                    (Color32::TRANSPARENT, *color)
                };
                self.edge_fades.push((fade.intersect(clip), left, right));
            }
        }

        let track = rect.shrink2(Vec2::new(2.0, 1.0));
        let thumb = Vec2::new(track.width() * width / content, 3.0);
        let travel = (track.width() - thumb.x) * scroll / (content - width);
        let thumb = Rect::from_min_size(track.left_bottom() + Vec2::new(travel, -thumb.y), thumb);
        ui.painter()
            .rect_filled(thumb, 1.5, self.ctx.get_lb_theme().neutral());
    }

    /// The block wider than the note that shows at `pos`.
    pub fn row_scroller_at(&self, pos: Pos2) -> Option<RowScroller> {
        let mut scrollers = self.row_scrollers.iter();
        let found = scrollers.find(|(rect, _)| rect.contains(pos))?.1;
        self.row_scrolls.contains_key(&found).then_some(found)
    }
}

impl MdEdit {
    /// Gives a touch drag on the note's body to the block wider than the
    /// note it began on, if it sets out along the rows: the block pans and
    /// the page stays where it is. One that sets out down the rows is the
    /// page's, as anywhere. True while the drag is not the page's.
    pub fn pan_rows(&mut self, ui: &Ui, body: &egui::Response) -> bool {
        const SLOP: f32 = 6.0;
        let (origin, pos, delta) =
            ui.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos(), i.pointer.delta()));
        // a touch the platform cancels ends with no release: the pointer is
        // gone and the body is dragged still, through the next press
        let (pressed, gone) = ui.input(|i| (i.pointer.any_pressed(), !i.pointer.has_pointer()));
        if body.drag_started() || (pressed && body.dragged()) {
            let scroller = origin.and_then(|origin| self.renderer.row_scroller_at(origin));
            self.row_pan = scroller.map(|scroller| (scroller, None));
            if let Some(scroll) = scroller.and_then(|key| self.renderer.row_scrolls.get_mut(&key)) {
                (scroll.velocity, scroll.tracked) = (0.0, 0.0);
            }
        } else if gone || !body.dragged() || self.in_progress_block_drag.is_some() {
            // a finger lifted mid-motion leaves the block coasting
            if let Some((key, Some(true))) = self.row_pan.take() {
                if let Some(scroll) = self.renderer.row_scrolls.get_mut(&key) {
                    scroll.velocity = if gone { 0.0 } else { scroll.tracked };
                }
            }
            self.row_pan = None;
        }
        let Some((key, along)) = &mut self.row_pan else { return false };
        if along.is_none() {
            let moved = origin
                .zip(pos)
                .map_or(Vec2::ZERO, |(origin, pos)| pos - origin);
            if moved.length() < SLOP {
                return true;
            }
            *along = Some(moved.x.abs() > moved.y.abs());
        }
        let scroll = self.renderer.row_scrolls.get_mut(key);
        if let (Some(true), Some(scroll)) = (*along, scroll) {
            scroll.offset -= delta.x;
            let dt = ui.input(|i| i.stable_dt).max(0.001);
            scroll.tracked = 0.6 * scroll.tracked - 0.4 * delta.x / dt;
        }
        *along == Some(true)
    }
}
