use std::{collections::VecDeque, fmt::Debug};

use minidom::Element;
use std::collections::HashMap;

use super::util;

const MAX_UNDOS: usize = 100;

pub struct Buffer {
    pub current: Element,
    undo: VecDeque<Event>,
    redo: Vec<Event>,
}

#[derive(Clone, Debug)]
pub enum Event {
    UpdateOpacity(UpdateOpacity),
    InsertElements(InsertElements),
    DeleteElements(DeleteElements),
}

#[derive(Clone, Debug)]

struct UpdateOpacity {
    id: String,
    opacity: String,
    prev_opacity: String,
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
        Buffer { current: root, undo: VecDeque::default(), redo: vec![] }
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
            Event::UpdateOpacity(payload) => {
                if let Some(node) = util::node_by_id(&mut self.current, payload.id.to_string()) {
                    node.set_attr("opacity", payload.opacity.to_string());
                }
            }
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
            }

            Event::DeleteElements(payload) => {
                payload.elements.iter().for_each(|(id, _)| {
                    if let Some(node) = util::node_by_id(&mut self.current, id.to_string()) {
                        // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                        node.set_attr("d", "");
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
            Event::UpdateOpacity(ref mut payload) => {
                std::mem::swap(&mut payload.opacity, &mut payload.prev_opacity);
            }
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use minidom::Element;

    use crate::account::tabs::svg_editor::{history::DeleteElements, util};

    use super::{Buffer, UpdateOpacity};

    #[test]
    fn opacity_change() {
        let test_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" >
            <path id="1" d="M 10 80 Q 95 10 180 80" stroke="black" fill="transparent"/>
        </svg>
        "#;

        let root: Element = test_svg.parse().unwrap();

        let mut bf = Buffer::new(root);

        bf.save(super::Event::UpdateOpacity(UpdateOpacity {
            id: "1".to_string(),
            opacity: "0.5".to_string(),
            prev_opacity: "1".to_string(),
        }));

        assert!(
            "0.5"
                == util::node_by_id(&mut bf.current, "1".to_string())
                    .unwrap()
                    .attr("opacity")
                    .unwrap()
        );

        bf.undo();

        assert!(
            "1" == util::node_by_id(&mut bf.current, "1".to_string())
                .unwrap()
                .attr("opacity")
                .unwrap()
        );

        bf.redo();

        assert!(
            "0.5"
                == util::node_by_id(&mut bf.current, "1".to_string())
                    .unwrap()
                    .attr("opacity")
                    .unwrap()
        );
    }
    #[test]
    fn insert_element() {
        let test_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" >
        <path d="moon" id ="1" />
        <path d="mars" id ="2" />
        </svg>
        "#;

        let root: Element = test_svg.parse().unwrap();

        let mut bf = Buffer::new(root);

        let el1 = bf
            .current
            .children()
            .find(|e| e.attr("id").unwrap().eq(&"1".to_string()))
            .unwrap()
            .to_owned();

        let el2 = bf
            .current
            .children()
            .find(|e| e.attr("id").unwrap().eq(&"2".to_string()))
            .unwrap()
            .to_owned();

        bf.save(super::Event::DeleteElements(DeleteElements {
            elements: HashMap::from_iter([("1".to_string(), el1), ("2".to_string(), el2)]),
        }));

        if let Some(n) = util::node_by_id(&mut bf.current, "1".to_string()) {
            n.set_attr("d", "");
        }

        if let Some(n) = util::node_by_id(&mut bf.current, "2".to_string()) {
            n.set_attr("d", "");
        }
        println!("{}", bf.to_string());
        bf.undo();
        println!("{}", bf.to_string());
    }
}
