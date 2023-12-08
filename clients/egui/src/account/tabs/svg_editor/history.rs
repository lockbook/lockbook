use std::{collections::VecDeque, fmt::Debug};

use bezier_rs::{Bezier, Identifier, Subpath};
use minidom::Element;
use resvg::{
    tiny_skia::Point,
    usvg::{Node, NodeKind},
};
use std::collections::HashMap;

use super::util;

const MAX_UNDOS: usize = 100;

pub struct Buffer {
    pub current: Element,
    pub paths: HashMap<String, Subpath<ManipulatorGroupId>>,
    undo: VecDeque<Event>,
    redo: Vec<Event>,
    pub needs_path_map_update: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ManipulatorGroupId;

impl Identifier for ManipulatorGroupId {
    fn new() -> Self {
        ManipulatorGroupId
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    InsertElements(InsertElements),
    DeleteElements(DeleteElements),
}

#[derive(Clone, Debug)]

pub struct DeleteElements {
    pub elements: HashMap<String, Element>,
}

#[derive(Clone, Debug)]

pub struct InsertElements {
    pub elements: HashMap<String, Element>,
}

impl Buffer {
    pub fn new(root: Element) -> Self {
        Buffer {
            current: root,
            undo: VecDeque::default(),
            redo: vec![],
            paths: HashMap::default(),
            needs_path_map_update: true,
        }
    }

    pub fn save(&mut self, event: Event) {
        if !self.redo.is_empty() {
            self.redo = vec![];
        }

        self.undo.push_back(event);
        if self.undo.len() > MAX_UNDOS {
            self.undo.pop_front();
        }
    }

    fn apply_event(&mut self, event: &Event) {
        match event {
            Event::InsertElements(payload) => {
                payload.elements.iter().for_each(|(id, element)| {
                    if let Some(node) = util::node_by_id(&mut self.current, id.to_string()) {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("d", element.attr("d"));
                        node.set_attr("opacity", "1"); // this is  bad  but works
                    } else {
                        self.current.append_child(element.clone());
                    }
                });
                self.needs_path_map_update = true;
            }

            Event::DeleteElements(payload) => {
                payload.elements.iter().for_each(|(id, _)| {
                    if let Some(node) = util::node_by_id(&mut self.current, id.to_string()) {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("d", "");
                    }
                });
                self.needs_path_map_update = true;
            }
        };
    }

    pub fn undo(&mut self) {
        if self.undo.is_empty() {
            return;
        }

        if let Some(undo_event) = self.undo.pop_back().to_owned() {
            let undo_event = self.swap_event(undo_event);
            self.apply_event(&undo_event);
            self.redo.push(undo_event);
        }
    }

    pub fn has_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn redo(&mut self) {
        if self.redo.is_empty() {
            return;
        }

        if let Some(redo_event) = self.redo.pop().to_owned() {
            let redo_event = self.swap_event(redo_event);
            self.apply_event(&redo_event);
            self.undo.push_back(redo_event);
        }
    }

    fn swap_event(&self, mut source: Event) -> Event {
        match source {
            Event::InsertElements(payload) => {
                source = Event::DeleteElements(DeleteElements { elements: payload.elements });
            }
            Event::DeleteElements(payload) => {
                source = Event::InsertElements(InsertElements { elements: payload.elements });
            }
        }
        source
    }

    pub fn has_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn recalc_paths(&mut self, utree: &Node) {
        for el in utree.children() {
            if let NodeKind::Path(ref p) = *el.borrow() {
                self.paths
                    .insert(p.id.clone(), convert_path_to_bezier(p.data.points()));
            }
        }
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        let mut out = Vec::new();
        self.current.write_to(&mut out).unwrap();
        std::str::from_utf8(&out).unwrap().replace("xmlns='' ", "")
    }
}

impl Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("undo", &self.undo)
            .field("redo", &self.redo)
            .finish()
    }
}

fn convert_path_to_bezier(data: &[Point]) -> Subpath<ManipulatorGroupId> {
    let mut bez = vec![];
    let mut i = 1;
    while i < data.len() - 2 {
        bez.push(Bezier::from_cubic_coordinates(
            data[i - 1].x as f64,
            data[i - 1].y as f64,
            data[i].x as f64,
            data[i].y as f64,
            data[i + 1].x as f64,
            data[i + 1].y as f64,
            data[i + 2].x as f64,
            data[i + 2].y as f64,
        ));
        i += 1;
    }
    Subpath::from_beziers(&bez, false)
}
