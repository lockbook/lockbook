use egui::epaint::text::cursor::Cursor;
use egui::{text::CCursor, Pos2};
use egui::{Stroke, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::tab::markdown_editor::widget::INLINE_PADDING;
use crate::tab::markdown_editor::{galleys::GalleyInfo, Editor};

#[derive(Debug, Default)]
pub struct CursorState {
    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,
}

impl Editor {
    pub fn show_selection(&mut self, ui: &mut egui::Ui) {
        let selection = self.buffer.current.selection;
        let cursor = selection.1;
        let mut cursor_drawn = false;

        // todo: binary search
        for galley_info in self.galleys.galleys.iter().rev() {
            let GalleyInfo { range, galley, mut rect, padded } = galley_info;
            if range.end() < selection.start() {
                break;
            } else if range.start() > selection.end() {
                continue;
            }

            if *padded {
                rect = rect.expand2(INLINE_PADDING * Vec2::X);
            }

            if range.contains(selection.start(), true, true) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (selection.start() - range.start()).0,
                    prefer_next_row: true,
                });
                rect.min.x = cursor_to_pos_abs(galley_info, cursor).x;
            }
            if range.contains(selection.end(), true, true) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (selection.end() - range.start()).0,
                    prefer_next_row: true,
                });
                rect.max.x = cursor_to_pos_abs(galley_info, cursor).x;
            }

            ui.painter().rect_filled(
                rect,
                2.,
                self.theme.fg().accent_secondary.gamma_multiply(0.15),
            );

            // draw cursor
            // we need to only draw the cursor for the later galley (prefer next row)
            // todo: improve cursor rendering at the end of inline code segments and similar constructs
            // todo: factor to use cursor_line (also used by iOS FFI)
            if !cursor_drawn && range.contains(cursor, true, true) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (selection.1 - range.start()).0,
                    prefer_next_row: true,
                });
                let x = cursor_to_pos_abs(galley_info, cursor).x;
                let y_range = rect.y_range();
                ui.painter().clone().vline(
                    x,
                    y_range,
                    Stroke::new(1., self.theme.fg().accent_secondary),
                );

                cursor_drawn = true;
            }
        }
    }

    // todo: improve cursor rendering at the end of inline code segments and similar constructs
    pub fn cursor_line(&self, offset: DocCharOffset) -> [Pos2; 2] {
        for galley_info in self.galleys.galleys.iter().rev() {
            let GalleyInfo { range, galley, rect, .. } = galley_info;
            if range.contains(offset, true, true) {
                let cursor = galley.from_ccursor(CCursor {
                    index: (offset - range.start()).0,
                    prefer_next_row: true,
                });
                let x = cursor_to_pos_abs(galley_info, cursor).x;
                let y_range = rect.y_range();
                return [Pos2 { x, y: y_range.min }, Pos2 { x, y: y_range.max }];
            }
        }

        // todo: better error handling
        Default::default()
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
