use bezier_rs::Subpath;
use glam::DVec2;

use super::{SelectedElement, SelectionOperation, SelectionResponse};
use crate::{
    tab::svg_editor::{
        history::ManipulatorGroupId,
        util::{bb_to_rect, deserialize_transform, pointer_interests_path},
        Buffer,
    },
    theme::icons::Icon,
};

// todo: consider making this value dynamic depending on the scale of the element
pub const SCALE_BRUSH_SIZE: f64 = 10.0;

pub struct SelectionRectContainer {
    pub container: SelectionRect,
    children: Vec<SelectionRect>,
}
impl SelectionRectContainer {
    pub fn new(
        els: &[SelectedElement], working_rect: egui::Rect, buffer: &mut Buffer,
    ) -> Option<Self> {
        let mut container_bb = [DVec2::new(f64::MAX, f64::MAX), DVec2::new(f64::MIN, f64::MIN)];
        let mut children = vec![];
        for el in els.iter() {
            let bb = match buffer.paths.get(&el.id) {
                Some(path) => path.bounding_box().unwrap_or_default(),
                None => continue,
            };

            if let Some(clipped_rect) = SelectionRect::new(bb, working_rect) {
                children.push(clipped_rect);
            }

            container_bb[0].x = container_bb[0].x.min(bb[0].x);
            container_bb[0].y = container_bb[0].y.min(bb[0].y);

            container_bb[1].x = container_bb[1].x.max(bb[1].x);
            container_bb[1].y = container_bb[1].y.max(bb[1].y);
        }

        SelectionRect::new(container_bb, working_rect)
            .map(|clipped_rect| SelectionRectContainer { container: clipped_rect, children })
    }

    pub fn get_cursor_icon(&self, cursor_pos: egui::Pos2) -> Option<SelectionResponse> {
        if self.container.raw.contains(cursor_pos) {
            return Some(SelectionResponse::new(SelectionOperation::Translation));
        }

        if let Some(left_path) = &self.container.left {
            if pointer_interests_path(left_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::WestScale));
            }
        };
        if let Some(right_path) = &self.container.right {
            if pointer_interests_path(right_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::EastScale));
            }
        };

        if let Some(top_path) = &self.container.top {
            if pointer_interests_path(top_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::NorthScale));
            }
        };
        if let Some(bottom_path) = &self.container.bottom {
            if pointer_interests_path(bottom_path, cursor_pos, None, SCALE_BRUSH_SIZE) {
                return Some(SelectionResponse::new(SelectionOperation::SouthScale));
            }
        };
        None
    }
    pub fn show(&self, ui: &mut egui::Ui) {
        self.children.iter().for_each(|rect| {
            rect.show(ui, true);
        });

        self.container.show(ui, false);
    }

    pub fn show_delete_btn(
        &self, buffer: &mut Buffer, ui: &mut egui::Ui, working_rect: egui::Rect,
    ) -> bool {
        let mut delete_toolbar_dim = egui::pos2(20.0, 20.0);
        let gap = 15.0;
        let icon_size = 19.0;
        if let Some(transform) = buffer.current.attr("transform") {
            let transform = deserialize_transform(transform);

            delete_toolbar_dim.x = (delete_toolbar_dim.x * transform[0] as f32).clamp(20.0, 35.0);
            delete_toolbar_dim.y = (delete_toolbar_dim.y * transform[3] as f32).clamp(20.0, 35.0);

            // gap *= transform[3] as f32;

            if (icon_size * transform[3] as f32) < 13. {
                println!("{:#?}", icon_size * transform[3] as f32);
                return false;
            }
        }

        let delete_toolbar_rect = egui::Rect {
            min: egui::pos2(
                self.container.raw.min.x,
                self.container.raw.min.y - delete_toolbar_dim.y - gap,
            ),
            max: egui::pos2(
                self.container.raw.min.x + delete_toolbar_dim.x,
                self.container.raw.min.y - gap,
            ),
        };

        if !working_rect.contains_rect(delete_toolbar_rect) {
            return false;
        }
        ui.allocate_ui_at_rect(delete_toolbar_rect, |ui| {
            let res = Icon::DELETE.size(icon_size).show(ui);
            let rect = res.rect.translate(egui::vec2(-res.rect.width() / 5.6, 0.0));
            let rect = rect.expand(5.0);
            ui.painter().circle_filled(
                rect.center(),
                (rect.left() - rect.center().x).abs(),
                egui::Color32::GRAY.gamma_multiply(0.3),
            );

            rect.contains(ui.input(|r| r.pointer.hover_pos().unwrap_or_default()))
                && ui.input(|r| r.pointer.primary_clicked())
        })
        .inner
    }
}

pub struct SelectionRect {
    left: Option<Subpath<ManipulatorGroupId>>,
    right: Option<Subpath<ManipulatorGroupId>>,
    top: Option<Subpath<ManipulatorGroupId>>,
    bottom: Option<Subpath<ManipulatorGroupId>>,
    pub raw: egui::Rect,
}

impl SelectionRect {
    fn new(bb: [DVec2; 2], working_rect: egui::Rect) -> Option<Self> {
        // clip the container bb to not overflow the canvas region
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

        let rect = bb_to_rect(bb);

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
            raw: rect,
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

    fn show(&self, ui: &mut egui::Ui, is_child_rect: bool) {
        if let Some(top_path) = &self.top {
            self.show_subpath(top_path, ui, is_child_rect);
        };
        if let Some(bottom_path) = &self.bottom {
            self.show_subpath(bottom_path, ui, is_child_rect);
        };

        if let Some(left_path) = &self.left {
            self.show_subpath(left_path, ui, is_child_rect);

            if self.top.is_some() {
                let corner = left_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui, is_child_rect);
            }
            if self.bottom.is_some() {
                let corner = left_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui, is_child_rect);
            }
        };
        if let Some(right_path) = &self.right {
            self.show_subpath(right_path, ui, is_child_rect);

            if self.top.is_some() {
                let corner = right_path.get_segment(0).unwrap().start();
                self.show_corner(corner, ui, is_child_rect);
            }
            if self.bottom.is_some() {
                let corner = right_path.get_segment(0).unwrap().end();
                self.show_corner(corner, ui, is_child_rect);
            }
        };
    }

    fn show_subpath(
        &self, path: &Subpath<ManipulatorGroupId>, ui: &mut egui::Ui, is_child_rect: bool,
    ) {
        let line_segment = path.get_segment(0).unwrap();
        let line_segment = [
            egui::pos2(line_segment.start().x as f32, line_segment.start().y as f32),
            egui::pos2(line_segment.end().x as f32, line_segment.end().y as f32),
        ];
        ui.painter().line_segment(
            line_segment,
            egui::Stroke {
                width: 1.0,
                color: ui
                    .visuals()
                    .hyperlink_color
                    .gamma_multiply(if is_child_rect { 0.4 } else { 1.0 }),
            },
        );
    }

    fn show_corner(&self, corner: DVec2, ui: &mut egui::Ui, is_child_rect: bool) {
        if is_child_rect {
            return;
        }
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
}
