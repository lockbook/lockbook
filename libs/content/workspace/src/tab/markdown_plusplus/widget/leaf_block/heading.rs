use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, ROW_HEIGHT, ROW_SPACING},
    MarkdownPlusPlus,
};

// todo:
// * account for whitespace trimming (try a setext heading with trailing whitespace to see what I mean)

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_heading(&self, parent: &AstNode<'_>, level: u8) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                size: match level {
                    6 => 16.,
                    5 => 19.,
                    4 => 22.,
                    3 => 25.,
                    2 => 28.,
                    _ => 32.,
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn height_heading(
        &self, node: &'ast AstNode<'ast>, width: f32, level: u8, setext: bool,
    ) -> f32 {
        let mut wrap = WrapContext::new(width);
        let range = self.sourcepos_to_range(node.data.borrow().sourcepos);
        let any_children = node.children().next().is_some();

        if !setext {
            // https://github.github.com/gfm/#atx-headings
            let prefix_range = if any_children {
                let first_child = node.children().next().unwrap();
                let first_child_range =
                    self.sourcepos_to_range(first_child.data.borrow().sourcepos);
                (range.start(), first_child_range.start())
            } else {
                range
            };

            wrap.offset += self.span_text_line(&wrap, prefix_range, self.text_format_syntax(node));
        }

        wrap.offset += self.inline_children_span(node, &wrap);

        if setext {
            // https://github.github.com/gfm/#setext-headings
            let postfix_range = if any_children {
                let last_child = node.children().last().unwrap();
                let last_child_range = self.sourcepos_to_range(last_child.data.borrow().sourcepos);
                (last_child_range.end() + 1, range.end()) // skip the newline
            } else {
                range
            };

            wrap.offset = wrap.line_end();

            wrap.offset += self.span_text_line(&wrap, postfix_range, self.text_format_syntax(node));
        }

        let text_height = {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };

        text_height + if level == 1 { ROW_HEIGHT } else { 0. }
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
        level: u8, setext: bool,
    ) {
        let mut wrap = WrapContext::new(width);
        let range = self.sourcepos_to_range(node.data.borrow().sourcepos);
        let any_children = node.children().next().is_some();

        if !setext {
            // https://github.github.com/gfm/#atx-headings
            let prefix_range = if any_children {
                let first_child = node.children().next().unwrap();
                let first_child_range =
                    self.sourcepos_to_range(first_child.data.borrow().sourcepos);
                (range.start(), first_child_range.start())
            } else {
                range
            };

            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                prefix_range,
                self.row_height(node),
                self.text_format_syntax(node),
                false,
            );

            self.bounds.paragraphs.push(prefix_range);
        }

        if any_children {
            self.show_inline_children(ui, node, top_left, &mut wrap);

            let first_child = node.children().next().unwrap();
            let first_child_range = self.sourcepos_to_range(first_child.data.borrow().sourcepos);
            let last_child = node.children().last().unwrap();
            let last_child_range = self.sourcepos_to_range(last_child.data.borrow().sourcepos);
            let content_range = (first_child_range.start(), last_child_range.end());
            self.bounds.paragraphs.push(content_range);
        }

        if setext {
            // https://github.github.com/gfm/#setext-headings
            let postfix_range = if any_children {
                let last_child = node.children().last().unwrap();
                let last_child_range = self.sourcepos_to_range(last_child.data.borrow().sourcepos);
                (last_child_range.end() + 1, range.end()) // skip the newline
            } else {
                range
            };

            wrap.offset = wrap.line_end();

            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                postfix_range,
                self.row_height(node),
                self.text_format_syntax(node),
                false,
            );
            self.bounds.paragraphs.push(postfix_range);
        }

        top_left.y += {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };

        if level == 1 {
            let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

            ui.painter().hline(
                line_break_rect.x_range(),
                line_break_rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );
        }
    }
}
