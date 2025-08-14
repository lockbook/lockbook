use std::collections::VecDeque;
use std::fmt::Debug;

use lb_rs::Uuid;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::element::Element;
use resvg::usvg::Transform;

#[derive(Default, Debug)]
pub struct History {
    undo: VecDeque<Event>,
    redo: Vec<Event>,
}

#[derive(Debug)]
pub enum Event {
    Insert(Vec<InsertElement>),
    Delete(Vec<DeleteElement>),
    Transform(Vec<TransformElement>),
}

#[derive(Clone, Copy, Debug)]
pub struct DeleteElement {
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug)]
pub struct InsertElement {
    pub id: Uuid,
}

#[derive(Clone, Copy, Debug)]
pub struct TransformElement {
    pub id: Uuid,
    pub transform: Transform,
}

impl History {
    pub fn save(&mut self, event: Event) {
        if !self.redo.is_empty() {
            self.redo = vec![];
        }

        self.undo.push_back(event);
    }

    pub fn apply_event(&mut self, event: &Event, buffer: &mut Buffer) {
        match event {
            Event::Insert(payload) => {
                payload.iter().for_each(|insert_payload| {
                    if let Some(el) = buffer.elements.get_mut(&insert_payload.id) {
                        match el {
                            Element::Path(p) => {
                                p.deleted = false;
                                p.diff_state.delete_changed = true;
                            }
                            Element::Image(i) => {
                                i.deleted = false;
                                i.diff_state.delete_changed = true;
                            }
                            Element::Text(_) => todo!(),
                        }
                    }
                });
            }
            Event::Delete(payload) => {
                payload.iter().for_each(|delete_payload| {
                    if let Some(el) = buffer.elements.get_mut(&delete_payload.id) {
                        match el {
                            Element::Path(p) => {
                                p.deleted = true;
                                p.diff_state.delete_changed = true;
                            }
                            Element::Image(i) => {
                                i.deleted = true;
                                i.diff_state.delete_changed = true;
                            }
                            Element::Text(_) => todo!(),
                        }
                    }
                });
            }
            Event::Transform(payload) => {
                payload.iter().for_each(|transform_payload| {
                    if let Some(el) = buffer.elements.get_mut(&transform_payload.id) {
                        el.transform(transform_payload.transform);
                    }
                });
            }
        };
    }

    pub fn undo(&mut self, buffer: &mut Buffer) {
        if self.undo.is_empty() {
            return;
        }

        if let Some(undo_event) = self.undo.pop_back() {
            let undo_event = self.swap_event(undo_event);
            self.apply_event(&undo_event, buffer);
            self.redo.push(undo_event);
        }
    }

    pub fn has_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn redo(&mut self, buffer: &mut Buffer) {
        if self.redo.is_empty() {
            return;
        }

        if let Some(redo_event) = self.redo.pop() {
            let redo_event = self.swap_event(redo_event);
            self.apply_event(&redo_event, buffer);
            self.undo.push_back(redo_event);
        }
    }

    fn swap_event(&self, mut source: Event) -> Event {
        match source {
            Event::Insert(payload) => {
                source = Event::Delete(
                    payload
                        .iter()
                        .map(|insert_payload| DeleteElement { id: insert_payload.id })
                        .collect(),
                );
            }
            Event::Delete(payload) => {
                source = Event::Insert(
                    payload
                        .iter()
                        .map(|delete_payload| InsertElement { id: delete_payload.id })
                        .collect(),
                );
            }
            Event::Transform(mut payload) => {
                source = Event::Transform(
                    payload
                        .iter_mut()
                        .map(|transform_payload| TransformElement {
                            id: transform_payload.id,
                            transform: transform_payload
                                .transform
                                .invert()
                                .unwrap_or(Transform::identity()),
                        })
                        .collect(),
                )
            }
        }
        source
    }

    pub fn has_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
