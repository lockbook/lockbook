use std::mem;

use egui::epaint::text::cursor::Cursor;
use egui::text::CCursor;
use egui::{Color32, Pos2, Rangef, Rect, Sense, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::ExtendedInput as _;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::galleys::GalleyInfo;
use crate::tab::markdown_editor::widget::INLINE_PADDING;

use super::{Event, Location, Region};

#[derive(Debug, Default)]
pub struct CursorState {
    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,
}

impl Editor {
    /// Highlights the provided range with a faded version of the provided accent color.
    pub fn show_range(
        &self, ui: &mut Ui, highlight_range: (DocCharOffset, DocCharOffset), accent: Color32,
    ) {
        for rect in self.range_rects(highlight_range) {
            ui.painter()
                .rect_filled(rect, 2., accent.gamma_multiply(0.15));
        }
    }

    pub fn range_rects(&self, range: (DocCharOffset, DocCharOffset)) -> Vec<Rect> {
        let mut result = Vec::new();

        // todo: binary search
        for galley_info in self.galleys.galleys.iter().rev() {
            let GalleyInfo { range: galley_range, galley, mut rect, padded, .. } = galley_info;
            if galley_range.end() < range.start() {
                break;
            } else if galley_range.start() > range.end() {
                continue;
            }

            if *padded {
                rect = rect.expand2(INLINE_PADDING * Vec2::X);
            }

            if galley_range.contains_inclusive(range.start()) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (range.start() - galley_range.start()).0,
                    prefer_next_row: true,
                });
                rect.min.x = cursor_to_pos_abs(galley_info, cursor).x;
            }
            if galley_range.contains_inclusive(range.end()) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (range.end() - galley_range.start()).0,
                    prefer_next_row: true,
                });
                rect.max.x = cursor_to_pos_abs(galley_info, cursor).x;
            }

            result.push(rect);
        }

        result.reverse();
        result
    }

    /// Draws a cursor at the provided offset with the provided accent color.
    // todo: improve cursor rendering at the end of inline code segments and similar constructs
    pub fn show_offset(&self, ui: &mut Ui, offset: DocCharOffset, accent: Color32) {
        if let Some([top, bot]) = self.cursor_line(offset) {
            ui.painter().clone().vline(
                top.x,
                Rangef { min: top.y, max: bot.y },
                Stroke::new(1., accent),
            );
        }
    }

    pub fn show_selection_handles(&mut self, ui: &mut Ui) {
        let color = self.theme.fg().accent_secondary;
        let selection = self
            .in_progress_selection
            .unwrap_or(self.buffer.current.selection);
        let selection_start_line = self.cursor_line(selection.0);
        let selection_end_line = self.cursor_line(selection.1);

        let radius = 10.0;

        // draw selection handles
        // handles invisible but still draggable when selection is empty
        // we must allocate handles to check if they were dragged last frame
        if !self.buffer.current.selection.is_empty() {
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
            };
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
                    end: Location::DocCharOffset(self.buffer.current.selection.1),
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
            };
            let end_response = ui.allocate_rect(selection_end_handle_rect, Sense::drag());

            // adjust cursor based on selection handle drag
            if end_response.drag_stopped() {
                if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                    let region = Region::from(in_progress_selection);
                    ui.ctx().push_markdown_event(Event::Select { region });
                }
            } else if end_response.dragged() {
                let region = Region::BetweenLocations {
                    start: Location::DocCharOffset(self.buffer.current.selection.0),
                    end: Location::Pos(
                        ui.input(|i| i.pointer.interact_pos().unwrap_or_default() - 10. * Vec2::Y),
                    ),
                };
                self.in_progress_selection = Some(self.region_to_range(region));
            }
        }
    }

    pub fn scroll_to_cursor(&self, ui: &mut Ui) {
        let selection = self
            .in_progress_selection
            .unwrap_or(self.buffer.current.selection);

        if let Some([top, bot]) = self.cursor_line(selection.1) {
            let rect = Rect::from_min_max(top, bot);
            ui.scroll_to_rect(rect.expand(rect.height()), None);
        }
    }

    pub fn cursor_line(&self, offset: DocCharOffset) -> Option<[Pos2; 2]> {
        let (galley_idx, cursor) = self.galleys.galley_and_cursor_by_offset(offset)?;
        let galley = &self.galleys[galley_idx];
        let x = cursor_to_pos_abs(galley, cursor).x;
        let y_range = galley.rect.y_range();
        Some([Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }])
    }
}

/// returns the x coordinate of the absolute position of `cursor` in `galley`
pub fn x_impl(galley: &GalleyInfo, cursor: Cursor) -> f32 {
    cursor_to_pos_abs(galley, cursor).x
}

/// adjusts cursor so that its absolute x coordinate matches the target (if there is one)
pub fn from_x(x: f32, galley: &GalleyInfo, cursor: Cursor) -> Cursor {
    let mut pos_abs = cursor_to_pos_abs(galley, cursor);
    pos_abs.x = x;
    pos_abs_to_cursor(galley, pos_abs)
}

/// returns the absolute position of `cursor` in `galley`
pub fn cursor_to_pos_abs(galley: &GalleyInfo, cursor: Cursor) -> Pos2 {
    // experimentally, max.y gives us the y that will put us in the correct row
    galley.rect.min + galley.galley.pos_from_cursor(&cursor).max.to_vec2()
}

/// returns a cursor which has the absolute position `pos_abs` in `galley`
pub fn pos_abs_to_cursor(galley: &GalleyInfo, pos_abs: Pos2) -> Cursor {
    galley.galley.cursor_from_pos(pos_abs - galley.rect.min)
}
