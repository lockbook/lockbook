use std::{collections::VecDeque, fmt::Debug, mem};

use bezier_rs::Identifier;
use resvg::usvg::{Transform, Visibility};

use super::parser;

const MAX_UNDOS: usize = 100;

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
    pub old_transform: Transform,
    pub new_transform: Transform,
}

impl History {
    pub fn save(&mut self, event: Event) {
        if !self.redo.is_empty() {
            self.redo = vec![];
        }

        self.undo.push_back(event);
        if self.undo.len() > MAX_UNDOS {
            self.undo.pop_front();
        }
    }

    pub fn apply_event(&mut self, event: &Event, buffer: &mut parser::Buffer) {
        match event {
            Event::Insert(payload) => {}
            Event::Delete(payload) => {
                payload.iter().for_each(|delete_payload| {
                    if let Some(el) = buffer.elements.get_mut(&delete_payload.id) {
                        match el {
                            parser::Element::Path(p) => p.visibility = Visibility::Hidden,
                            parser::Element::Image(img) => img.visibility = Visibility::Hidden,
                            _ => {}
                        }
                    }
                });
            }
            Event::Transform(payload) => {
                payload.iter().for_each(|transform_payload| {
                    if let Some(el) = buffer.elements.get_mut(&transform_payload.id) {
                        match el {
                            parser::Element::Path(p) => {
                                p.transform = transform_payload.new_transform
                            }
                            parser::Element::Image(img) => {
                                img.transform = transform_payload.new_transform
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
                        .map(|transform_payload| {
                            mem::swap(
                                &mut transform_payload.new_transform,
                                &mut transform_payload.old_transform,
                            );
                            transform_payload.clone()
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
