use bezier_rs::Subpath;
use egui::TextWrapMode;
use glam::DVec2;

use super::{SelectedElement, SelectionOperation, SelectionResponse};
use crate::{
    tab::svg_editor::{
        parser::ManipulatorGroupId,
        util::{bb_to_rect, pointer_intersects_outline, rect_to_bb},
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
    pub fn new(els: &[SelectedElement], buffer: &mut Buffer) -> Option<Self> {
        if els.is_empty() {
            return None;
        }
        let mut container_bb = [DVec2::new(f64::MAX, f64::MAX), DVec2::new(f64::MIN, f64::MIN)];
        let mut children = vec![];
        for el in els.iter() {
            let rect = match buffer.elements.get(&el.id) {
                Some(el) => el.bounding_box(),
                None => continue,
            };
            let bb = rect_to_bb(rect);

            if let Some(clipped_rect) = SelectionRect::new(bb) {
                children.push(clipped_rect);
            }

            container_bb[0].x = container_bb[0].x.min(bb[0].x);
            container_bb[0].y = container_bb[0].y.min(bb[0].y);

            container_bb[1].x = container_bb[1].x.max(bb[1].x);
            container_bb[1].y = container_bb[1].y.max(bb[1].y);
        }

        SelectionRect::new(container_bb)
            .map(|clipped_rect| SelectionRectContainer { container: clipped_rect, children })
    }

    pub fn get_cursor_icon(&self, cursor_pos: egui::Pos2) -> Option<SelectionResponse> {
        if self.container.raw.contains(cursor_pos) {
            return Some(SelectionResponse::new(SelectionOperation::Translation));
        }

        if pointer_intersects_outline(&self.container.left, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(SelectionResponse::new(SelectionOperation::WestScale));
        }
        if pointer_intersects_outline(&self.container.right, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(SelectionResponse::new(SelectionOperation::EastScale));
        }

        if pointer_intersects_outline(&self.container.top, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(SelectionResponse::new(SelectionOperation::NorthScale));
        }
        if pointer_intersects_outline(&self.container.bottom, cursor_pos, None, SCALE_BRUSH_SIZE) {
            return Some(SelectionResponse::new(SelectionOperation::SouthScale));
        }
        None
    }

    pub fn show(&self, ui: &mut egui::Ui, painter: &egui::Painter) {
        self.children.iter().for_each(|rect| {
            rect.show(ui, painter, true);
        });

        self.container.show(ui, painter, false);
    }

    pub fn show_delete_btn(&self, ui: &mut egui::Ui, painter: &egui::Painter) -> bool {
        let delete_toolbar_dim = egui::pos2(20.0, 20.0);
        let gap_between_btn_and_rect = 15.0;
        let icon_size = 19.0;

        let delete_toolbar_rect = egui::Rect {
            min: egui::pos2(
                self.container.raw.min.x,
                self.container.raw.min.y - delete_toolbar_dim.y - gap_between_btn_and_rect,
            ),
            max: egui::pos2(
                self.container.raw.min.x + delete_toolbar_dim.x,
                self.container.raw.min.y - gap_between_btn_and_rect,
            ),
        };

        if ui.is_rect_visible(delete_toolbar_rect) {
            // let text_color = ui.style().interact(&res).text_color();
            let wrap_width = ui.available_width();

            let icon_pos = egui::pos2(
                delete_toolbar_rect.min.x,
                delete_toolbar_rect.center().y - icon_size / 2.0,
            );

            let icon: egui::WidgetText = (&Icon::DELETE).into();
            let icon =
                icon.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);

            painter.galley(icon_pos, icon, ui.style().visuals.hyperlink_color);

            let padding = 7.0;
            painter.rect_filled(
                delete_toolbar_rect.expand(padding),
                egui::Rounding::same(10.0),
                ui.style().visuals.hyperlink_color.gamma_multiply(0.1),
            );
        }
        delete_toolbar_rect.contains(ui.input(|r| r.pointer.hover_pos().unwrap_or_default()))
            && ui.input(|r| r.pointer.primary_clicked())
    }
}

pub struct SelectionRect {
    left: Subpath<ManipulatorGroupId>,
    right: Subpath<ManipulatorGroupId>,
    top: Subpath<ManipulatorGroupId>,
    bottom: Subpath<ManipulatorGroupId>,
    pub raw: egui::Rect,
}

impl SelectionRect {
    fn new(bb: [DVec2; 2]) -> Option<Self> {
        // clip the container bb to not overflow the canvas region

        let rect = bb_to_rect(bb);

        let selection_rect = SelectionRect {
            left: Subpath::from_anchors(
                [DVec2 { x: bb[0].x, y: bb[0].y }, DVec2 { x: bb[0].x, y: bb[1].y }],
                false,
            ),
            right: Subpath::from_anchors(
                [DVec2 { x: bb[1].x, y: bb[0].y }, DVec2 { x: bb[1].x, y: bb[1].y }],
                false,
            ),
            top: Subpath::from_anchors(
                [DVec2 { x: bb[0].x, y: bb[0].y }, DVec2 { x: bb[1].x, y: bb[0].y }],
                false,
            ),
            bottom: Subpath::from_anchors(
                [DVec2 { x: bb[0].x, y: bb[1].y }, DVec2 { x: bb[1].x, y: bb[1].y }],
                false,
            ),
            raw: rect,
        };

        Some(selection_rect)
    }

    fn show(&self, ui: &mut egui::Ui, painter: &egui::Painter, is_child_rect: bool) {
        self.show_subpath(&self.top, ui, painter, is_child_rect);
        self.show_subpath(&self.bottom, ui, painter, is_child_rect);
        self.show_subpath(&self.right, ui, painter, is_child_rect);
        self.show_subpath(&self.left, ui, painter, is_child_rect);

        let corner = self.left.get_segment(0).unwrap().start();
        self.show_corner(corner, ui, painter, is_child_rect);
        let corner = self.left.get_segment(0).unwrap().end();
        self.show_corner(corner, ui, painter, is_child_rect);

        let corner = self.right.get_segment(0).unwrap().start();
        self.show_corner(corner, ui, painter, is_child_rect);
        let corner = self.right.get_segment(0).unwrap().end();
        self.show_corner(corner, ui, painter, is_child_rect);
    }

    fn show_subpath(
        &self, path: &Subpath<ManipulatorGroupId>, ui: &mut egui::Ui, painter: &egui::Painter,
        is_child_rect: bool,
    ) {
        let line_segment = path.get_segment(0).unwrap();
        let line_segment = [
            egui::pos2(line_segment.start().x as f32, line_segment.start().y as f32),
            egui::pos2(line_segment.end().x as f32, line_segment.end().y as f32),
        ];
        painter.line_segment(
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

    fn show_corner(
        &self, corner: DVec2, ui: &mut egui::Ui, painter: &egui::Painter, is_child_rect: bool,
    ) {
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
        painter.rect(
            rect,
            egui::Rounding::ZERO,
            egui::Color32::WHITE,
            egui::Stroke { width: 1.0, color: ui.visuals().hyperlink_color },
        );
    }
}
