use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Rect, Stroke, Ui, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::{
    Grapheme, IntoRangeExt as _, RangeExt as _, RangeIterExt as _,
};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::IconButton;

impl<'ast> MdRender {
    pub fn heading_row_height(&self, level: u8) -> f32 {
        self.layout.row_height
            * match level {
                6 => 1.2,
                5 => 1.4,
                4 => 1.6,
                3 => 1.8,
                2 => 2.0,
                _ => 2.4,
            }
    }

    pub fn height_heading(&self, node: &'ast AstNode<'ast>, level: u8, setext: bool) -> f32 {
        let text_height =
            if setext { self.height_setext_heading(node) } else { self.height_atx_heading(node) };
        text_height + if level == 1 { self.layout.block_spacing } else { 0. }
    }

    /// Build `Layout` for a setext heading's content line (non-underline
    /// row of a setext heading). When syntax is revealed, leading
    /// indentation and the source-line prefix come through as syntax-
    /// formatted text; otherwise just the inline children.
    fn layout_setext_heading_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme), reveal: bool,
    ) -> Layout {
        let mut layout = Layout::new(node_line);
        let Some((indentation, prefix, _, postfix_whitespace, _)) =
            self.split_range(node, node_line)
        else {
            unreachable!("setext headings never have empty lines");
        };
        if reveal {
            if !indentation.is_empty() {
                layout.push_source(
                    indentation,
                    &self.buffer[indentation],
                    self.text_format_syntax(),
                );
            }
            if !prefix.is_empty() {
                layout.push_source(prefix, &self.buffer[prefix], self.text_format_syntax());
            }
        }
        self.layout_inline_children(&mut layout, node, node_line);
        if reveal && !postfix_whitespace.is_empty() {
            layout.push_source(
                postfix_whitespace,
                &self.buffer[postfix_whitespace],
                self.text_format(node),
            );
        }
        layout
    }

    // https://github.github.com/gfm/#setext-headings
    fn height_setext_heading(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let row_height = self.row_height(node);
        let reveal = self.reveal_setext_syntax(node);
        let mut result = 0.;

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            if line_idx < last_line_idx {
                result += self.height_setext_heading_line(node, node_line, reveal);
                result += self.layout.row_spacing;
            } else if reveal {
                // setext heading underline as its own wrap unit
                let underline = self.compute_section_layout_new(
                    node_line,
                    width,
                    row_height,
                    self.text_format_syntax(),
                );
                result += underline.height;
                result += self.layout.row_spacing;
            }
        }

        result - self.layout.row_spacing
    }

    pub fn height_setext_heading_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme), reveal: bool,
    ) -> f32 {
        let width = self.width(node);
        let row_height = self.row_height(node);
        let layout = self.layout_setext_heading_line(node, node_line, reveal);
        self.compute_layout_from(layout, width, row_height).height
    }

    pub fn reveal_setext_syntax(&self, node: &'ast AstNode<'ast>) -> bool {
        // reveal syntax even if the cursor is in the indentation before the node
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];

            if self.range_revealed(line, true) {
                return true;
            }
        }
        false
    }

    /// Build `Layout` for an ATX heading line (`# heading`).
    /// Indentation + prefix `#`s + inline children + postfix
    /// whitespace, with syntax visibility driven by reveal. Empty
    /// heading falls back to rendering the whole line as syntax
    /// (Obsidian-inspired).
    fn layout_atx_heading_line(
        &self, node: &'ast AstNode<'ast>, node_line: (Grapheme, Grapheme), reveal: bool,
    ) -> Layout {
        let mut layout = Layout::new(node_line);
        if let Some((indentation, prefix_range, _, postfix_range, _)) =
            self.split_range(node, node_line)
        {
            if reveal {
                if !indentation.is_empty() {
                    layout.push_source(
                        indentation,
                        &self.buffer[indentation],
                        self.text_format_syntax(),
                    );
                }
                if !prefix_range.is_empty() {
                    layout.push_source(
                        prefix_range,
                        &self.buffer[prefix_range],
                        self.text_format_syntax(),
                    );
                }
            }
            if self.infix_range(node).is_some() {
                self.layout_inline_children(&mut layout, node, node_line);
            }
            if reveal && !postfix_range.is_empty() {
                layout.push_source(
                    postfix_range,
                    &self.buffer[postfix_range],
                    self.text_format(node),
                );
            }
        } else {
            layout.push_source(node_line, &self.buffer[node_line], self.text_format_syntax());
        }
        layout
    }

    // https://github.github.com/gfm/#atx-headings
    fn height_atx_heading(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let row_height = self.row_height(node);
        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let node_line = self.node_line(node, line);
        let reveal = self.range_revealed(line, true);
        let layout = self.layout_atx_heading_line(node, node_line, reveal);
        self.compute_layout_from(layout, width, row_height).height
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, level: u8, setext: bool,
    ) {
        if setext {
            self.show_setext_heading(ui, node, top_left, level);
        } else {
            self.show_atx_heading(ui, node, top_left, level);
        }

        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());
        let hovered = {
            let siblings_height = self.height(node) + self.heading_contained_siblings_height(node);
            let siblings_space =
                Rect::from_min_size(top_left, Vec2::new(self.width(node), siblings_height));

            siblings_space.contains(pointer)
        };

        // fold button
        // todo: proper hit-testing (this ignores anything covering the space)
        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);

        let (fold_button_size, fold_button_icon_size, fold_button_space) =
            Self::fold_button_size_icon_size_space(top_left, row_height, self.layout.indent);
        let show_fold_button = self.interactive
            && (self.touch_mode
                || hovered
                || fold_button_space.contains(pointer)
                || self.fold(node).is_some()
                || self.selected_block(node));
        if !show_fold_button {
            return;
        }

        self.show_fold_button(
            ui,
            node,
            (fold_button_size, fold_button_icon_size, fold_button_space),
            self.heading_contents(node),
            self.heading_fold_reveal(node),
        );
    }

    // https://github.github.com/gfm/#setext-headings
    fn show_setext_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) -> Response {
        let resp = Default::default();
        let width = self.width(node);
        let row_height = self.row_height(node);
        let reveal = self.reveal_setext_syntax(node);

        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            if line_idx < last_line_idx {
                let layout = self.layout_setext_heading_line(node, node_line, reveal);
                let result = self.compute_layout_from(layout, width, row_height);
                let h = result.height;
                self.show_wrap_layout(ui, top_left, &result);
                self.show_block_line_prefixes(ui, node, line, top_left, row_height);
                top_left.y += h;
                top_left.y += self.layout.row_spacing;
            } else if reveal {
                let underline = self.compute_section_layout_new(
                    node_line,
                    width,
                    row_height,
                    self.text_format_syntax(),
                );
                let h = underline.height;
                self.show_wrap_layout(ui, top_left, &underline);
                top_left.y += h;
                top_left.y += self.layout.row_spacing;
            }
        }

        top_left.y -= self.layout.row_spacing;
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
        resp
    }

    // https://github.github.com/gfm/#atx-headings
    fn show_atx_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, level: u8,
    ) -> Response {
        let resp = Default::default();
        let width = self.width(node);
        let row_height = self.row_height(node);
        let line = self.node_first_line(node); // more like node_ONLY_line amirite?
        let node_line = self.node_line(node, line);
        let reveal = self.range_revealed(line, true);

        let layout = self.layout_atx_heading_line(node, node_line, reveal);
        let result = self.compute_layout_from(layout, width, row_height);
        let height = result.height;
        self.show_wrap_layout(ui, top_left, &result);
        self.show_block_line_prefixes(ui, node, line, top_left, row_height);

        top_left.y += height;
        if level == 1 {
            self.show_heading_rule(ui, top_left, width);
        }
        resp
    }

    fn show_heading_rule(&mut self, ui: &mut Ui, top_left: Pos2, width: f32) {
        let line_break_rect =
            Rect::from_min_size(top_left, Vec2::new(width, self.layout.block_spacing));

        ui.painter().hline(
            line_break_rect.x_range(),
            line_break_rect.center().y,
            Stroke { width: 1.0, color: self.ctx.get_lb_theme().neutral() },
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
            }
        }
    }

    fn compute_bounds_setext_heading_line(
        &mut self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme), reveal: bool,
    ) {
        let node_line = self.node_line(node, line);

        if reveal {
            let Some((_, _, children, postfix_whitespace, _)) = self.split_range(node, node_line)
            else {
                self.bounds.inline_paragraphs.push(node_line);
                return;
            };

            if !postfix_whitespace.is_empty() {
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
                self.bounds.inline_paragraphs.push(node_line);
                return;
            };

            self.bounds.inline_paragraphs.push(children);
        }
    }

    fn compute_bounds_atx_heading(&mut self, node: &'ast AstNode<'ast>) {
        let line = self.node_first_line(node);
        let node_line = self.node_line(node, line);

        if let Some((_, _, _infix_range, postfix_range, _)) = self.split_range(node, node_line) {
            if let Some(infix_range) = self.infix_range(node) {
                if !postfix_range.is_empty() {
                    self.bounds
                        .inline_paragraphs
                        .push((infix_range.start(), postfix_range.end()));
                } else {
                    self.bounds.inline_paragraphs.push(infix_range);
                }
            } else if !postfix_range.is_empty() {
                self.bounds.inline_paragraphs.push(postfix_range);
            }
        } else {
            // heading is empty - show the syntax regardless if cursored (Obsidian-inspired)
            self.bounds.inline_paragraphs.push(node_line);
        }
    }

    fn heading_contained_siblings_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let NodeValue::Heading(heading) = &node.data.borrow().value else {
            panic!("heading_contained_siblings_height() invoked for non-heading")
        };
        let level = heading.level;
        let mut height_sum = 0.0;
        let mut sibling = node.next_sibling();
        while let Some(s) = sibling {
            if let NodeValue::Heading(sib_h) = &s.data.borrow().value {
                if sib_h.level <= level {
                    break;
                }
            }
            height_sum += self.block_pre_spacing_height(s);
            height_sum += self.height(s);
            height_sum += self.block_post_spacing_height(s);
            sibling = s.next_sibling();
        }
        height_sum
    }

    /// Contents end at the last contained sibling's last line: blank
    /// lines past that — before the next section or at doc end — render
    /// as visible spacing rows.
    pub fn heading_contents(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        let NodeValue::Heading(heading) = &node.data.borrow().value else {
            panic!("heading_contents() invoked for non-heading")
        };

        let mut contents = self.node_range(node).end().into_range();
        let mut sibling = node.next_sibling();
        while let Some(s) = sibling {
            if let NodeValue::Heading(sib_h) = &s.data.borrow().value {
                if sib_h.level <= heading.level {
                    break;
                }
            }
            contents.1 = self.node_last_line(s).end();
            sibling = s.next_sibling();
        }
        contents
    }

    pub fn fold_button_size_icon_size_space(
        top_left: Pos2, row_height: f32, indent: f32,
    ) -> (f32, f32, Rect) {
        let fold_button_size = indent * 0.8;
        let fold_button_icon_size = fold_button_size * 0.6;
        let fold_button_space = Rect::from_min_size(
            top_left + Vec2::new(-indent, (row_height - fold_button_size) / 2.),
            Vec2::splat(fold_button_size),
        );
        (fold_button_size, fold_button_icon_size, fold_button_space)
    }

    pub fn show_fold_button(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, size_icon_size_space: (f32, f32, Rect),
        contents: (Grapheme, Grapheme), fold_reveal: bool,
    ) {
        let (size, icon_size, space) = size_icon_size_space;
        self.touch_consuming_rects.push(space);

        if self.fold(node).is_some() {
            ui.scope_builder(UiBuilder::new().max_rect(space), |ui| {
                let theme = self.ctx.get_lb_theme();
                let icon = Icon::CHEVRON_RIGHT.size(icon_size).color(if fold_reveal {
                    theme.neutral_fg_secondary()
                } else {
                    theme.fg().get_color(theme.prefs().primary)
                });
                if IconButton::new(icon)
                    .size(size)
                    .tooltip("Show Contents")
                    .show(ui)
                    .clicked()
                {
                    self.apply_fold(node, contents, true);
                }
            });
        } else if self.foldable(node).is_some() {
            ui.scope_builder(UiBuilder::new().max_rect(space), |ui| {
                let icon = Icon::CHEVRON_DOWN
                    .size(icon_size)
                    .color(self.ctx.get_lb_theme().neutral_fg_secondary());
                if IconButton::new(icon)
                    .size(size)
                    .tooltip("Hide Contents")
                    .show(ui)
                    .clicked()
                {
                    self.apply_fold(node, contents, false);
                }
            });
        }
    }

    /// Returns true if the heading contents should be revealed whether the heading is folded or not
    pub fn heading_fold_reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        self.range_contains_fold_revealed(self.heading_contents(node), false, true)
    }
}
