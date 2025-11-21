use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_editor::widget::INDENT;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::{Editor, Event};

impl<'ast> Editor {
    pub fn height_task_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_task_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        maybe_check: Option<char>,
    ) {
        {
            let ui = &mut self.node_ui(ui, node, top_left);
            let mut checked = maybe_check.is_some();

            let icon_width = ui.style_mut().spacing.icon_width;
            ui.style_mut().spacing.icon_width = 18.;
            let scale_factor = ui.style_mut().spacing.icon_width / icon_width;
            ui.style_mut().spacing.icon_width_inner *= scale_factor;

            ui.checkbox(&mut checked, "");
            if checked != maybe_check.is_some() {
                let check_offset = self.check_offset(node);
                let check = if checked { 'x' } else { ' ' };

                self.event.internal_events.push(Event::Replace {
                    region: (check_offset, check_offset + 1).into(),
                    text: check.into(),
                    advance_cursor: false,
                });
            }
        }

        top_left.x += INDENT;

        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            let mut wrap = Wrap::new(self.width(node));
            self.show_section(
                ui,
                top_left,
                &mut wrap,
                line_content,
                self.text_format_document(),
                false,
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub fn own_prefix_len_task_item(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> Option<RelCharOffset> {
        let node_line = self.node_line(node, line);
        let mut result = 0.into();

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let indentation = {
            let first_line = self.node_first_line(node);
            let node_line = self.node_line(node, first_line);

            let text = &self.buffer[(node_line.start(), node_line.end())];
            if text.starts_with("   ") {
                "   ".len()
            } else if text.starts_with("  ") {
                "  ".len()
            } else if text.starts_with(" ") {
                " ".len()
            } else {
                0
            }
        };
        let marker_width_including_spaces: usize = {
            // task items don't have a NodeList so we have to do this ourselves
            let first_line = self.node_first_line(node);

            // "   - [ ]   item"
            // indentation + marker + spaces + content
            let node_line = self.node_line(node, first_line);

            // spaces + content
            let marker_width = 5;
            let text =
                &self.buffer[(node_line.start() + indentation + marker_width, node_line.end())];
            marker_width
                + if text.starts_with("   ") {
                    "   ".len()
                } else if text.starts_with("  ") {
                    "  ".len()
                } else if text.starts_with(" ") {
                    " ".len()
                } else {
                    0
                }
        };
        if line == self.node_first_line(node) {
            result += indentation;

            // "If a sequence of lines Ls constitute a sequence of blocks Bs starting
            // with a non-whitespace character, and M is a list marker of width W
            // followed by 1 ≤ N ≤ 4 spaces, then the result of prepending M and the
            // following spaces to the first line of Ls, and indenting subsequent lines
            // of Ls by W + N spaces, is a list item with Bs as its contents."
            //
            // "If a sequence of lines Ls starting with a single blank line
            // constitute a (possibly empty) sequence of blocks Bs, not separated
            // from each other by more than one blank line, and M is a list marker
            // of width W, then the result of prepending M to the first line of Ls,
            // and indenting subsequent lines of Ls by W + 1 spaces, is a list item
            // with Bs as its contents."
            result += marker_width_including_spaces;
        } else {
            // "If a string of lines Ls constitute a list item with contents Bs, then
            // the result of deleting some or all of the indentation from one or
            // more lines in which the next non-whitespace character after the
            // indentation is paragraph continuation text is a list item with the
            // same contents and attributes."
            //
            // "If a line is empty, then it need not be indented."
            let text = &self.buffer[node_line];
            for i in 0..(marker_width_including_spaces + indentation) {
                if text.starts_with(&" ".repeat(marker_width_including_spaces + indentation - i)) {
                    result += marker_width_including_spaces + indentation - i;
                    break;
                }
            }
        }

        Some(result)
    }

    pub fn compute_bounds_task_item(&mut self, node: &'ast AstNode<'ast>) {
        // Push bounds for line prefix
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            self.bounds
                .paragraphs
                .push(self.line_own_prefix(node, line));
        }

        // Handle children or line content
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
            self.bounds.paragraphs.push(line_content);
            self.bounds.inline_paragraphs.push(line_content);
        }
    }

    fn check_offset(&self, node: &'ast AstNode<'ast>) -> DocCharOffset {
        let line = self.node_first_line(node);
        let node_line = self.node_line(node, line);

        let text = &self.buffer[(node_line.start(), node_line.end())];
        let indentation = if text.starts_with("   ") {
            "   ".len()
        } else if text.starts_with("  ") {
            "  ".len()
        } else if text.starts_with(" ") {
            " ".len()
        } else {
            0
        };

        node_line.start() + indentation + 3
    }
}
