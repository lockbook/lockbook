use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_SPACING, ROW_HEIGHT, ROW_SPACING},
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

    pub fn height_heading(&self, node: &'ast AstNode<'ast>, level: u8, setext: bool) -> f32 {
        let text_height =
            if setext { self.height_setext_heading(node) } else { self.height_atx_heading(node) };
        text_height + if level == 1 { BLOCK_SPACING } else { 0. }
    }

    // https://github.github.com/gfm/#setext-headings
    fn height_setext_heading(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let reveal = self.reveal_setext_syntax(node);
        let mut result = 0.;

        let last_line_idx = self.node_lines(node).iter().count() - 1;
        for (line_idx, line) in self.node_lines(node).iter().enumerate() {
            let line = self.bounds.source_lines[line];

            let parent = node.parent().unwrap();
            let parent_prefix_len = self.line_prefix_len(parent, line);
            let node_line = (line.start() + parent_prefix_len, line.end());

            if line_idx < last_line_idx {
                // non-underline content
                self.height_setext_heading_line(node, line, reveal);

                result += self.height_setext_heading_line(node, line, reveal);
                result += ROW_SPACING;
            } else {
                // setext heading underline
                if reveal {
                    let mut wrap = Wrap::new(width);
                    wrap.row_height = self.row_height(node);
                    wrap.offset =
                        self.span_text_line(&wrap, node_line, self.text_format_syntax(node));

                    result += self.height_setext_underline(node, line);
                    result += ROW_SPACING;
                }
            }
        }

        result - ROW_SPACING
    }

    pub fn height_setext_heading_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset), reveal: bool,
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        if let Some((indentation, prefix, _, postfix_whitespace, _)) = self.line_ranges(node, line)
        {
            if reveal {
                wrap.offset +=
                    self.span_text_line(&wrap, indentation, self.text_format_syntax(node));
                wrap.offset += self.span_text_line(&wrap, prefix, self.text_format_syntax(node));
            }
            for child in &self.children_in_line(node, line) {
                wrap.offset += self.span(child, &wrap);
            }
            if reveal {
                wrap.offset +=
                    self.span_text_line(&wrap, postfix_whitespace, self.text_format_syntax(node));
            }
        } else {
            unreachable!("setext headings never have empty lines");
        }

        wrap.height()
    }

    pub fn height_setext_underline(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let node_line = (line.start() + parent_prefix_len, line.end());

        wrap.offset += self.span_text_line(&wrap, node_line, self.text_format_syntax(node));

        wrap.height()
    }

    pub fn reveal_setext_syntax(&self, node: &'ast AstNode<'ast>) -> bool {
        // reveal syntax even if the cursor is in the indentation before the node
        let mut reveal = false;
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];

            if line.intersects(&self.buffer.current.selection, true) {
                reveal = true;
                break;
            }
        }
        reveal
    }

    // https://github.github.com/gfm/#atx-headings
    fn height_atx_heading(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        let parent = node.parent().unwrap();
        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let node_line = (line.start() + parent_prefix_len, line.end());

        let reveal = line.intersects(&self.buffer.current.selection, true);

        if let Some((indentation, prefix_range, _, postfix_range, _)) = self.line_ranges(node, line)
        {
            if reveal {
                if !indentation.is_empty() {
                    wrap.offset +=
                        self.span_text_line(&wrap, indentation, self.text_format_syntax(node));
                }
                wrap.offset +=
                    self.span_text_line(&wrap, prefix_range, self.text_format_syntax(node));
            }

            if self.infix_range(node).is_some() {
                wrap.offset += self.inline_children_span(node, &wrap);
            }

            if reveal && !postfix_range.is_empty() {
                wrap.offset +=
                    self.span_text_line(&wrap, postfix_range, self.text_format_syntax(node));
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            wrap.offset += self.span_text_line(&wrap, node_line, self.text_format_syntax(node));
        }

        wrap.height()
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, level: u8, setext: bool,
    ) {
        if setext {
            self.show_setext_heading(ui, node, top_left, level);
        } else {
            self.show_atx_heading(ui, node, top_left, level);
        }
    }

    // https://github.github.com/gfm/#setext-headings
    fn show_setext_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) {
        let width = self.width(node);
        let reveal = self.reveal_setext_syntax(node);

        let last_line_idx = self.node_lines(node).iter().count() - 1;
        for (line_idx, line) in self.node_lines(node).iter().enumerate() {
            let line = self.bounds.source_lines[line];

            let parent = node.parent().unwrap();
            let parent_prefix_len = self.line_prefix_len(parent, line);
            let node_line = (line.start() + parent_prefix_len, line.end());

            if line_idx < last_line_idx {
                // non-underline content
                let last_shown_line = if reveal { false } else { line_idx == last_line_idx - 1 };
                self.show_setext_heading_line(
                    ui,
                    node,
                    top_left,
                    line,
                    level,
                    reveal,
                    last_shown_line,
                );

                top_left.y += self.height_setext_heading_line(node, line, reveal);
                top_left.y += ROW_SPACING;
            } else {
                // setext heading underline
                if reveal {
                    let mut wrap = Wrap::new(width);
                    wrap.row_height = self.row_height(node);

                    let line_height = self.height_setext_underline(node, line);
                    self.show_line_prefix(
                        ui,
                        parent,
                        line,
                        top_left,
                        line_height + if level == 1 { BLOCK_SPACING } else { 0. },
                        wrap.row_height,
                    );

                    self.show_text_line(
                        ui,
                        top_left,
                        &mut wrap,
                        node_line,
                        self.text_format_syntax(node),
                        false,
                    );

                    top_left.y += line_height;
                    top_left.y += ROW_SPACING;
                }

                self.bounds.paragraphs.push(node_line);
            }
        }

        top_left.y -= ROW_SPACING;
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_setext_heading_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        line: (DocCharOffset, DocCharOffset), level: u8, reveal: bool, last_shown_line: bool,
    ) {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        let parent = node.parent().unwrap();
        let line_height = self.height_setext_heading_line(node, line, reveal);
        self.show_line_prefix(
            ui,
            parent,
            line,
            top_left,
            line_height + if level == 1 && last_shown_line { BLOCK_SPACING } else { 0. },
            wrap.row_height,
        );

        if let Some((indentation, prefix, children, postfix_whitespace, _)) =
            self.line_ranges(node, line)
        {
            if !indentation.is_empty() {
                self.bounds.paragraphs.push(indentation);
            }
            if !prefix.is_empty() {
                self.bounds.paragraphs.push(prefix);
            }
            self.bounds.paragraphs.push(children);
            if !postfix_whitespace.is_empty() {
                self.bounds.paragraphs.push(postfix_whitespace);
            }

            if reveal {
                if !indentation.is_empty() {
                    self.show_text_line(
                        ui,
                        top_left,
                        &mut wrap,
                        indentation,
                        self.text_format_syntax(node),
                        false,
                    );
                }
                if !prefix.is_empty() {
                    self.show_text_line(
                        ui,
                        top_left,
                        &mut wrap,
                        prefix,
                        self.text_format_syntax(node),
                        false,
                    );
                }
            }
            for child in &self.children_in_line(node, line) {
                self.show_inline(ui, child, top_left, &mut wrap);
            }
            if reveal && !postfix_whitespace.is_empty() {
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix_whitespace,
                    self.text_format_syntax(node),
                    false,
                );
            }
        } else {
            unreachable!("setext headings never have empty lines");
        }
    }

    // https://github.github.com/gfm/#atx-headings
    fn show_atx_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        let parent = node.parent().unwrap();
        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let node_line = (line.start() + parent_prefix_len, line.end());

        let line_height = self.height_atx_heading(node);
        self.show_line_prefix(
            ui,
            parent,
            line,
            top_left,
            line_height + if level == 1 { BLOCK_SPACING } else { 0. },
            wrap.row_height,
        );

        let reveal = line.intersects(&self.buffer.current.selection, true);

        if let Some((indentation, prefix_range, _, postfix_range, _)) = self.line_ranges(node, line)
        {
            if reveal {
                if !indentation.is_empty() {
                    self.show_text_line(
                        ui,
                        top_left,
                        &mut wrap,
                        indentation,
                        self.text_format_syntax(node),
                        false,
                    );
                }
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    prefix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
            if !indentation.is_empty() {
                self.bounds.paragraphs.push(indentation);
            }
            self.bounds.paragraphs.push(prefix_range);

            if let Some(infix_range) = self.infix_range(node) {
                self.show_inline_children(ui, node, top_left, &mut wrap);
                self.bounds.paragraphs.push(infix_range);
            }

            if reveal && !postfix_range.is_empty() {
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
            if !postfix_range.is_empty() {
                self.bounds.paragraphs.push(postfix_range);
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_syntax(node),
                false,
            );
            self.bounds.paragraphs.push(node_line);
        }

        top_left.y += line_height;
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
    }

    fn show_heading_rule(&mut self, ui: &mut Ui, top_left: Pos2, width: f32) {
        let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, BLOCK_SPACING));

        ui.painter().hline(
            line_break_rect.x_range(),
            line_break_rect.center().y,
            Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
        );
    }
}
