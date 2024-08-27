use crate::tab::markdown_editor::appearance::Appearance;
use crate::tab::markdown_editor::ast::Ast;
use crate::tab::markdown_editor::bounds::BoundExt as _;
use crate::tab::markdown_editor::bounds::Bounds;
use crate::tab::markdown_editor::buffer::Buffer;
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input::mutation;
use crate::tab::markdown_editor::input::Bound;
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::style::{InlineNode, ListItem, MarkdownNode};
use egui::{Pos2, Rect};
use lb_rs::text::offset_types::{DocCharOffset, RangeExt};

pub trait ClickChecker {
    fn ui(self, pos: Pos2) -> bool; // was the click even in the ui?
    fn text(self, pos: Pos2) -> Option<usize>; // returns galley index
    fn checkbox(self, pos: Pos2, touch_mode: bool) -> Option<usize>; // returns galley index of checkbox
    fn link(self, pos: Pos2) -> Option<String>; // returns url to open
    fn pos_to_char_offset(self, pos: Pos2) -> DocCharOffset; // converts pos to char offset
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
    fn ui(self, pos: Pos2) -> bool {
        self.ui_rect.contains(pos)
    }

    fn text(self, pos: Pos2) -> Option<usize> {
        for (galley_idx, galley) in self.galleys.galleys.iter().enumerate() {
            if galley.galley_location.contains(pos) {
                // galleys stretch across the screen, so we need to check if we're to the right of the text
                // use a tolerance of 10.0 for x and a tolerance of one line for y (supports noncapture when pointer is over a code block)
                let offset = mutation::pos_to_char_offset(
                    pos,
                    self.galleys,
                    &self.buffer.current.segs,
                    &self.bounds.text,
                );

                let prev_line_end_pos_x = {
                    let line_start_offset = offset
                        .advance_to_bound(Bound::Line, true, self.bounds)
                        .advance_to_next_bound(Bound::Line, true, self.bounds);
                    let line_end_offset =
                        line_start_offset.advance_to_bound(Bound::Line, false, self.bounds);
                    let (_, egui_cursor) = self
                        .galleys
                        .galley_and_cursor_by_char_offset(line_end_offset, &self.bounds.text);
                    galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
                };
                let curr_line_end_pos_x = {
                    let line_end_offset = offset.advance_to_bound(Bound::Line, false, self.bounds);
                    let (_, egui_cursor) = self
                        .galleys
                        .galley_and_cursor_by_char_offset(line_end_offset, &self.bounds.text);
                    galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
                };
                let next_line_end_pos_x = {
                    let line_end_offset = offset
                        .advance_to_bound(Bound::Line, false, self.bounds)
                        .advance_to_next_bound(Bound::Line, false, self.bounds);
                    let (_, egui_cursor) = self
                        .galleys
                        .galley_and_cursor_by_char_offset(line_end_offset, &self.bounds.text);
                    galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
                };

                let max_pos_x = prev_line_end_pos_x
                    .max(curr_line_end_pos_x)
                    .max(next_line_end_pos_x);
                let tolerance = 10.0;
                return if max_pos_x + tolerance > pos.x { Some(galley_idx) } else { None };
            }
        }
        None
    }

    fn checkbox(self, pos: Pos2, touch_mode: bool) -> Option<usize> {
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

    fn link(self, pos: Pos2) -> Option<String> {
        self.text(pos)?;
        let offset = mutation::pos_to_char_offset(
            pos,
            self.galleys,
            &self.buffer.current.segs,
            &self.bounds.text,
        );

        // todo: binary search
        for ast_node in &self.ast.nodes {
            if let MarkdownNode::Inline(InlineNode::Link(_, url, _)) = &ast_node.node_type {
                if ast_node.range.contains_inclusive(offset) {
                    return Some(url.to_string());
                }
            }
        }
        for plaintext_link in &self.bounds.links {
            if plaintext_link.contains_inclusive(offset) {
                return Some(self.buffer[*plaintext_link].to_string());
            }
        }

        None
    }

    fn pos_to_char_offset(self, pos: Pos2) -> DocCharOffset {
        mutation::pos_to_char_offset(
            pos,
            self.galleys,
            &self.buffer.current.segs,
            &self.bounds.text,
        )
    }
}
