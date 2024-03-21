use bezier_rs::Subpath;
use glam::DVec2;

use crate::tab::svg_editor::{history::ManipulatorGroupId, util::pointer_interests_path};

use super::{SelectionOperation, SelectionResponse};

// todo: consider making this value dynamic depending on the scale of the element
pub const SCALE_BRUSH_SIZE: f64 = 10.0;

#[derive(Default)]
pub struct SelectionRect {
    left: Option<Subpath<ManipulatorGroupId>>,
    right: Option<Subpath<ManipulatorGroupId>>,
    top: Option<Subpath<ManipulatorGroupId>>,
    bottom: Option<Subpath<ManipulatorGroupId>>,
}

impl SelectionRect {
    pub fn new(path: &Subpath<ManipulatorGroupId>, working_rect: egui::Rect) -> Option<Self> {
        let bb = match path.bounding_box() {
            Some(b) => b,
            None => {
                return None;
            }
        };

        let mut clipped_bb = bb;
        clipped_bb[0].x = clipped_bb[0].x.max(working_rect.left() as f64);
        clipped_bb[0].y = clipped_bb[0].y.max(working_rect.top() as f64);

        clipped_bb[1].x = clipped_bb[1].x.min(working_rect.right() as f64);
        clipped_bb[1].y = clipped_bb[1].y.min(working_rect.bottom() as f64);

        let is_clipped_bb_outside_of_working_rect =
            clipped_bb[0].x > clipped_bb[1].x || clipped_bb[0].y > clipped_bb[1].y;

        if is_clipped_bb_outside_of_working_rect {
            return None;
        }

        let mut selection_rect = SelectionRect {
            left: Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[1].y },
                ],
                false,
            )),
            right: Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[1].y },
                ],
                false,
            )),
            top: Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[0].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[0].y },
                ],
                false,
            )),
            bottom: Some(Subpath::from_anchors(
                [
                    DVec2 { x: clipped_bb[0].x, y: clipped_bb[1].y },
                    DVec2 { x: clipped_bb[1].x, y: clipped_bb[1].y },
                ],
                false,
            )),
        };

        // when a bb is clipped, don't show the edge that's being clipeed
        if clipped_bb[1].y != bb[1].y {
            selection_rect.bottom = None;
        }
        if clipped_bb[0].y != bb[0].y {
            selection_rect.top = None;
        }
        if clipped_bb[1].x != bb[1].x {
            selection_rect.right = None;
        }
        if clipped_bb[0].x != bb[0].x {
            selection_rect.left = None;
        }

        Some(selection_rect)
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        if let Some(top_path) = &self.top {
            self.show_subpath(top_path, ui);
        };
        if let Some(bottom_path) = &self.bottom {
            self.show_subpath(bottom_path, ui);
        };

        if let Some(left_path) = &self.left {
            self.show_subpath(left_path, ui);

            if self.top.is_some() {
                let corner = left_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui);
            }
            if self.bottom.is_some() {
                let corner = left_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui);
            }
        };
        if let Some(right_path) = &self.right {
            self.show_subpath(right_path, ui);

            if self.top.is_some() {
                let corner = right_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui);
            }
            if self.bottom.is_some() {
                let corner = right_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui);
            }
        };
    }

    fn show_subpath(&self, path: &Subpath<ManipulatorGroupId>, ui: &mut egui::Ui) {
        let line_segment = path.get_segment(0).unwrap();
        let line_segment = [
            egui::pos2(line_segment.start().x as f32, line_segment.start().y as f32),
            egui::pos2(line_segment.end().x as f32, line_segment.end().y as f32),
        ];
        ui.painter().line_segment(
            line_segment,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        );
    }

    fn show_corner(&self, corner: DVec2, ui: &mut egui::Ui) {
        let handle_side_length = 8.0; // handle is a square
        let corner = egui::pos2(corner.x as f32, corner.y as f32);
        let rect = egui::Rect {
            min: egui::pos2(
                corner.x - handle_side_length / 2.0,
                corner.y - handle_side_length / 2.0,
            ),
            max: egui::pos2(
                corner.x + handle_side_length / 2.0,
                corner.y + handle_side_length / 2.0,
            ),
        };
        ui.painter().rect(
            rect,
            egui::Rounding::none(),
            egui::Color32::WHITE,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        )
    }

    pub fn get_cursor_icon(
        &self, path: &Subpath<ManipulatorGroupId>, cursor_pos: egui::Pos2,
    ) -> Option<SelectionResponse> {
        let bb = match path.bounding_box() {
            Some(b) => b,
            None => {
                return None;
            }
        };
        let rect = egui::Rect {
            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
        };

        if rect.contains(cursor_pos) {
            return Some(SelectionResponse::new(SelectionOperation::Translation));
        }

        if let Some(left_path) = &self.left {
            if pointer_interests_path(left_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::WestScale));
            }
        };
        if let Some(right_path) = &self.right {
            if pointer_interests_path(right_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::EastScale));
            }
        };

        if let Some(top_path) = &self.top {
            if pointer_interests_path(top_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::NorthScale));
            }
        };
        if let Some(bottom_path) = &self.bottom {
            if pointer_interests_path(bottom_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::SouthScale));
            }
        };

        None
    }
}
