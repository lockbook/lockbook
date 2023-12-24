use std::{collections::VecDeque, fmt::Debug, mem};

use bezier_rs::{Bezier, Identifier, ManipulatorGroup, Subpath};
use glam::{DAffine2, DMat2, DVec2};
use minidom::Element;
use resvg::{
    tiny_skia::Point,
    usvg::{self, Node, NodeKind, TreeWriting},
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
    TransformElements(TransformElements),
}

#[derive(Clone, Debug)]
pub struct DeleteElements {
    pub elements: HashMap<String, Element>,
}

#[derive(Clone, Debug)]
pub struct InsertElements {
    pub elements: HashMap<String, Element>,
}

#[derive(Clone, Debug)]
pub struct TransformElements {
    /// (old transform, new transform)
    pub elements: HashMap<String, (String, String)>,
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
        self.needs_path_map_update = true;

        self.undo.push_back(event);
        if self.undo.len() > MAX_UNDOS {
            self.undo.pop_front();
        }
    }

    pub fn apply_event(&mut self, event: &Event) {
        match event {
            Event::InsertElements(payload) => {
                payload.elements.iter().for_each(|(id, element)| {
                    if let Some(node) = util::node_by_id(&mut self.current, id.to_string()) {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("d", element.attr("d"));
                    } else {
                        self.current.append_child(element.clone());
                    }
                });
            }

            Event::DeleteElements(payload) => {
                payload.elements.iter().for_each(|(id, _)| {
                    self.current.remove_child(id);
                });
            }
            Event::TransformElements(payload) => {
                payload.elements.iter().for_each(|(id, transform)| {
                    if let Some(node) = util::node_by_id(&mut self.current, id.to_string()) {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("transform", transform.1.clone());
                    }
                });
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
            Event::TransformElements(mut payload) => {
                payload
                    .elements
                    .iter_mut()
                    .for_each(|(_, transform)| mem::swap(&mut transform.0, &mut transform.1));
                source = Event::TransformElements(TransformElements { elements: payload.elements })
            }
        }
        source
    }

    pub fn has_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // todo: just accept the dom tree, and escalete the tree to the render tree by parsing those elements one by one
    /**
     * <g transform="matrix(...)">
     *  <path transform d>
     *  <path transform d>
     *  <path transform d>
     *  
     * </g>
     */
    pub fn recalc_paths(&mut self) {
        for el in self.current.children() {
            if el.name().eq("path") {
                let data = match el.attr("d") {
                    Some(d) => d,
                    None => continue,
                };
                let mut start = (0.0, 0.0);
                let mut subpath: Subpath<ManipulatorGroupId> = Subpath::new(vec![], false);
                // todo: remove when path deletion is fixed
                if data.eq("") {
                    continue;
                }

                for segment in svgtypes::SimplifyingPathParser::from(data) {
                    let segment = match segment {
                        Ok(v) => v,
                        Err(_) => break,
                    };

                    match segment {
                        svgtypes::SimplePathSegment::MoveTo { x, y } => {
                            start = (x, y);
                        }
                        svgtypes::SimplePathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                            let bez = Bezier::from_cubic_coordinates(
                                start.0, start.1, x1, y1, x2, y2, x, y,
                            );
                            subpath.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
                            start = (x, y)
                        }
                        _ => { /*handle error */ }
                    }
                }

                if let Some(transform) = el.attr("transform") {
                    for segment in svgtypes::TransformListParser::from(transform) {
                        let segment = match segment {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                        match segment {
                            svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } => {
                                subpath.apply_transform(DAffine2 {
                                    matrix2: DMat2 {
                                        x_axis: DVec2 { x: a, y: b },
                                        y_axis: DVec2 { x: c as f64, y: d },
                                    },
                                    translation: DVec2 { x: e, y: f },
                                });
                            }
                            _ => { /* todo: maybe convert to matrix? */ }
                        }
                    }
                }
                if let Some(id) = el.attr("id") {
                    self.paths.insert(id.to_string(), subpath);
                }
            }
        }
        self.needs_path_map_update = false;
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

fn get_subpath_from_points(data: &[Point]) -> Subpath<ManipulatorGroupId> {
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
