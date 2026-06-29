use std::mem;

use egui::{Color32, Pos2, Rangef, Rect, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::MdEdit;
use crate::theme::palette_v2::ThemeExt as _;

use super::{Event, Location, Region};

const SELECTION_HANDLE_RADIUS: f32 = 12.0;
pub(in crate::tab::markdown_editor) const SELECTION_HANDLE_HEIGHT: f32 =
    SELECTION_HANDLE_RADIUS * 2.0;

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
        if selection.is_empty() {
            // A collapsed cursor has no handle, so size the menu-on-tap zone
            // to the caret: tap the caret to menu, tap a neighboring glyph to
            // reposition.
            const PAD_X: f32 = 6.0;
            const PAD_Y: f32 = 12.0;
            self.cursor_line(selection.0)
                .map(|[top, bot]| {
                    Rect::from_min_max(top, bot)
                        .expand2(Vec2::new(PAD_X, PAD_Y))
                        .contains(pos)
                })
                .unwrap_or(false)
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

        // Selected newlines / blank lines have no glyph to highlight. When
        // the selection crosses a source line's end, add a fixed-width slab
        // at that row's end so the captured `\n` reads as selected.
        // (Soft-wrap whitespace shares its boundary offset with the next
        // row's start, so it isn't covered here.)
        if !range.is_empty() {
            let slab_w = self.renderer.layout.row_height * 0.4;
            let line_count = self.renderer.bounds.source_lines.len();
            for i in 0..line_count.saturating_sub(1) {
                // The `\n` grapheme sits at the line's end offset (source
                // lines exclude their trailing newline).
                let newline = self.renderer.bounds.source_lines[i].end();
                if newline < range.start() || newline >= range.end() {
                    continue;
                }
                // Match the row's content rects (bare fragment rect), not
                // `cursor_line`'s caret-height-expanded range.
                if let Some(frag) = self.renderer.fragment_at_offset(newline) {
                    let x = self.renderer.fragment_x(frag, newline);
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

        result.sort_by(|a, b| {
            a.top()
                .total_cmp(&b.top())
                .then(a.left().total_cmp(&b.left()))
        });

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

        let radius = SELECTION_HANDLE_RADIUS;

        // Handles are interactive only where they're drawn — a non-empty
        // selection. A collapsed cursor has no handle, so it gets no drag
        // target (else an invisible zone around the caret eats nearby taps).
        if !self.renderer.buffer.current.selection.is_empty() {
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
            if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
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
        use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
        let frag = self.renderer.fragment_at_offset(offset)?;
        let x = self.renderer.fragment_x(frag, offset);
        // Image fragments span the image band; `rect.bottom()` is the
        // row's text baseline. Caret stays text-height around it.
        let y_range = match &frag.content {
            FragmentContent::Image { .. } => {
                let baseline = frag.rect.bottom();
                let row_h = self.renderer.layout.row_height;
                egui::Rangef::new(baseline - row_h * 0.8, baseline + row_h * 0.2)
            }
            _ => frag.rect.y_range(),
        };
        let y_range = y_range.expand(self.renderer.layout.row_spacing / 2.);
        Some([Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }])
    }
}
