use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::Buffer;
use crate::element::{Element, ItemType};
use crate::galleys::Galleys;
use crate::input::mutation;
use crate::layouts::Annotation;
use crate::offset_types::RangeExt;
use egui::{Pos2, Rect};

pub trait ClickChecker {
    fn ui(&self, pos: Pos2) -> bool; // was the click even in the ui?
    fn text(&self, pos: Pos2) -> Option<usize>; // returns galley index
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

    fn text(&self, pos: Pos2) -> Option<usize> {
        for (galley_idx, galley) in self.galleys.galleys.iter().enumerate() {
            if galley.galley_location.contains(pos) {
                // galleys stretch across the screen, so we need to check if we're to the right of the text
                let offset =
                    mutation::pos_to_char_offset(pos, self.galleys, &self.buffer.current.segs);
                let line_end_offset = offset.advance_to_line_bound(false, self.galleys);
                let (_, egui_cursor) = self
                    .galleys
                    .galley_and_cursor_by_char_offset(line_end_offset);
                let end_pos_x =
                    galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x;
                let tolerance = 10.0;
                return if end_pos_x + tolerance > pos.x { Some(galley_idx) } else { None };
            }
        }
        None
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
        self.text(pos)?;
        let offset = mutation::pos_to_char_offset(pos, self.galleys, &self.buffer.current.segs);
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
