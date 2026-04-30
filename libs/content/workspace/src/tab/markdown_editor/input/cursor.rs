use std::mem;

use egui::{Color32, Pos2, Rangef, Rect, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdEdit;

use crate::tab::{ExtendedInput as _, markdown_editor::galleys::GalleyInfo};
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
        let mut result = Vec::new();

        // todo: binary search
        for galley_info in self.renderer.galleys.galleys.iter().rev() {
            let GalleyInfo { range: galley_range, mut rect, padded, .. } = galley_info;
            if galley_range.end() < range.start() {
                break;
            } else if galley_range.start() > range.end() {
                continue;
            }

            if galley_range.contains_inclusive(range.start()) {
                rect.min.x = self.renderer.galley_x(galley_info, range.start());
            }
            if galley_range.contains_inclusive(range.end()) {
                rect.max.x = self.renderer.galley_x(galley_info, range.end());
            }

            if rect.area() > 0.001 && *padded {
                rect = rect.expand2(self.renderer.layout.inline_padding * Vec2::X);
            }

            result.push(rect);
        }

        result.reverse();
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
        let hit_pad = 12.0;

        if !self.renderer.buffer.current.selection.is_empty() {
            if let Some(selection_start_line) = selection_start_line {
                let selection_start_center = Pos2 {
                    x: selection_start_line[1].x - radius,
                    y: selection_start_line[1].y + radius,
                };
                ui.painter()
                    .circle_filled(selection_start_center, radius, color);
                ui.painter().rect_filled(
                    Rect {
                        min: Pos2 {
                            x: selection_start_center.x,
                            y: selection_start_center.y - radius,
                        },
                        max: Pos2 {
                            x: selection_start_center.x + radius,
                            y: selection_start_center.y,
                        },
                    },
                    0.,
                    color,
                );
            }

            if let Some(selection_end_line) = selection_end_line {
                let selection_end_center = Pos2 {
                    x: selection_end_line[1].x + radius,
                    y: selection_end_line[1].y + radius,
                };
                ui.painter()
                    .circle_filled(selection_end_center, radius, color);
                ui.painter().rect_filled(
                    Rect {
                        min: Pos2 {
                            x: selection_end_center.x - radius,
                            y: selection_end_center.y - radius,
                        },
                        max: Pos2 { x: selection_end_center.x, y: selection_end_center.y },
                    },
                    0.,
                    color,
                );
            }
        }

        if let Some(selection_start_line) = selection_start_line {
            // allocate rects to capture selection handle drag
            let selection_start_handle_rect = Rect {
                min: Pos2 {
                    x: selection_start_line[1].x - 2. * radius,
                    y: selection_start_line[1].y,
                },
                max: Pos2 {
                    x: selection_start_line[1].x,
                    y: selection_start_line[1].y + 2. * radius,
                },
            }
            .expand(hit_pad);
            let start_response = ui.allocate_rect(selection_start_handle_rect, Sense::drag());

            // adjust cursor based on selection handle drag
            if start_response.drag_stopped() {
                if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                    let region = Region::from(in_progress_selection);
                    ui.ctx().push_markdown_event(Event::Select { region });
                }
            } else if start_response.dragged() {
                let region = Region::BetweenLocations {
                    start: Location::Pos(
                        ui.input(|i| i.pointer.interact_pos().unwrap_or_default() - 10. * Vec2::Y),
                    ),
                    end: Location::Grapheme(self.renderer.buffer.current.selection.1),
                };
                self.in_progress_selection = Some(self.region_to_range(region));
            }
        }
        if let Some(selection_end_line) = selection_end_line {
            // allocate rects to capture selection handle drag
            let selection_end_handle_rect = Rect {
                min: Pos2 { x: selection_end_line[1].x, y: selection_end_line[1].y },
                max: Pos2 {
                    x: selection_end_line[1].x + 2. * radius,
                    y: selection_end_line[1].y + 2. * radius,
                },
            }
            .expand(hit_pad);
            let end_response = ui.allocate_rect(selection_end_handle_rect, Sense::drag());

            // adjust cursor based on selection handle drag
            if end_response.drag_stopped() {
                if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                    let region = Region::from(in_progress_selection);
                    ui.ctx().push_markdown_event(Event::Select { region });
                }
            } else if end_response.dragged() {
                let region = Region::BetweenLocations {
                    start: Location::Grapheme(self.renderer.buffer.current.selection.0),
                    end: Location::Pos(
                        ui.input(|i| i.pointer.interact_pos().unwrap_or_default() - 10. * Vec2::Y),
                    ),
                };
                self.in_progress_selection = Some(self.region_to_range(region));
            }
        }
    }

    pub fn scroll_to_cursor(&mut self, ui: &mut Ui, scroll_id: egui::Id, viewport_height: f32) {
        let target = self
            .in_progress_selection
            .unwrap_or(self.renderer.buffer.current.selection)
            .1;

        let arena = comrak::Arena::new();
        let root = self.renderer.reparse(&arena);
        let mut content = crate::tab::markdown_editor::scroll_content::DocScrollContent::new(
            &mut self.renderer,
            root,
            0.0,
            viewport_height / 2.0,
        )
        .with_default_leading();

        let scroll = crate::widgets::affine_scroll::AffineScrollArea::new(scroll_id);
        let current_offset = scroll.offset(ui.ctx());

        // Inclusive on `end` so an end-of-line cursor matches its row.
        let offset = crate::widgets::affine_scroll::make_visible_offset(
            &mut content,
            viewport_height,
            current_offset,
            |c| {
                c.text_range()
                    .is_some_and(|(start, end)| target >= start && target <= end)
            },
        );

        if let Some(o) = offset {
            scroll.set_offset(ui.ctx(), o);
        }
    }

    pub fn cursor_line(&self, offset: Grapheme) -> Option<[Pos2; 2]> {
        let galley_idx = self.renderer.galleys.galley_at_offset(offset)?;
        let galley = &self.renderer.galleys[galley_idx];
        let x = self.renderer.galley_x(galley, offset);
        let y_range = galley
            .rect
            .y_range()
            .expand(self.renderer.layout.row_spacing / 2.);
        Some([Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }])
    }
}
