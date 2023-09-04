use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::bounds::Bounds;
use crate::buffer::Buffer;
use crate::galleys::Galleys;
use crate::input::canonical::Bound;
use crate::input::mutation;
use crate::layouts::Annotation;
use crate::offset_types::{DocCharOffset, RangeExt};
use crate::style::{InlineNode, ListItem, MarkdownNode};
use egui::{Pos2, Rect};

pub trait ClickChecker {
    fn ui(&self, pos: Pos2) -> bool; // was the click even in the ui?
    fn text(&self, pos: Pos2) -> Option<usize>; // returns galley index
    fn checkbox(&self, pos: Pos2, touch_mode: bool) -> Option<usize>; // returns galley index of checkbox
    fn link(&self, pos: Pos2) -> Option<String>; // returns url to open
    fn pos_to_char_offset(&self, pos: Pos2) -> DocCharOffset; // converts pos to char offset
}

pub struct EditorClickChecker<'a> {
    pub ui_rect: Rect,
    pub galleys: &'a Galleys,
    pub bounds: &'a Bounds,
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
                let offset = mutation::pos_to_char_offset(
                    pos,
                    self.galleys,
                    &self.buffer.current.segs,
                    &self.bounds.text,
                );
                let line_end_offset = offset.advance_to_bound(Bound::Line, false, self.bounds);
                let (_, egui_cursor) = self
                    .galleys
                    .galley_and_cursor_by_char_offset(line_end_offset, &self.bounds.text);
                let end_pos_x =
                    galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x;
                let tolerance = 10.0;
                return if end_pos_x + tolerance > pos.x { Some(galley_idx) } else { None };
            }
        }
        None
    }

    fn checkbox(&self, pos: Pos2, touch_mode: bool) -> Option<usize> {
        for (galley_idx, galley) in self.galleys.galleys.iter().enumerate() {
            if let Some(Annotation::Item(ListItem::Todo(_), ..)) = galley.annotation {
                if galley
                    .checkbox_bounds(touch_mode, self.appearance)
                    .contains(pos)
                {
                    return Some(galley_idx);
                }
            }
        }
        None
    }

    fn link(&self, pos: Pos2) -> Option<String> {
        self.text(pos)?;
        let offset = mutation::pos_to_char_offset(
            pos,
            self.galleys,
            &self.buffer.current.segs,
            &self.bounds.text,
        );
        for ast_node in &self.ast.nodes {
            if let MarkdownNode::Inline(InlineNode::Link(_, url, _)) = &ast_node.node_type {
                if ast_node.range.contains_inclusive(offset) {
                    return Some(url.to_string());
                }
            }
        }
        None
    }

    fn pos_to_char_offset(&self, pos: Pos2) -> DocCharOffset {
        mutation::pos_to_char_offset(
            pos,
            self.galleys,
            &self.buffer.current.segs,
            &self.bounds.text,
        )
    }
}
