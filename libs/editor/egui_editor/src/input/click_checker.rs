use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::Buffer;
use crate::element::{Element, ItemType};
use crate::galleys::Galleys;
use crate::input::mutation::pos_to_char_offset;
use crate::layouts::Annotation;
use crate::offset_types::RangeExt;
use egui::{Pos2, Rect};

pub trait ClickChecker {
    fn ui(&self, pos: Pos2) -> bool; // was the click even in the ui?
    fn checkbox(&self, pos: Pos2) -> Option<usize>; // returns galley index of checkbox
    fn link(&self, pos: Pos2) -> Option<String>; // returns url to open
}

pub struct EditorClickChecker<'a> {
    pub ui_rect: Rect,
    pub galleys: &'a Galleys,
    pub buffer: &'a Buffer,
    pub ast: &'a Ast,
    pub appearance: &'a Appearance,
}

impl<'a> ClickChecker for &'a EditorClickChecker<'a> {
    fn ui(&self, pos: Pos2) -> bool {
        self.ui_rect.contains(pos)
    }

    fn checkbox(&self, pos: Pos2) -> Option<usize> {
        for (galley_idx, galley) in self.galleys.galleys.iter().enumerate() {
            if let Some(Annotation::Item(ItemType::Todo(_), ..)) = galley.annotation {
                if galley.checkbox_bounds(self.appearance).contains(pos) {
                    return Some(galley_idx);
                }
            }
        }
        None
    }

    fn link(&self, pos: Pos2) -> Option<String> {
        let offset = pos_to_char_offset(pos, self.galleys, &self.buffer.current.segs);
        for ast_node in &self.ast.nodes {
            if let Element::Link(_, url, _) = &ast_node.element {
                if ast_node.range.contains(offset) {
                    return Some(url.to_string());
                }
            }
        }
        None
    }
}
