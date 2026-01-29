use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt as _, RangeExt as _, RangeIterExt as _,
};

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::inline::html_inline::FOLD_TAG;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{BLOCK_SPACING, INDENT, ROW_SPACING};
use crate::tab::markdown_editor::{Editor, Event};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

impl<'ast> Editor {
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
        text_height + if level <= 2 { BLOCK_SPACING } else { 0. }
    }

    // https://github.github.com/gfm/#setext-headings
    fn height_setext_heading(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let reveal = self.reveal_setext_syntax(node);
        let mut result = 0.;

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            let node_line = self.node_line(node, line);

            if line_idx < last_line_idx {
                // non-underline content
                result += self.height_setext_heading_line(node, node_line, reveal);
                result += ROW_SPACING;
            } else {
                // setext heading underline
                if reveal {
                    let mut wrap = Wrap::new(width);
                    wrap.row_height = self.row_height(node);
                    wrap.offset =
                        self.span_section(&wrap, node_line, self.text_format_syntax(node));

                    result += wrap.height();
                    result += ROW_SPACING;
                }
            }
        }

        result - ROW_SPACING
    }

    pub fn height_setext_heading_line(
        &self, node: &'ast AstNode<'ast>, node_line: (DocCharOffset, DocCharOffset), reveal: bool,
    ) -> f32 {
        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        if let Some((indentation, prefix, _, postfix_whitespace, _)) =
            self.split_range(node, node_line)
        {
            if reveal {
                wrap.offset += self.span_section(&wrap, indentation, self.text_format_syntax(node));
                wrap.offset += self.span_section(&wrap, prefix, self.text_format_syntax(node));
            }
            wrap.offset += self.inline_children_span(node, &wrap, node_line);
            if reveal {
                wrap.offset += self.span_section(&wrap, postfix_whitespace, self.text_format(node));
            }
        } else {
            unreachable!("setext headings never have empty lines");
        }

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

        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let node_line = self.node_line(node, line);

        let reveal = line.intersects(&self.buffer.current.selection, true);

        if let Some((indentation, prefix_range, _, postfix_range, _)) =
            self.split_range(node, node_line)
        {
            if reveal {
                if !indentation.is_empty() {
                    wrap.offset +=
                        self.span_section(&wrap, indentation, self.text_format_syntax(node));
                }
                wrap.offset +=
                    self.span_section(&wrap, prefix_range, self.text_format_syntax(node));
            }

            if self.infix_range(node).is_some() {
                wrap.offset += self.inline_children_span(node, &wrap, node_line);
            }

            if reveal && !postfix_range.is_empty() {
                wrap.offset += self.span_section(&wrap, postfix_range, self.text_format(node));
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            wrap.offset += self.span_section(&wrap, node_line, self.text_format_syntax(node));
        }

        wrap.height()
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, level: u8, setext: bool,
    ) {
        let resp = if setext {
            self.show_setext_heading(ui, node, top_left, level)
        } else {
            self.show_atx_heading(ui, node, top_left, level)
        };

        // show/hide button (fold)
        // todo: factor (copied for list items)
        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);
        let annotation_size = Vec2 { x: INDENT, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        let fold_button_space = annotation_space.translate(Vec2::X * -INDENT);
        let fold_button_size = (self.row_height(node) * 0.6).min(INDENT / 2.);
        self.touch_consuming_rects.push(fold_button_space);

        // todo: proper hit-testing (this ignores anything covering the space)
        let show_fold_button = self.touch_mode
            || resp.hovered
            || fold_button_space.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()))
            || self.fold(node).is_some();
        if !show_fold_button {
            return;
        }

        if let Some(fold) = self.fold(node) {
            ui.allocate_ui_at_rect(fold_button_space, |ui| {
                let icon = Icon::CHEVRON_RIGHT
                    .size(fold_button_size)
                    .color(self.theme.fg().accent_secondary);
                if IconButton::new(icon)
                    .tooltip("Show Contents")
                    .show(ui)
                    .clicked()
                {
                    self.event.internal_events.push(Event::Replace {
                        region: self.node_range(fold).into(),
                        text: "".into(),
                        advance_cursor: false,
                    });
                }
            });
        } else if let Some(foldable) = self.foldable(node) {
            ui.allocate_ui_at_rect(fold_button_space, |ui| {
                let icon = Icon::CHEVRON_DOWN
                    .size(fold_button_size)
                    .color(self.theme.fg().neutral_quarternary);
                if IconButton::new(icon)
                    .tooltip("Hide Contents")
                    .show(ui)
                    .clicked()
                {
                    self.event.internal_events.push(Event::Replace {
                        region: self.node_range(foldable).end().into_range().into(),
                        text: FOLD_TAG.into(),
                        advance_cursor: false,
                    });
                }
            });
        }
    }

    // https://github.github.com/gfm/#setext-headings
    fn show_setext_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) -> Response {
        let mut resp = Default::default();

        let width = self.width(node);
        let reveal = self.reveal_setext_syntax(node);

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            let node_line = self.node_line(node, line);

            if line_idx < last_line_idx {
                // non-underline content
                resp |= self.show_setext_heading_line(ui, node, top_left, node_line, reveal);

                top_left.y += self.height_setext_heading_line(node, node_line, reveal);
                top_left.y += ROW_SPACING;
            } else {
                // setext heading underline
                if reveal {
                    let mut wrap = Wrap::new(width);
                    wrap.row_height = self.row_height(node);

                    self.show_section(
                        ui,
                        top_left,
                        &mut wrap,
                        node_line,
                        self.text_format_syntax(node),
                        false,
                    );

                    top_left.y += wrap.height();
                    top_left.y += ROW_SPACING;
                    self.bounds.wrap_lines.extend(wrap.row_ranges);
                }
            }
        }

        top_left.y -= ROW_SPACING;
        if level <= 2 {
            self.show_heading_rule(ui, top_left, width);
        }

        resp
    }

    #[allow(clippy::too_many_arguments)]
    fn show_setext_heading_line(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_line: (DocCharOffset, DocCharOffset), reveal: bool,
    ) -> Response {
        let mut resp = Default::default();

        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        if let Some((indentation, prefix, _children, postfix_whitespace, _)) =
            self.split_range(node, node_line)
        {
            if reveal {
                if !indentation.is_empty() {
                    self.show_section(
                        ui,
                        top_left,
                        &mut wrap,
                        indentation,
                        self.text_format_syntax(node),
                        false,
                    );
                }
                if !prefix.is_empty() {
                    self.show_section(
                        ui,
                        top_left,
                        &mut wrap,
                        prefix,
                        self.text_format_syntax(node),
                        false,
                    );
                }
            }
            self.show_inline_children(ui, node, top_left, &mut wrap, node_line);
            if reveal && !postfix_whitespace.is_empty() {
                resp |= self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix_whitespace,
                    self.text_format(node),
                    false,
                );
            }
        } else {
            unreachable!("setext headings never have empty lines");
        }

        resp
    }

    // https://github.github.com/gfm/#atx-headings
    fn show_atx_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) -> Response {
        let mut resp = Default::default();

        let width = self.width(node);
        let mut wrap = Wrap::new(width);
        wrap.row_height = self.row_height(node);

        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let node_line = self.node_line(node, line);

        let height = self.height_atx_heading(node);
        let reveal = line.intersects(&self.buffer.current.selection, true);

        if let Some((indentation, prefix_range, _, postfix_range, _)) =
            self.split_range(node, node_line)
        {
            if reveal {
                if !indentation.is_empty() {
                    resp |= self.show_section(
                        ui,
                        top_left,
                        &mut wrap,
                        indentation,
                        self.text_format_syntax(node),
                        false,
                    );
                }
                resp |= self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    prefix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
            if self.infix_range(node).is_some() {
                resp |= self.show_inline_children(ui, node, top_left, &mut wrap, node_line);
            }

            if reveal && !postfix_range.is_empty() {
                resp |= self.show_section(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix_range,
                    self.text_format(node),
                    false,
                );
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            resp |= self.show_section(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_syntax(node),
                false,
            );
        }

        top_left.y += height;
        self.bounds.wrap_lines.extend(wrap.row_ranges);
        if level <= 2 {
            self.show_heading_rule(ui, top_left, width);
        }

        resp
    }

    fn show_heading_rule(&mut self, ui: &mut Ui, top_left: Pos2, width: f32) {
        let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, BLOCK_SPACING));

        ui.painter().hline(
            line_break_rect.x_range(),
            line_break_rect.center().y,
            Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
        );
    }

    pub fn compute_bounds_heading(&mut self, node: &'ast AstNode<'ast>, _level: u8, setext: bool) {
        if setext {
            self.compute_bounds_setext_heading(node)
        } else {
            self.compute_bounds_atx_heading(node)
        }
    }

    fn compute_bounds_setext_heading(&mut self, node: &'ast AstNode<'ast>) {
        let reveal = self.reveal_setext_syntax(node);
        let last_line_idx = self.node_last_line_idx(node);

        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            if line_idx < last_line_idx {
                // non-underline content
                self.compute_bounds_setext_heading_line(node, line, reveal);
            } else {
                // underline
                let node_line = self.node_line(node, line);
                self.bounds.paragraphs.push(node_line);
            }
        }
    }

    fn compute_bounds_setext_heading_line(
        &mut self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset), reveal: bool,
    ) {
        let node_line = self.node_line(node, line);

        if reveal {
            let Some((indentation, prefix, children, postfix_whitespace, _)) =
                self.split_range(node, node_line)
            else {
                self.bounds.paragraphs.push(node_line);
                self.bounds.inline_paragraphs.push(node_line);
                return;
            };

            if !indentation.is_empty() {
                self.bounds.paragraphs.push(indentation);
            }
            if !prefix.is_empty() {
                self.bounds.paragraphs.push(prefix);
            }
            self.bounds.paragraphs.push(children);
            if !postfix_whitespace.is_empty() {
                self.bounds.paragraphs.push(postfix_whitespace);
                self.bounds
                    .inline_paragraphs
                    .push((children.start(), postfix_whitespace.end()));
            } else {
                self.bounds.inline_paragraphs.push(children);
            }
        } else {
            let Some((_indentation, _prefix, children, _postfix_whitespace, _)) =
                self.split_range(node, node_line)
            else {
                self.bounds.paragraphs.push(node_line);
                self.bounds.inline_paragraphs.push(node_line);
                return;
            };

            self.bounds.paragraphs.push(children);
            self.bounds.inline_paragraphs.push(children);
        }
    }

    fn compute_bounds_atx_heading(&mut self, node: &'ast AstNode<'ast>) {
        let line = self.node_first_line(node);
        let node_line = self.node_line(node, line);

        if let Some((indentation, prefix_range, _infix_range, postfix_range, _)) =
            self.split_range(node, node_line)
        {
            if !indentation.is_empty() {
                self.bounds.paragraphs.push(indentation);
            }
            self.bounds.paragraphs.push(prefix_range);

            if let Some(infix_range) = self.infix_range(node) {
                self.bounds.paragraphs.push(infix_range);

                if !postfix_range.is_empty() {
                    self.bounds.paragraphs.push(postfix_range);
                    self.bounds
                        .inline_paragraphs
                        .push((infix_range.start(), postfix_range.end()));
                } else {
                    self.bounds.inline_paragraphs.push(infix_range);
                }
            } else if !postfix_range.is_empty() {
                self.bounds.paragraphs.push(postfix_range);
                self.bounds.inline_paragraphs.push(postfix_range);
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            self.bounds.paragraphs.push(node_line);
            self.bounds.inline_paragraphs.push(node_line);
        }
    }
}
