use std::collections::VecDeque;

use minidom::Element;

use super::util;

const MAX_UNDOS: usize = 100;

pub struct Buffer {
    pub current: Element,
    undo: VecDeque<Event>,
    redo: Vec<Event>,
}

#[derive(Clone)]
pub enum Event {
    UpdateOpacity(UpdateOpacity),
    InsertElement(InsertElement),
    DeleteElement(DeleteElement),
}

#[derive(Clone)]

struct UpdateOpacity {
    id: String,
    opacity: String,
    prev_opacity: String,
}

#[derive(Clone)]

pub struct DeleteElement {
    pub id: String,
    pub element: Element,
}

#[derive(Clone)]

pub struct InsertElement {
    pub id: String,
    pub element: Element,
}

impl Buffer {
    pub fn new(root: Element) -> Self {
        Buffer { current: root, undo: VecDeque::default(), redo: vec![] }
    }

    pub fn apply(&mut self, event: Event) {
        if !self.redo.is_empty() {
            self.redo = vec![];
        }
        self.apply_event(&event);

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
            Event::InsertElement(payload) => {
                if let Some(node) = util::node_by_id(&mut self.current, payload.id.to_string()) {
                    node.set_attr("d", payload.element.attr("d").unwrap());
                } else {
                    self.current.append_child(payload.element.clone());
                }
            }
            Event::DeleteElement(payload) => {
                if let Some(node) = util::node_by_id(&mut self.current, payload.id.to_string()) {
                    // todo: figure out a less hacky way, to detach a node (not just paths) from the tree
                    node.set_attr("d", "");
                }
            }
        };
    }

    pub fn undo(&mut self) {
        if self.undo.is_empty() {
            return;
        }

        if let Some(mut undo_event) = self.undo.pop_back().to_owned() {
            match undo_event {
                Event::UpdateOpacity(ref mut payload) => {
                    std::mem::swap(&mut payload.opacity, &mut payload.prev_opacity);
                }
                Event::InsertElement(payload) => {
                    undo_event = Event::DeleteElement(DeleteElement {
                        id: payload.id,
                        element: payload.element,
                    });
                }
                Event::DeleteElement(payload) => {
                    undo_event = Event::InsertElement(InsertElement {
                        id: payload.id,
                        element: payload.element,
                    });
                }
            }
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

        if let Some(mut redo_event) = self.redo.pop().to_owned() {
            match redo_event {
                Event::UpdateOpacity(ref mut payload) => {
                    std::mem::swap(&mut payload.opacity, &mut payload.prev_opacity);
                }
                Event::InsertElement(payload) => {
                    redo_event = Event::DeleteElement(DeleteElement {
                        id: payload.id,
                        element: payload.element,
                    });
                }
                Event::DeleteElement(payload) => {
                    redo_event = Event::InsertElement(InsertElement {
                        id: payload.id,
                        element: payload.element,
                    });
                }
            }
            self.apply_event(&redo_event);
            self.undo.push_back(redo_event);
        }
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

#[cfg(test)]
mod test {
    use minidom::Element;

    use crate::account::tabs::svg_editor::{history::InsertElement, util};

    use super::{Buffer, UpdateOpacity};

    #[test]
    fn opacity_change() {
        let test_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" >
            <path id="1" d="M 10 80 Q 95 10 180 80" stroke="black" fill="transparent"/>
        </svg>
        "#;

        let root: Element = test_svg.parse().unwrap();

        let mut bf = Buffer::new(root);

        bf.apply(super::Event::UpdateOpacity(UpdateOpacity {
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
            <path id="1" d="M 10 80 Q 95 10 180 80" stroke="black" fill="transparent"/>
        </svg>
        "#;

        let root: Element = test_svg.parse().unwrap();

        let mut bf = Buffer::new(root);
        let child = Element::builder("path", "")
            .attr("fill", "none")
            .attr("id", "2")
            .attr("d", "M mars.x mars.y")
            .build();

        bf.apply(super::Event::InsertElement(InsertElement {
            id: "2".to_string(),
            element: child,
        }));

        bf.undo();

        // let mut buffer = Vec::new();
        // bf.current.write_to(&mut buffer).unwrap();
        // println!(
        //     "{}",
        //     std::str::from_utf8(&buffer)
        //         .unwrap()
        //         .replace("xmlns='' ", "")
        // );

        bf.redo();

        let mut buffer = Vec::new();
        bf.current.write_to(&mut buffer).unwrap();
        println!(
            "{}",
            std::str::from_utf8(&buffer)
                .unwrap()
                .replace("xmlns='' ", "")
        );
    }
}
