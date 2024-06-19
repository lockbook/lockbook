use bezier_rs::Subpath;
use glam::DVec2;
use resvg::usvg::Transform;

use crate::tab::svg_editor::{parser::ManipulatorGroupId, Buffer};

use super::{
    rect::SelectionRectContainer, u_transform_to_bezier, SelectedElement, SelectionOperation,
    SelectionResponse,
};

pub fn scale_group_from_center(
    factor: f32, els: &mut [SelectedElement], selected_rect: &SelectionRectContainer,
    buffer: &mut Buffer,
) {
    els.iter_mut().for_each(|el| {
        scale_from_center(factor, el, selected_rect, buffer);
    });
}

pub fn scale_from_center(
    factor: f32, el: &mut SelectedElement, selected_rect: &SelectionRectContainer,
    buffer: &mut Buffer,
) {
    let path: Subpath<ManipulatorGroupId> = Subpath::new_rect(
        DVec2 {
            x: selected_rect.container.raw.min.x as f64,
            y: selected_rect.container.raw.min.y as f64,
        },
        DVec2 {
            x: selected_rect.container.raw.max.x as f64,
            y: selected_rect.container.raw.max.y as f64,
        },
    );

    let bb = path.bounding_box().unwrap();
    let element_rect = egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    };

    if buffer.elements.get_mut(&el.id).is_some() {
        let u_transform = Transform::identity()
            .post_scale(factor, factor)
            .post_translate(
                -(1. - factor) * (element_rect.width() / 2. - element_rect.right()),
                -(1. - factor) * (element_rect.height() / 2. - element_rect.bottom()),
            );
        let b_transform = u_transform_to_bezier(&u_transform);

        match node {
            crate::tab::svg_editor::parser::Element::Path(p) => {
                el.transform = el.transform.post_concat(u_transform);

                p.data.apply_transform(b_transform);
            }
            crate::tab::svg_editor::parser::Element::Image(img) => img.apply_transform(u_transform),
            crate::tab::svg_editor::parser::Element::Text(_) => todo!(),
        }
    }
}

pub fn snap_scale(
    pos: egui::Pos2, els: &mut [SelectedElement], selected_rect: &SelectionRectContainer,
    buffer: &mut Buffer,
) -> Option<egui::CursorIcon> {
    let element_rect = selected_rect.container.raw;

    let top_distance = pos.y - element_rect.min.y;
    let bottom_distance = element_rect.max.y - pos.y;
    let left_distance = pos.x - element_rect.min.x;
    let right_distance = element_rect.max.x - pos.x;

    let min_distance =
        f32::min(f32::min(top_distance, bottom_distance), f32::min(left_distance, right_distance));

    let (res_icon, factor) = if min_distance == top_distance {
        (
            Some(SelectionResponse::new(SelectionOperation::NorthScale).cursor_icon),
            (element_rect.bottom() - pos.y) / element_rect.height().abs(),
        )
    } else if min_distance == bottom_distance {
        (
            Some(SelectionResponse::new(SelectionOperation::SouthScale).cursor_icon),
            (pos.y - element_rect.top()) / element_rect.height().abs(),
        )
    } else if min_distance == right_distance {
        (
            Some(SelectionResponse::new(SelectionOperation::EastScale).cursor_icon),
            (pos.x - element_rect.left()) / element_rect.width().abs(),
        )
    } else {
        (
            Some(SelectionResponse::new(SelectionOperation::WestScale).cursor_icon),
            (element_rect.right() - pos.x) / element_rect.width().abs(),
        )
    };

    els.iter_mut().for_each(|el| {
        scale_from_center(factor, el, selected_rect, buffer);
    });

    res_icon
}
