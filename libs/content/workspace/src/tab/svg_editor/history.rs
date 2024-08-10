use std::{collections::VecDeque, fmt::Debug};

use glam::DAffine2;

use super::{parser, selection::bezier_transform_to_u};

#[derive(Default)]
pub struct History {
    undo: VecDeque<Event>,
    redo: Vec<Event>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Insert(Vec<InsertElement>),
    Delete(Vec<DeleteElement>),
    Transform(Vec<TransformElement>),
}

#[derive(Clone, Debug)]
pub struct DeleteElement {
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct InsertElement {
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct TransformElement {
    pub id: String,
    pub transform: DAffine2,
}

impl History {
    pub fn save(&mut self, event: Event) {
        if !self.redo.is_empty() {
            self.redo = vec![];
        }

        self.undo.push_back(event);
    }

    pub fn apply_event(&mut self, event: &Event, buffer: &mut parser::Buffer) {
        match event {
            Event::Insert(payload) => {
                payload.iter().for_each(|insert_payload| {
                    if let Some(el) = buffer.elements.get_mut(&insert_payload.id) {
                        match el {
                            parser::Element::Path(p) => {
                                p.deleted = false;
                                p.diff_state.delete_changed = true;
                            }
                            parser::Element::Image(i) => {
                                i.deleted = false;
                                i.diff_state.delete_changed = true;
                            }
                            parser::Element::Text(_) => todo!(),
                        }
                    }
                });
            }
            Event::Delete(payload) => {
                payload.iter().for_each(|delete_payload| {
                    if let Some(el) = buffer.elements.get_mut(&delete_payload.id) {
                        match el {
                            parser::Element::Path(p) => {
                                p.deleted = true;
                                p.diff_state.delete_changed = true;
                            }
                            parser::Element::Image(i) => {
                                i.deleted = true;
                                i.diff_state.delete_changed = true;
                            }
                            parser::Element::Text(_) => todo!(),
                        }
                    }
                });
            }
            Event::Transform(payload) => {
                payload.iter().for_each(|transform_payload| {
                    if let Some(el) = buffer.elements.get_mut(&transform_payload.id) {
                        match el {
                            parser::Element::Path(p) => {
                                p.data.apply_transform(transform_payload.transform);
                            }
                            parser::Element::Image(img) => {
                                img.apply_transform(bezier_transform_to_u(
                                    &transform_payload.transform,
                                ));
                            }
                            _ => {}
                        }
                    }
                });
            }
        };
    }

    pub fn undo(&mut self, buffer: &mut parser::Buffer) {
        if self.undo.is_empty() {
            return;
        }

        if let Some(undo_event) = self.undo.pop_back().to_owned() {
            let undo_event = self.swap_event(undo_event);
            self.apply_event(&undo_event, buffer);
            self.redo.push(undo_event);
        }
    }

    pub fn has_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn redo(&mut self, buffer: &mut parser::Buffer) {
        if self.redo.is_empty() {
            return;
        }

        if let Some(redo_event) = self.redo.pop().to_owned() {
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
                        .map(|insert_payload| DeleteElement { id: insert_payload.id.clone() })
                        .collect(),
                );
            }
            Event::Delete(payload) => {
                source = Event::Insert(
                    payload
                        .iter()
                        .map(|delete_payload| InsertElement { id: delete_payload.id.clone() })
                        .collect(),
                );
            }
            Event::Transform(mut payload) => {
                source = Event::Transform(
                    payload
                        .iter_mut()
                        .map(|transform_payload| TransformElement {
                            id: transform_payload.id.clone(),
                            transform: transform_payload.transform.inverse(),
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