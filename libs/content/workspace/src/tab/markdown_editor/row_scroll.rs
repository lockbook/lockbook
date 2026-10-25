//! Blocks wider than the note — tables and code blocks — scroll along their
//! rows: a wheel over one, a touch drag that sets out sideways, or the
//! platform's own pan recognizer moves it, and it coasts after a fling. It
//! follows what the user is shown: a find or preview match, or the end of a
//! selection the user moved. An edge with more beyond it fades out.

use egui::{Color32, Pos2, Rangef, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::style::{EDGE_FADE, overlay_scroll};
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// A block that scrolls along its rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowScroller {
    /// A table, by its place among the tables.
    Table(usize),
    /// A block drawn as code, by its place among them.
    Code(usize),
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
    /// Only the program moved it this frame — to show the caret or a match,
    /// or back inside its bounds — which leaves its bar hidden.
    pub quiet: bool,
}

impl MdRender {
    /// Forgets what the last frame painted of scrolled blocks and the
    /// chrome over them.
    pub fn clear_row_scroll_frame(&mut self) {
        self.row_scrollers.clear();
        self.code_line_clips.clear();
        self.edge_fades.clear();
        self.copy_buttons.clear();
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
        let users = scroll.offset;
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
        scroll.quiet = scroll.offset != users;
        self.row_scrolls.insert(key, scroll);
        scroll.offset
    }

    /// How far `key` is scrolled along its rows.
    pub fn row_scrolled(&self, key: RowScroller) -> f32 {
        self.row_scrolls
            .get(&key)
            .map_or(0.0, |scroll| scroll.offset)
    }

    /// The chrome of `key`, `content` wide and showing in `rect` within
    /// `clip`: its rows fade out at an edge with more beyond it — painted
    /// over the text, once it is — each band `(top, bottom, color)` in its
    /// own color, and the design system's bar runs along the bottom while
    /// the user scrolls.
    pub fn row_scroll_chrome(
        &mut self, ui: &mut Ui, key: RowScroller, rect: Rect, clip: Rect, content: f32,
        bands: &[(f32, f32, Color32)],
    ) {
        let (width, fade) = (rect.width(), EDGE_FADE.pts());
        let scroll = self.row_scrolled(key);
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

        if self.painting_drag_float {
            return;
        }
        let Some(row_scroll) = self.row_scrolls.get_mut(&key) else { return };
        let id = ui.scope_id().with(("row_scroll", key));
        let mut offset = row_scroll.offset;
        overlay_scroll::scroller_bar_x(ui, id, rect, content, &mut offset, row_scroll.quiet);
        if offset != row_scroll.offset {
            (row_scroll.offset, row_scroll.velocity) = (offset, 0.0);
        }
    }

    /// The block wider than the note that shows at `pos`.
    pub fn row_scroller_at(&self, pos: Pos2) -> Option<RowScroller> {
        let mut scrollers = self.row_scrollers.iter();
        let found = scrollers.find(|(rect, _)| rect.contains(pos))?.1;
        self.row_scrolls.contains_key(&found).then_some(found)
    }

    /// What shows of `rect`, painted for `source`: a block too wide for the
    /// note hides the rest. `None` if nothing does. A rect off every such
    /// block is as it was.
    pub fn row_showing(&self, source: (Grapheme, Grapheme), rect: Rect) -> Option<Rect> {
        let y = rect.center().y;
        let code_line = self.code_line_clips.iter().find(|(clip, line)| {
            clip.y_range().contains(y) && line.contains_range(&source, true, true)
        });
        let Some((clip, _)) = code_line else { return self.table_showing(rect) };
        let x = rect.x_range().intersection(clip.x_range());
        (x.min <= x.max).then(|| Rect::from_x_y_ranges(x, rect.y_range()))
    }

    /// Whether a scrolled block hides where the caret at `offset` is: it
    /// isn't drawn then, until a move or an edit scrolls back to it.
    pub fn caret_scrolled_out(&self, offset: Grapheme) -> bool {
        let Some(frag) = self.fragment_at_offset(offset) else { return false };
        let x = self.fragment_x(frag, offset);
        let caret = Rect::from_x_y_ranges(x..=x, frag.rect.y_range());
        self.row_showing(frag.source_range, caret).is_none()
    }

    /// `rect`, painted for `source`, as far as a pointer can reach it: a
    /// code block too wide for the note squeezes what it hides to the edge
    /// it is hidden past.
    pub fn row_reachable(&self, source: (Grapheme, Grapheme), rect: Rect) -> Rect {
        let y = rect.center().y;
        let code_line = self.code_line_clips.iter().find(|(clip, line)| {
            clip.y_range().contains(y) && line.contains_range(&source, true, true)
        });
        let Some((clip, _)) = code_line else { return rect };
        let x = |x: f32| x.clamp(clip.left(), clip.right());
        Rect::from_x_y_ranges(x(rect.left())..=x(rect.right()), rect.y_range())
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
