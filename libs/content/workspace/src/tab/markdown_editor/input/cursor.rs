use std::mem;

use egui::{Color32, Pos2, Rangef, Rect, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::MdEdit;
use crate::theme::palette_v2::ThemeExt as _;

use super::{Event, Location, Region};

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
        for rect in self.range_rects(highlight_range) {
            ui.painter().rect_filled(rect, 2., color);
        }
    }

    pub fn selection_tap(&self, pos: Pos2) -> bool {
        let selection = self.renderer.buffer.current.selection;
        let pad_rect = |rect: Rect| {
            let pad_x = ((48.0 - rect.width()) / 2.0).max(0.0);
            let pad_y = ((48.0 - rect.height()) / 2.0).max(0.0);
            rect.expand2(Vec2::new(pad_x, pad_y))
        };
        if selection.is_empty() {
            self.cursor_line(selection.0)
                .map(|[top, bot]| pad_rect(Rect::from_min_max(top, bot)).contains(pos))
                .unwrap_or(false)
        } else {
            self.range_rects(selection)
                .iter()
                .any(|&r| pad_rect(r).contains(pos))
        }
    }

    pub fn range_rects(&self, range: (Grapheme, Grapheme)) -> Vec<Rect> {
        let mut result: Vec<Rect> = Vec::new();
        for frag in self.renderer.fragments.iter() {
            let frag_range = frag.source_range;
            // Skip empty-range fragments (anchors) — they contribute
            // no width, so a selection rect over them would be 0×N
            // and add noise.
            if frag_range.start() == frag_range.end() {
                continue;
            }
            if frag_range.end() <= range.start() || frag_range.start() >= range.end() {
                continue;
            }
            let mut rect = frag.rect;
            if frag_range.contains_inclusive(range.start()) {
                rect.min.x = self.renderer.fragment_x(frag, range.start());
            }
            if frag_range.contains_inclusive(range.end()) {
                rect.max.x = self.renderer.fragment_x(frag, range.end());
            }
            if rect.area() <= 0.001 {
                continue;
            }
            // Coalesce contiguous same-row rects so each row's
            // selection paints as one merged rounded rect (outer
            // corners rounded, no rounding at internal fragment
            // seams).
            if let Some(last) = result.last_mut() {
                let same_row = (last.top() - rect.top()).abs() < 0.001
                    && (last.bottom() - rect.bottom()).abs() < 0.001;
                let contiguous = (last.right() - rect.left()).abs() < 0.001;
                if same_row && contiguous {
                    last.max.x = rect.max.x;
                    continue;
                }
            }
            result.push(rect);
        }
        result
    }

    /// Draws a cursor at the provided offset with the provided accent color.
    // todo: improve cursor rendering at the end of inline code segments and similar constructs
    pub fn show_offset(&self, ui: &mut Ui, offset: Grapheme, accent: Color32) {
        if let Some([top, bot]) = self.cursor_line(offset) {
            ui.painter().clone().vline(
                top.x,
                Rangef { min: top.y, max: bot.y },
                Stroke::new(1., accent),
            );
        }
    }

    pub fn show_selection_handles(&mut self, ui: &mut Ui) {
        let theme = self.renderer.ctx.get_lb_theme();
        let color = theme.fg().get_color(theme.prefs().primary);
        let selection = self
            .in_progress_selection
            .unwrap_or(self.renderer.buffer.current.selection);
        let selection_start_line = self.cursor_line(selection.0);
        let selection_end_line = self.cursor_line(selection.1);

        let radius = 12.0;

        if !self.renderer.buffer.current.selection.is_empty() {
            if let Some(line) = selection_start_line {
                self.paint_handle(ui, line, radius, color, true);
            }
            if let Some(line) = selection_end_line {
                self.paint_handle(ui, line, radius, color, false);
            }
        }

        if let Some(line) = selection_start_line {
            self.interact_handle(ui, line, radius, true);
        }
        if let Some(line) = selection_end_line {
            self.interact_handle(ui, line, radius, false);
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

        if response.clicked() || response.drag_started() || response.dragged() || response.drag_stopped() {
            tracing::info!(
                target: "lb::md::sel",
                is_start,
                clicked = response.clicked(),
                drag_started = response.drag_started(),
                dragged = response.dragged(),
                drag_stopped = response.drag_stopped(),
                hit_rect = ?hit_rect,
                in_progress = ?self.in_progress_selection,
                selection = ?self.renderer.buffer.current.selection,
                "handle interact",
            );
        }

        if response.drag_stopped() {
            if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                tracing::info!(target: "lb::md::sel", taken = ?in_progress_selection, is_start, "handle drag_stopped -> consume");
                let region = Region::from(in_progress_selection);
                ui.ctx().push_markdown_event(Event::Select { region });
            }
        } else if response.dragged() {
            let line_height = line[1].y - line[0].y;
            let offset = Vec2::new(0.0, -line_height - radius);
            let mut new_pos = ui.input(|i| i.pointer.interact_pos().unwrap_or_default()) + offset;
            // stay within the last fragment's y-range so `pos_to_range`
            // uses x-aware placement instead of jumping to doc end
            if let Some(last) = self.renderer.fragments.last() {
                new_pos.y = new_pos.y.min(last.rect.max.y - 1.0);
            }
            let selection = self.renderer.buffer.current.selection;
            let region = if is_start {
                Region::BetweenLocations {
                    start: Location::Pos(new_pos),
                    end: Location::Grapheme(selection.1),
                }
            } else {
                Region::BetweenLocations {
                    start: Location::Grapheme(selection.0),
                    end: Location::Pos(new_pos),
                }
            };
            self.in_progress_selection = Some(self.region_to_range(region));
            tracing::info!(target: "lb::md::sel", is_start, "handle dragged -> set in_progress_selection {:?}", self.in_progress_selection);
            self.pending_scroll = Some(crate::tab::markdown_editor::ScrollTarget::Cursor);
        }
    }

    pub fn scroll_to_cursor(&mut self, canvas_rect: Rect) {
        use crate::tab::markdown_editor::build_target_reveal;
        use crate::tab::markdown_editor::scroll_content::DocScrollContent;
        use crate::widgets::affine_scroll::Align;

        // Make the moving end of the selection visible. Passed as a
        // zero-length range — `build_target_reveal` handles single-point
        // and multi-line ranges identically.
        let cursor = self
            .in_progress_selection
            .unwrap_or(self.renderer.buffer.current.selection)
            .1;

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
        let frag = self.renderer.fragment_at_offset(offset)?;
        let x = self.renderer.fragment_x(frag, offset);
        let y_range = frag
            .rect
            .y_range()
            .expand(self.renderer.layout.row_spacing / 2.);
        Some([Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }])
    }
}
