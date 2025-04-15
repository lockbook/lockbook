use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, ROW_HEIGHT, ROW_SPACING},
    MarkdownPlusPlus,
};

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
        if setext {
            self.height_setext_heading(node, width, level)
        } else {
            self.height_atx_heading(node, width, level)
        }
    }

    // https://github.github.com/gfm/#setext-headings
    fn height_setext_heading(&self, node: &'ast AstNode<'ast>, width: f32, level: u8) -> f32 {
        let mut wrap = WrapContext::new(width);

        {
            let postfix_range = self
                .postfix_range(node)
                .expect("setext headings cannot be empty");
            wrap.offset += self.inline_children_span(node, &wrap);

            if self.node_intersects_selection(node) {
                for postfix_line_range in self.range_lines(postfix_range) {
                    wrap.offset += self.span_text_line(
                        &wrap,
                        postfix_line_range,
                        self.text_format_syntax(node),
                    );
                    wrap.offset = wrap.line_end();
                }
            }
        }

        let text_height = {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };

        text_height + if level == 1 { ROW_HEIGHT } else { 0. }
    }

    // https://github.github.com/gfm/#atx-headings
    fn height_atx_heading(&self, node: &'ast AstNode<'ast>, width: f32, level: u8) -> f32 {
        let mut wrap = WrapContext::new(width);

        if let Some((prefix_range, _, postfix_range)) = self.prefix_infix_postfix_ranges(node) {
            if self.node_intersects_selection(node) {
                wrap.offset +=
                    self.span_text_line(&wrap, prefix_range, self.text_format_syntax(node));
            }

            wrap.offset += self.inline_children_span(node, &wrap);

            if self.node_intersects_selection(node) && !postfix_range.is_empty() {
                wrap.offset +=
                    self.span_text_line(&wrap, postfix_range, self.text_format_syntax(node));
            }
        } else {
            // heading is empty
            let range = self.sourcepos_to_range(node.data.borrow().sourcepos);
            if self.node_intersects_selection(node) {
                wrap.offset += self.span_text_line(&wrap, range, self.text_format_syntax(node));
            }
            wrap.offset = wrap.line_end();
        }

        let text_height = {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };

        text_height + if level == 1 { ROW_HEIGHT } else { 0. }
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32, level: u8,
        setext: bool,
    ) {
        if setext {
            self.show_setext_heading(ui, node, top_left, width, level);
        } else {
            self.show_atx_heading(ui, node, top_left, width, level);
        }
    }

    // https://github.github.com/gfm/#setext-headings
    fn show_setext_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
        level: u8,
    ) {
        let mut wrap = WrapContext::new(width);

        {
            let (_, infix_range, postfix_range) = self
                .prefix_infix_postfix_ranges(node)
                .expect("setext headings cannot be empty");
            self.show_inline_children(ui, node, top_left, &mut wrap);
            self.bounds.paragraphs.push(infix_range);

            for postfix_line_range in self.range_lines(postfix_range) {
                if self.node_intersects_selection(node) {
                    self.show_text_line(
                        ui,
                        top_left,
                        &mut wrap,
                        postfix_line_range,
                        self.row_height(node),
                        self.text_format_syntax(node),
                        false,
                    );
                    wrap.offset = wrap.line_end();
                }
                if !postfix_line_range.is_empty() {
                    self.bounds.paragraphs.push(postfix_line_range);
                }
            }
        }

        top_left.y += {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
    }

    // https://github.github.com/gfm/#atx-headings
    fn show_atx_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
        level: u8,
    ) {
        let mut wrap = WrapContext::new(width);

        if let Some((prefix_range, _, postfix_range)) = self.prefix_infix_postfix_ranges(node) {
            if self.node_intersects_selection(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    prefix_range,
                    self.row_height(node),
                    self.text_format_syntax(node),
                    false,
                );
            }
            self.bounds.paragraphs.push(prefix_range);

            if let Some(infix_range) = self.infix_range(node) {
                self.show_inline_children(ui, node, top_left, &mut wrap);
                self.bounds.paragraphs.push(infix_range);
            }

            if self.node_intersects_selection(node) && !postfix_range.is_empty() {
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
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            let range = self.sourcepos_to_range(node.data.borrow().sourcepos);
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                range,
                self.row_height(node),
                self.text_format_syntax(node),
                false,
            );
            self.bounds.paragraphs.push(range);
        }

        top_left.y += {
            let rows = (wrap.offset / width).ceil();
            rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
        };
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
    }

    fn show_heading_rule(&mut self, ui: &mut Ui, top_left: Pos2, width: f32) {
        let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

        ui.painter().hline(
            line_break_rect.x_range(),
            line_break_rect.center().y,
            Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
        );
    }
}
