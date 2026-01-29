use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeIterExt as _, RelCharOffset};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{BLOCK_SPACING, INDENT};

impl<'ast> Editor {
    pub fn text_format_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_block_quote(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut result = 0.;

        let first_line_idx = self.node_first_line_idx(node);
        let any_children = node.children().next().is_some();
        if any_children {
            result += self.block_children_height(node)
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let line_content = self.line_content(node, line);

                if line_idx != first_line_idx {
                    result += BLOCK_SPACING;
                }
                result += self.height_section(
                    &mut Wrap::new(self.width(node)),
                    line_content,
                    self.text_format_syntax(node),
                );
            }
        }

        result
    }

    pub fn show_block_quote(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let height = self.height(node);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.theme.bg().neutral_tertiary),
        );

        top_left.x += annotation_space.width();

        let first_line_idx = self.node_first_line_idx(node);
        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let line_content = self.line_content(node, line);

                if line_idx != first_line_idx {
                    top_left.y += BLOCK_SPACING;
                }

                let mut wrap = Wrap::new(self.width(node));
                self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    line_content,
                    self.text_format_syntax(node),
                    false,
                );
                top_left.y += wrap.height();
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            }
        }
    }

    pub fn compute_bounds_block_quote(&mut self, node: &'ast AstNode<'ast>) {
        // Push bounds for line prefix (vertical line annotation)
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            self.bounds
                .paragraphs
                .push(self.line_own_prefix(node, line));
        }

        // Handle children or remaining lines
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let line_content = self.line_content(node, line);
                self.bounds.paragraphs.push(line_content);
                self.bounds.inline_paragraphs.push(line_content);
            }
        }
    }

    // This routine is standard-/reference-complexity, as the prefix len is
    // line-by-line (unlike list items) and block quotes contain blocks
    // including other block quotes. Most of the fundamental behavior with line
    // prefix lengths can be observed with block quotes alone.
    //
    // This implementation does benefit from the simplicity of the node - there
    // are only 8 cases.
    pub fn own_prefix_len_block_quote(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> Option<RelCharOffset> {
        let node_line = self.node_line(node, line);
        let mut result = 0.into();

        // "A block quote marker consists of 0-3 spaces of initial indent, plus
        // (a) the character > together with a following space, or (b) a single
        // character > not followed by a space."
        //
        // "If a string of lines Ls constitute a sequence of blocks Bs, then the
        // result of prepending a block quote marker to the beginning of each
        // line in Ls is a block quote containing Bs."
        let text = &self.buffer[node_line];
        if text.starts_with("   > ") {
            result += 5;
        } else if text.starts_with("   >") || text.starts_with("  > ") {
            result += 4;
        } else if text.starts_with("  >") || text.starts_with(" > ") {
            result += 3;
        } else if text.starts_with(" >") || text.starts_with("> ") {
            result += 2;
        } else if text.starts_with(">") {
            result += 1;
        } else {
            // "If a string of lines Ls constitute a block quote with contents Bs,
            // then the result of deleting the initial block quote marker from one
            // or more lines in which the next non-whitespace character after the
            // block quote marker is paragraph continuation text is a block quote
            // with Bs as its content."
            return None;
        }

        Some(result)
    }
}
