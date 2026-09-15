use std::f32::consts::SQRT_2;
use std::mem;

use egui::{Color32, Pos2, Rangef, Rect, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::theme::palette_v2::ThemeExt as _;

use super::{Event, Region};

/// Material handle size in dp (not scaled with font). Touch target is padded.
const HANDLE_SIZE: f32 = 22.0;
const SELECTION_HANDLE_RADIUS: f32 = HANDLE_SIZE / 2.0;
/// Insertion teardrop hangs this far below the caret (`r + r√2`).
pub(in crate::tab::markdown_editor) const SELECTION_HANDLE_HEIGHT: f32 =
    SELECTION_HANDLE_RADIUS * (1.0 + SQRT_2);

#[derive(Debug, Default)]
pub struct CursorState {
    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,
}

impl MdEdit {
    /// Highlights the provided range with a faded version of the provided accent color.
    pub fn show_range(&self, ui: &mut Ui, highlight_range: (Grapheme, Grapheme), color: Color32) {
        self.renderer.show_range(ui, highlight_range, color)
    }

    pub fn range_rects(&self, range: (Grapheme, Grapheme)) -> Vec<Rect> {
        self.renderer.range_rects(range)
    }

    pub fn selection_tap(&self, pos: Pos2) -> bool {
        let selection = self.renderer.buffer.current.selection;
        if selection.is_empty() {
            // Menu-on-tap: the caret, and the insertion handle when it's
            // showing (the handle hangs below the line; without this a tap
            // on it would place the caret on the next row).
            const PAD_X: f32 = 6.0;
            const PAD_Y: f32 = 12.0;
            let on_caret = self
                .cursor_line(selection.0)
                .map(|[top, bot]| {
                    Rect::from_min_max(top, bot)
                        .expand2(Vec2::new(PAD_X, PAD_Y))
                        .contains(pos)
                })
                .unwrap_or(false);
            let on_handle = self.insertion_handle_visible
                && self
                    .cursor_line(selection.0)
                    .is_some_and(|line| self.insertion_handle_hit_rect(line).contains(pos));
            on_caret || on_handle
        } else {
            // Tapping the selection (padded to the 48dp handle target) menus.
            let pad_rect = |rect: Rect| {
                let pad_x = ((48.0 - rect.width()) / 2.0).max(0.0);
                let pad_y = ((48.0 - rect.height()) / 2.0).max(0.0);
                rect.expand2(Vec2::new(pad_x, pad_y))
            };
            self.range_rects(selection)
                .iter()
                .any(|&r| pad_rect(r).contains(pos))
        }
    }

    /// Draws a caret at the provided offset.
    // todo: improve cursor rendering at the end of inline code segments and similar constructs
    pub fn show_offset(
        &self, ui: &mut Ui, offset: Grapheme, accent: Color32, time_since_interact: Option<f64>,
    ) {
        if let Some([top, bot]) = self.cursor_line(offset) {
            let paint = |alpha| {
                crate::widgets::paint_caret(
                    ui,
                    top.x,
                    Rangef::new(top.y, bot.y),
                    accent.gamma_multiply(alpha),
                );
            };
            if let Some(dt) = time_since_interact {
                crate::widgets::with_blinking_caret(ui, dt, paint);
            } else {
                paint(1.0);
            }
        }
    }

    pub fn show_selection_handles(&mut self, ui: &mut Ui) {
        let theme = self.renderer.ctx.get_lb_theme();
        let color = theme.fg().get_color(theme.prefs().primary);
        let selection = self
            .in_progress_selection
            .unwrap_or(self.renderer.buffer.current.selection);
        let radius = SELECTION_HANDLE_RADIUS;

        if selection.is_empty() {
            // Insertion teardrop under a collapsed caret. Hidden while typing;
            // stays up during its own drag.
            let dragging_insertion = self.in_progress_selection.is_some_and(|s| s.is_empty());
            if !self.renderer.readonly && (self.insertion_handle_visible || dragging_insertion) {
                if let Some(line) = self.cursor_line(selection.0) {
                    self.paint_insertion_handle(ui, line, radius, color);
                    self.interact_insertion_handle(ui, line);
                }
            }
            return;
        }

        let selection_start_line = self.cursor_line(selection.0);
        let selection_end_line = self.cursor_line(selection.1);

        if let Some(line) = selection_start_line {
            self.paint_handle(ui, line, radius, color, true);
        }
        if let Some(line) = selection_end_line {
            self.paint_handle(ui, line, radius, color, false);
        }

        if let Some(line) = selection_start_line {
            self.interact_handle(ui, line, radius, true);
        }
        if let Some(line) = selection_end_line {
            self.interact_handle(ui, line, radius, false);
        }
    }

    pub(in crate::tab::markdown_editor) fn insertion_handle_hit_rect(
        &self, line: [Pos2; 2],
    ) -> Rect {
        let r = SELECTION_HANDLE_RADIUS;
        let tip = line[1];
        let center = Pos2 { x: tip.x, y: tip.y + r * SQRT_2 };
        Rect::from_min_max(Pos2::new(center.x - r, tip.y), Pos2::new(center.x + r, center.y + r))
            .expand(12.0)
    }

    /// Material collapsed handle: circle + square, rotated 45° so the corner
    /// points up at the caret.
    fn paint_insertion_handle(&self, ui: &Ui, line: [Pos2; 2], radius: f32, color: Color32) {
        let tip = line[1];
        let center = Pos2 { x: tip.x, y: tip.y + radius * SQRT_2 };
        ui.painter().circle_filled(center, radius, color);
        let diag = radius * (SQRT_2 / 2.0);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                tip,
                Pos2::new(center.x + diag, center.y - diag),
                center,
                Pos2::new(center.x - diag, center.y - diag),
            ],
            color,
            Stroke::NONE,
        ));
    }

    fn interact_insertion_handle(&mut self, ui: &mut Ui, line: [Pos2; 2]) {
        let hit_rect = self.insertion_handle_hit_rect(line);
        let id = ui.id().with("insertion_handle");
        let response = ui.interact(hit_rect, id, Sense::drag());

        if response.drag_stopped() {
            self.handle_drag_touch_offset = None;
            self.in_progress_handle = None;
            if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                let region = Region::from(in_progress_selection);
                ui.ctx().push_markdown_event(Event::Select { region });
            }
        } else if response.dragged() {
            let new_pos = self.handle_drag_query_pos(ui, line, response.drag_started());
            let dragged = self.pos_to_char_offset(new_pos);
            let displayed = self
                .in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection);
            self.in_progress_selection = Some((dragged, dragged));
            self.in_progress_handle = Some(dragged);
            if displayed != (dragged, dragged) {
                self.pending_scroll = Some(crate::tab::markdown_editor::ScrollTarget::Cursor);
            }
        }
    }

    fn paint_handle(&self, ui: &Ui, line: [Pos2; 2], radius: f32, color: Color32, is_start: bool) {
        let cursor_bot = line[1];
        let center = Pos2 {
            x: if is_start { cursor_bot.x - radius } else { cursor_bot.x + radius },
            y: cursor_bot.y + radius,
        };
        ui.painter().circle_filled(center, radius, color);
        let (rect_min_x, rect_max_x) =
            if is_start { (center.x, center.x + radius) } else { (center.x - radius, center.x) };
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect_min_x, center.y - radius),
                Pos2::new(rect_max_x, center.y),
            ),
            0.,
            color,
        );
    }

    fn interact_handle(&mut self, ui: &mut Ui, line: [Pos2; 2], radius: f32, is_start: bool) {
        let hit_pad = 12.0;
        let cursor_bot = line[1];
        let (min_x, max_x) = if is_start {
            (cursor_bot.x - 2. * radius, cursor_bot.x)
        } else {
            (cursor_bot.x, cursor_bot.x + 2. * radius)
        };
        let hit_rect = Rect::from_min_max(
            Pos2::new(min_x, cursor_bot.y),
            Pos2::new(max_x, cursor_bot.y + 2. * radius),
        )
        .expand(hit_pad);
        let id =
            ui.id()
                .with(if is_start { "selection_handle_start" } else { "selection_handle_end" });
        let response = ui.interact(hit_rect, id, Sense::drag());

        if response.drag_stopped() {
            self.handle_drag_touch_offset = None;
            self.in_progress_handle = None;
            if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                let region = Region::from(in_progress_selection);
                ui.ctx().push_markdown_event(Event::Select { region });
            }
        } else if response.dragged() {
            let new_pos = self.handle_drag_query_pos(ui, line, response.drag_started());
            // The fixed handle anchors at the committed selection (the buffer
            // selection doesn't change until drag release); handles paint at
            // raw .0/.1. Clamp the dragged handle so the two never cross and
            // the selection keeps ≥1 grapheme — matching Google Keep.
            let selection = self.renderer.buffer.current.selection;
            let dragged = self.pos_to_char_offset(new_pos);
            let moving =
                if is_start { dragged.min(selection.1 - 1) } else { dragged.max(selection.0 + 1) };
            let new_sel = if is_start { (moving, selection.1) } else { (selection.0, moving) };
            let displayed = self.in_progress_selection.unwrap_or(selection);
            self.in_progress_selection = Some(new_sel);
            self.in_progress_handle = Some(moving);
            if displayed != new_sel {
                self.pending_scroll = Some(crate::tab::markdown_editor::ScrollTarget::Cursor);
            }
        }
    }

    /// Map the finger to a caret query point using the grab-time offset.
    /// Querying at the caret midpoint (not the line-top boundary) and keeping
    /// the offset constant prevents adjacent lines of different height from
    /// feeding back into the next frame's mapping.
    fn handle_drag_query_pos(&mut self, ui: &Ui, line: [Pos2; 2], drag_started: bool) -> Pos2 {
        let finger = ui.input(|i| i.pointer.interact_pos().unwrap_or_default());
        if drag_started || self.handle_drag_touch_offset.is_none() {
            let caret = Pos2::new(line[0].x, (line[0].y + line[1].y) * 0.5);
            self.handle_drag_touch_offset = Some(caret - finger);
        }
        let mut new_pos = finger + self.handle_drag_touch_offset.unwrap_or(Vec2::ZERO);
        // stay within the last fragment's y-range so `pos_to_range`
        // uses x-aware placement instead of jumping to doc end
        if let Some(last) = self.renderer.fragments.last() {
            new_pos.y = new_pos.y.min(last.rect.max.y - 1.0);
        }
        new_pos
    }

    pub fn scroll_to_cursor(&mut self, canvas_rect: Rect) {
        use crate::tab::markdown_editor::build_target_reveal;
        use crate::tab::markdown_editor::scroll_content::DocScrollContent;
        use crate::widgets::affine_scroll::Align;

        // Make the moving end of the selection visible. Handle drag sets
        // `in_progress_handle`; otherwise the active end.
        // Passed as a zero-length range — `build_target_reveal` handles
        // single-point and multi-line ranges identically.
        let cursor = self.in_progress_handle.unwrap_or_else(|| {
            self.in_progress_selection
                .unwrap_or(self.renderer.buffer.current.selection)
                .1
        });

        // expand cursor rect by one row to scroll while drag selecting
        let pad = if self.in_progress_selection.is_some() {
            self.renderer.layout.row_height
        } else {
            self.renderer.layout.row_spacing / 2.0
        };

        let arena = comrak::Arena::new();
        let root = self.renderer.reparse(&arena);
        let content = DocScrollContent::for_frame(&self.renderer, root, canvas_rect.height());

        let Some(target_rect) = build_target_reveal(
            &self.renderer,
            &content,
            &self.scroll_area.state,
            (cursor, cursor),
            canvas_rect,
            pad,
        ) else {
            return;
        };
        self.scroll_area
            .reveal(&content, target_rect, Align::Nearest);
    }

    pub fn cursor_line(&self, offset: Grapheme) -> Option<[Pos2; 2]> {
        use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
        let frag = self.renderer.fragment_at_offset(offset)?;
        let x = self.renderer.fragment_x(frag, offset);
        let row_h = self.renderer.layout.row_height;
        // A caret bordering a collapsed embed (image or link card) spans the
        // embed's height — it reads as one big glyph.
        let border_image = self.renderer.fragments.iter().find(|f| {
            matches!(f.content, FragmentContent::Embed { .. })
                && (f.source_range.start() == offset || f.source_range.end() == offset)
        });
        let y_range = match (border_image, &frag.content) {
            (Some(image), _) => {
                egui::Rangef::new(image.rect.top(), image.rect.bottom() + row_h * 0.2)
            }
            // interior of an image
            (None, FragmentContent::Embed { .. }) => {
                let baseline = frag.rect.bottom();
                egui::Rangef::new(baseline - row_h * 0.8, baseline + row_h * 0.2)
            }
            (None, _) => frag.rect.y_range(),
        };
        let y_range = y_range.expand(self.renderer.layout.row_spacing / 2.);
        Some([Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }])
    }
}

impl MdRender {
    /// Highlights the provided range with a faded version of the provided accent color.
    pub fn show_range(&self, ui: &mut Ui, highlight_range: (Grapheme, Grapheme), color: Color32) {
        for rect in self.range_rects(highlight_range) {
            ui.painter().rect_filled(rect, 2., color);
        }
    }

    /// Rects covering `range` in the rendered layout — the geometry behind
    /// selection, find highlights, and diff tints. Read-only labels reach it
    /// on the renderer directly.
    pub fn range_rects(&self, range: (Grapheme, Grapheme)) -> Vec<Rect> {
        use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
        let mut result: Vec<Rect> = Vec::new();
        for frag in self.fragments.iter() {
            let frag_range = frag.source_range;
            // Empty-range fragments contribute no width — except a chip's pad
            // spacers, which carry the capsule's real side padding. Include
            // those when their capsule overlaps the selection so the highlight
            // spans the full pill (and the last rect — where iOS drops the end
            // handle — reaches the pill's edge).
            if frag_range.start() == frag_range.end() {
                let pad_scope = frag
                    .style_stack
                    .last()
                    .filter(|s| s.chip && matches!(frag.content, FragmentContent::Spacer))
                    .map(|s| s.source_range);
                let overlaps =
                    pad_scope.is_some_and(|s| s.start() < range.end() && range.start() < s.end());
                if !overlaps {
                    continue;
                }
            }
            // (pads passed their own scope-overlap check above; their empty
            // range would always fail this one)
            if frag_range.start() != frag_range.end()
                && (frag_range.end() <= range.start() || frag_range.start() >= range.end())
            {
                continue;
            }
            let mut rect = frag.rect;
            // Recompute an edge only when the selection endpoint falls
            // *strictly inside* the fragment. At `==` the fragment's own edge
            // already is the endpoint — and for capsule fragments, which all
            // share the atom's full source range, recomputing both edges from
            // global offsets would mangle every segment's rect.
            if frag_range.start() < range.start() {
                rect.min.x = self.fragment_x(frag, range.start());
            }
            if frag_range.end() > range.end() {
                rect.max.x = self.fragment_x(frag, range.end());
            }
            if rect.area() <= 0.001 {
                continue;
            }
            result.push(rect);
        }

        // Selected newlines / blank lines have no glyph to highlight. When
        // the selection crosses a source line's end, add a fixed-width slab
        // at that row's end so the captured `\n` reads as selected.
        // (Soft-wrap whitespace shares its boundary offset with the next
        // row's start, so it isn't covered here.)
        if !range.is_empty() {
            let slab_w = self.layout.row_height * 0.4;
            let line_count = self.bounds.source_lines.len();
            for i in 0..line_count.saturating_sub(1) {
                // The `\n` grapheme sits at the line's end offset (source
                // lines exclude their trailing newline).
                let newline = self.bounds.source_lines[i].end();
                if newline < range.start() || newline >= range.end() {
                    continue;
                }
                // Match the row's content rects (bare fragment rect), not
                // `cursor_line`'s caret-height-expanded range.
                if let Some(frag) = self.fragment_at_offset(newline) {
                    let x = self.fragment_x(frag, newline);
                    let (top, bot) = (frag.rect.min.y, frag.rect.max.y);
                    // Extend the row's content rect rightward into the slab so
                    // the newline flows out of the row's highlight as one
                    // rounded shape, rather than a separate notched rect.
                    if let Some(r) = result.iter_mut().find(|r| {
                        (r.top() - top).abs() < 0.001
                            && (r.bottom() - bot).abs() < 0.001
                            && (r.right() - x).abs() < 0.5
                    }) {
                        r.max.x = r.max.x.max(x + slab_w);
                    } else {
                        result.push(Rect::from_min_max(
                            Pos2::new(x, top),
                            Pos2::new(x + slab_w, bot),
                        ));
                    }
                }
            }
        }

        // Reading order: group vertically-overlapping rects into rows, then by
        // x — so the last rect is the selection's geometric end, where iOS reads
        // the end handle. A raw top-sort would put a low inter-image space rect
        // last, dropping the handle on the next image's left edge.
        let mut by_top: Vec<usize> = (0..result.len()).collect();
        by_top.sort_by(|&a, &b| result[a].top().total_cmp(&result[b].top()));
        let mut row_of = vec![0usize; result.len()];
        let mut row = 0;
        let mut row_bottom = f32::NEG_INFINITY;
        for (i, &k) in by_top.iter().enumerate() {
            if i != 0 && result[k].top() > row_bottom + 0.5 {
                row += 1;
                row_bottom = result[k].bottom();
            } else {
                row_bottom = row_bottom.max(result[k].bottom());
            }
            row_of[k] = row;
        }
        let mut ordered: Vec<(usize, Rect)> = result
            .iter()
            .enumerate()
            .map(|(i, r)| (row_of[i], *r))
            .collect();
        ordered.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.left().total_cmp(&b.1.left())));

        // Coalesce contiguous same-row rects (post-sort — the fragment list
        // interleaves rows) so each row's selection paints as one merged
        // rounded rect: outer corners rounded, no seams at fragment edges.
        let mut merged: Vec<Rect> = Vec::new();
        for (_, rect) in ordered {
            if let Some(last) = merged.last_mut() {
                let same_row = (last.top() - rect.top()).abs() < 0.001
                    && (last.bottom() - rect.bottom()).abs() < 0.001;
                let contiguous = (last.right() - rect.left()).abs() < 0.001;
                if same_row && contiguous {
                    last.max.x = last.max.x.max(rect.max.x);
                    continue;
                }
            }
            merged.push(rect);
        }
        merged
    }
}
