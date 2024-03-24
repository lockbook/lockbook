use bezier_rs::Subpath;
use glam::{DAffine2, DMat2, DVec2};

use crate::tab::svg_editor::{
    history::ManipulatorGroupId,
    node_by_id,
    util::{deserialize_transform, serialize_transform},
    Buffer,
};

use super::{rect::SelectionRect, SelectedElement, SelectionOperation, SelectionResponse};

pub fn scale_group_from_center(
    factor: f64, els: &mut [SelectedElement], selected_rect: &SelectionRect, buffer: &mut Buffer,
) {
    for el in els.iter_mut() {
        scale_from_center(factor, el, selected_rect, buffer)
    }
}

pub fn scale_from_center(
    factor: f64, el: &mut SelectedElement, selected_rect: &SelectionRect, buffer: &mut Buffer,
) {
    let mut path: Subpath<ManipulatorGroupId> = Subpath::new_rect(
        DVec2 { x: selected_rect.rect.min.x as f64, y: selected_rect.rect.min.y as f64 },
        DVec2 { x: selected_rect.rect.max.x as f64, y: selected_rect.rect.max.y as f64 },
    );

    // the inverse of the master transform will get the location of the
    // path's in terms of the svg viewport instead of the default egui
    // viewport. those cords are used for center based scaling.
    if let Some(transform) = buffer.current.attr("transform") {
        let [a, b, c, d, e, f] = deserialize_transform(transform);
        path.apply_transform(
            DAffine2 {
                matrix2: DMat2 { x_axis: DVec2 { x: a, y: b }, y_axis: DVec2 { x: c, y: d } },
                translation: DVec2 { x: e, y: f },
            }
            .inverse(),
        );
    }

    let bb = path.bounding_box().unwrap();
    let element_rect = egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    };

    if let Some(node) = node_by_id(&mut buffer.current, el.id.clone()) {
        let mut scaled_matrix = deserialize_transform(node.attr("transform").unwrap_or_default());
        scaled_matrix = scaled_matrix.map(|n| n * factor);

        // after scaling the matrix, a corrective translate is applied
        // to ensure that it's scaled from the center
        scaled_matrix[4] -=
            (1. - factor) * (element_rect.width() / 2. - element_rect.right()) as f64;
        scaled_matrix[5] -=
            (1. - factor) * (element_rect.height() / 2. - element_rect.bottom()) as f64;

        node.set_attr("transform", serialize_transform(&scaled_matrix));
        buffer.needs_path_map_update = true;
    }
}

pub fn snap_scale(
    pos: egui::Pos2, els: &mut [SelectedElement], selected_rect: &SelectionRect,
    buffer: &mut Buffer,
) -> Option<egui::CursorIcon> {
    let mut res_icon = None;

    let element_rect = selected_rect.rect;

    let top_distance = pos.y - element_rect.min.y;
    let bottom_distance = element_rect.max.y - pos.y;
    let left_distance = pos.x - element_rect.min.x;
    let right_distance = element_rect.max.x - pos.x;

    let min_distance =
        f32::min(f32::min(top_distance, bottom_distance), f32::min(left_distance, right_distance));

    let factor = if min_distance == top_distance {
        res_icon = Some(SelectionResponse::new(SelectionOperation::NorthScale).cursor_icon);
        (element_rect.bottom() - pos.y) / element_rect.height().abs()
    } else if min_distance == bottom_distance {
        res_icon = Some(SelectionResponse::new(SelectionOperation::SouthScale).cursor_icon);
        (pos.y - element_rect.top()) / element_rect.height().abs()
    } else if min_distance == right_distance {
        res_icon = Some(SelectionResponse::new(SelectionOperation::EastScale).cursor_icon);
        (pos.x - element_rect.left()) / element_rect.width().abs()
    } else {
        res_icon = Some(SelectionResponse::new(SelectionOperation::WestScale).cursor_icon);
        (element_rect.right() - pos.x) / element_rect.width().abs()
    };

    for el in els.iter_mut() {
        scale_from_center(factor as f64, el, selected_rect, buffer);
    }
    res_icon
}
