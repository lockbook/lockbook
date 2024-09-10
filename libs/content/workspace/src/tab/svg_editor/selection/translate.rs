use resvg::usvg::Transform;

use crate::tab::svg_editor::{
    history::{History, TransformElement},
    util::pointer_intersects_element,
    Buffer, Event,
};

use super::SelectedElement;

pub fn end_translation(
    buffer: &mut Buffer, history: &mut History, els: &mut [SelectedElement], pos: egui::Pos2,
    save_event: bool,
) {
    let events: Vec<TransformElement> = els
        .iter_mut()
        .filter_map(|el| {
            el.prev_pos = pos;
            if buffer.elements.get_mut(&el.id).is_some() {
                if save_event {
                    Some(TransformElement { id: el.id.to_owned(), transform: el.transform })
                } else {
                    None
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
        if el.deleted() {
            continue;
        }
        if pointer_intersects_element(el, current_pos, last_pos, 10.0) {
            return Some(SelectedElement {
                id: *id,
                prev_pos: current_pos,
                transform: Transform::identity(),
            });
        }
    }
    None
}
