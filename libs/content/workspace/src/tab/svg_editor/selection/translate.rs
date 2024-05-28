use resvg::usvg::Transform;

use crate::tab::svg_editor::{
    history::{self, History, TransformElement},
    node_by_id,
    util::{deserialize_transform, pointer_interests_path},
    Buffer, Event,
};

use super::{u_transform_to_bezier, SelectedElement};

// pub fn save_translate(delta: egui::Pos2, de: &mut SelectedElement, buffer: &mut Buffer) {
//     if let Some(node) = node_by_id(&mut buffer.current, de.id.clone()) {
//         node.set_attr(
//             "transform",
//             format!(
//                 "matrix({},0,0,{},{},{} )",
//                 de.original_matrix.1[0],
//                 de.original_matrix.1[3],
//                 delta.x as f64 + de.original_matrix.clone().1[4],
//                 delta.y as f64 + de.original_matrix.clone().1[5]
//             ),
//         );
//         buffer.needs_path_map_update = true;
//     }
// }

// pub fn save_translates(delta: egui::Pos2, els: &mut [SelectedElement], buffer: &mut Buffer) {
//     els.iter().for_each(|el| {
//         if let Some(node) = node_by_id(&mut buffer.current, el.id.clone()) {
//             node.set_attr(
//                 "transform",
//                 format!(
//                     "matrix({},0,0,{},{},{} )",
//                     el.original_matrix.1[0],
//                     el.original_matrix.1[3],
//                     delta.x as f64 + el.original_matrix.clone().1[4],
//                     delta.y as f64 + el.original_matrix.clone().1[5]
//                 ),
//             );
//             buffer.needs_path_map_update = true;
//         }
//     });
// }

pub fn end_translation(
    buffer: &mut Buffer, history: &mut History, els: &mut [SelectedElement], pos: egui::Pos2,
    save_event: bool,
) {
    let events: Vec<TransformElement> = els
        .iter_mut()
        .filter_map(|el| {
            el.prev_pos = pos;
            if let Some(node) = buffer.elements.get_mut(&el.id) {
                match node {
                    crate::tab::svg_editor::parser::Element::Path(_) => {
                        if save_event {
                            Some(TransformElement {
                                id: el.id.to_owned(),
                                transform: u_transform_to_bezier(&el.transform),
                            })
                        } else {
                            None
                        }
                    }
                    crate::tab::svg_editor::parser::Element::Image(_) => todo!(),
                    crate::tab::svg_editor::parser::Element::Text(_) => todo!(),
                }
            } else {
                None
            }
        })
        .collect();
    if !events.is_empty() {
        history.save(Event::Transform(events));
    }
}

pub fn detect_translation(
    buffer: &mut Buffer, last_pos: Option<egui::Pos2>, current_pos: egui::Pos2,
) -> Option<SelectedElement> {
    for (id, el) in buffer.elements.iter() {
        match el {
            crate::tab::svg_editor::parser::Element::Path(p) => {
                if pointer_interests_path(&p.data, current_pos, last_pos, 10.0) {
                    return Some(SelectedElement {
                        id: id.clone(),
                        prev_pos: current_pos,
                        transform: Transform::identity(),
                    });
                }
            }
            crate::tab::svg_editor::parser::Element::Image(_) => todo!(),
            crate::tab::svg_editor::parser::Element::Text(_) => todo!(),
        }
    }
    None
}
