use std::{collections::VecDeque, fmt::Debug, mem};

use bezier_rs::{Identifier, Subpath};
use glam::{DAffine2, DMat2, DVec2};
use minidom::Element;
use resvg::usvg::{NodeKind, Tree};

use std::collections::HashMap;

use super::{
    util::{self, d_to_subpath, deserialize_transform},
    zoom::{verify_zoom_g, G_CONTAINER_ID},
};

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
    Insert(Vec<InsertElement>),
    Delete(Vec<DeleteElement>),
    Transform(Vec<TransformElement>),
}

#[derive(Clone, Debug)]
pub struct DeleteElement {
    pub id: String,
    pub element: Element,
}

#[derive(Clone, Debug)]
pub struct InsertElement {
    pub id: String,
    pub element: Element,
}

#[derive(Clone, Debug)]
pub struct TransformElement {
    pub id: String,
    pub old_transform: String,
    pub new_transform: String,
}

impl Buffer {
    pub fn new(root: Element) -> Self {
        let mut buff = Buffer {
            current: root,
            undo: VecDeque::default(),
            redo: vec![],
            paths: HashMap::default(),
            needs_path_map_update: true,
        };
        if let Some(first_child) = buff.current.children().next() {
            if first_child
                .attr("id")
                .unwrap_or_default()
                .eq(G_CONTAINER_ID)
            {
                buff.current = first_child.clone();
            } else {
                verify_zoom_g(&mut buff);
            }
        } else {
            verify_zoom_g(&mut buff);
        }
        buff
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
            Event::Insert(payload) => {
                payload.iter().for_each(|insert_payload| {
                    if let Some(node) =
                        util::node_by_id(&mut self.current, insert_payload.id.to_string())
                    {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("d", insert_payload.element.attr("d"));
                    } else {
                        self.current.append_child(insert_payload.element.clone());
                    }
                });
            }

            Event::Delete(payload) => {
                payload.iter().for_each(|delete_payload| {
                    self.current.remove_child(&delete_payload.id);
                });
            }
            Event::Transform(payload) => {
                payload.iter().for_each(|transform_payload| {
                    if let Some(node) =
                        util::node_by_id(&mut self.current, transform_payload.id.to_string())
                    {
                        node.set_attr("transform", transform_payload.new_transform.clone());
                    }
                });
            }
        };
        self.needs_path_map_update = true;
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
            Event::Insert(payload) => {
                source = Event::Delete(
                    payload
                        .iter()
                        .map(|insert_payload| DeleteElement {
                            id: insert_payload.id.clone(),
                            element: insert_payload.element.clone(),
                        })
                        .collect(),
                );
            }
            Event::Delete(payload) => {
                source = Event::Insert(
                    payload
                        .iter()
                        .map(|delete_payload| InsertElement {
                            id: delete_payload.id.clone(),
                            element: delete_payload.element.clone(),
                        })
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

    pub fn recalc_paths(&mut self, utree: &Tree) {
        self.paths.clear();

        let mut master_transform = None;
        if let Some(transform) = self.current.attr("transform") {
            master_transform = Some(deserialize_transform(transform));
        }

        for el in self.current.children() {
            if el.name().eq("path") {
                let data = match el.attr("d") {
                    Some(d) => d,
                    None => continue,
                };
                let id = match el.attr("id") {
                    Some(d) => d,
                    None => continue,
                };
                let mut subpath = d_to_subpath(data);

                apply_compounded_transforms(master_transform, el.attr("transform"), &mut subpath);
                self.paths.insert(id.to_string(), subpath);
            }
        }

        // insert images
        for child in utree.root.descendants() {
            if let NodeKind::Image(ref img) = *child.borrow() {
                if img.id.is_empty() {
                    continue;
                }

                let corner1 = DVec2 {
                    y: img.view_box.rect.bottom() as f64,
                    x: img.view_box.rect.right() as f64,
                };
                let corner2 =
                    DVec2 { y: img.view_box.rect.top() as f64, x: img.view_box.rect.left() as f64 };
                let mut path: Subpath<ManipulatorGroupId> = Subpath::new_rect(corner1, corner2);

                let el = self
                    .current
                    .children()
                    .find(|el| el.attr("id").unwrap_or_default().eq(&img.id));
                let mut transform = None;
                if let Some(e) = el {
                    transform = e.attr("transform");
                }

                apply_compounded_transforms(master_transform, transform, &mut path);
                self.paths.insert(img.id.to_string(), path);
            }
        }

        self.needs_path_map_update = false;
    }
}

fn apply_compounded_transforms(
    master_transform: Option<[f64; 6]>, local_transform: Option<&str>,
    subpath: &mut Subpath<ManipulatorGroupId>,
) {
    if let Some(transform) = local_transform {
        let [a, b, c, d, e, f] = deserialize_transform(transform);

        subpath.apply_transform(DAffine2 {
            matrix2: DMat2 { x_axis: DVec2 { x: a, y: b }, y_axis: DVec2 { x: c, y: d } },
            translation: DVec2 { x: e, y: f },
        });
    }

    if let Some([a, b, c, d, e, f]) = master_transform {
        subpath.apply_transform(DAffine2 {
            matrix2: DMat2 { x_axis: DVec2 { x: a, y: b }, y_axis: DVec2 { x: c, y: d } },
            translation: DVec2 { x: e, y: f },
        });
    }
}

impl ToString for Buffer {
    fn to_string(&self) -> String {
        let mut out = Vec::new();
        if let Err(msg) = self.current.write_to(&mut out) {
            println!("{:#?}", msg);
        }
        let out = std::str::from_utf8(&out)
            .unwrap()
            .replace("href", "xlink:href"); // risky
        let out = out.replace("xmlns='' ", "");
        let out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</svg>", out);
        out
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
