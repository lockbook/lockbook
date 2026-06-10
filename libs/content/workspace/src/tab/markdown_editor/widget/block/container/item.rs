use comrak::nodes::{AstNode, ListType, NodeList, NodeValue};
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, Graphemes, IntoRangeExt as _, RangeExt as _};

use crate::TextBufferArea;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{BufferExt as _, FontFamily};
use crate::tab::markdown_editor::widget::utils::{
    consume_indent_columns, consume_indent_columns_ceil,
};

use crate::theme::palette_v2::ThemeExt as _;

// https://github.github.com/gfm/#list-items
impl<'ast> MdRender {
    pub fn height_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        let any_children = node.children().next().is_some();
        if any_children {
            self.block_children_height(node)
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
            let width = self.width(node) - self.layout.indent;
            self.compute_section_layout_new(
                line_content,
                width,
                self.layout.row_height,
                self.text_format_syntax(),
            )
            .height
        }
    }

    pub fn show_item(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);

        let parent = node.parent().unwrap();
        let NodeValue::List(node_list) = parent.data.borrow().value else {
            unreachable!("items always have list parents")
        };
        let NodeList { list_type, start, .. } = node_list;

        let annotation_size = Vec2 { x: self.layout.indent, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        let annotation_color = self.ctx.get_lb_theme().neutral_fg_secondary();

        // when revealed, the raw marker occupies this column instead
        if !self.reveal_line(node, first_line) {
            match list_type {
                ListType::Bullet => {
                    ui.painter().circle_filled(
                        annotation_space.center(),
                        self.layout.bullet_radius * row_height / self.layout.row_height,
                        annotation_color,
                    );
                }
                ListType::Ordered => {
                    let mut sibling_index = 0usize;
                    let mut prev = node.previous_sibling();
                    while let Some(p) = prev {
                        sibling_index += 1;
                        prev = p.previous_sibling();
                    }
                    let number = start + sibling_index;

                    // Trailing space included, as in source, so this matches
                    // the marker the reveal path draws from the source bytes.
                    let text = format!("{number}. ");
                    let ppi = self.ctx.pixels_per_point();
                    let [r, g, b, a] = annotation_color.to_array();
                    let color = glyphon::Color::rgba(r, g, b, a);
                    let mut format = self.text_format_document();
                    format.family = FontFamily::Mono;
                    format.color = annotation_color;
                    let afs = self.layout.annotation_font_size;
                    let buffer = self.upsert_glyphon_buffer(&text, afs, afs, f32::MAX, &format);
                    let size = buffer.read().unwrap().shaped_size(ppi);

                    // Right-align to the content edge, baseline-shifted into the
                    // row, identically to the revealed marker (overflow left) so
                    // revealing the item doesn't shift the number.
                    let x = annotation_space.right() - size.x;
                    let y = annotation_space.top() + (row_height - afs) * 0.8;
                    let rect = Rect::from_min_size(Pos2::new(x, y), size);
                    self.text_areas.push(TextBufferArea::new(
                        buffer,
                        rect,
                        color,
                        ui.ctx(),
                        ui.clip_rect(),
                    ));
                }
            }
        }

        let any_children = node.children().next().is_some();
        let hovered = if any_children {
            self.show_block_children(ui, node, top_left + self.layout.indent * Vec2::X);

            // todo: proper hit-testing (this ignores anything covering the space)
            let children_height = self.block_children_height(node);
            let children_space =
                Rect::from_min_size(top_left, Vec2::new(self.width(node), children_height));
            children_space.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()))
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
            let width = self.width(node) - self.layout.indent;
            let result = self.compute_section_layout_new(
                line_content,
                width,
                self.layout.row_height,
                self.text_format_syntax(),
            );
            self.show_wrap_layout(ui, top_left + self.layout.indent * Vec2::X, &result);
            self.show_block_line_prefixes(
                ui,
                node,
                line,
                top_left + self.layout.indent * Vec2::X,
                row_height,
            );
            let item_rect =
                Rect::from_min_size(top_left, Vec2::new(self.width(node), result.height));
            item_rect.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()))
        };

        // fold button
        // todo: proper hit-testing (this ignores anything covering the space)
        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());

        let (fold_button_size, fold_button_icon_size, fold_button_space) =
            Self::fold_button_size_icon_size_space(top_left, row_height, self.layout.indent);
        let show_fold_button = self.interactive
            && !self.reveal_line(node, first_line) // the revealed marker occupies the gutter
            && (self.touch_mode
                || hovered
                || fold_button_space.contains(pointer)
                || annotation_space.contains(pointer)
                || self.fold(node).is_some()
                || self.selected_fold_item(node));
        if !show_fold_button {
            return;
        }

        self.show_fold_button(
            ui,
            node,
            (fold_button_size, fold_button_icon_size, fold_button_space),
            self.item_contents(node),
            self.item_fold_reveal(node),
        );
    }

    pub fn own_prefix_len_item(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme), node_list: &NodeList,
    ) -> Option<Graphemes> {
        let node_line = self.node_line(node, line);
        let mut result: Graphemes = 0.into();

        // "If a sequence of lines Ls constitutes a list item according to rule
        // #1, #2, or #3, then the result of indenting each line of Ls by 1-3
        // spaces (the same for each line) also constitutes a list item with the
        // same contents and attributes."
        let indentation = {
            let first_line = self.node_first_line(node);
            let first_node_line = self.node_line(node, first_line);

            // 1-3 columns of relative indent before the marker. Ceil so a
            // tab a parent level left straddling the boundary is claimed as
            // this item's indent rather than leaking into content.
            let text = &self.buffer[first_node_line];
            consume_indent_columns_ceil(text, 3)
        };
        let NodeList { padding: marker_width_including_spaces, .. } = *node_list;
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
            //
            // Two regimes: an indented-code-block child line defers
            // stripping to `code_block.rs` (combined item.padding+4
            // column-aware strip — needed so a tab straddling the
            // item/code boundary doesn't render differently from
            // 4 spaces). All other continuation lines strip leading
            // ws here. Child detection uses sourcepos (cheap) instead
            // of `node_range` (recursive).
            let line_1_based = self
                .bounds
                .source_lines
                .find_containing(line.start(), true, false)
                .start()
                + 1;
            let line_has_code_block_child = node.children().any(|c| match &c.data.borrow().value {
                NodeValue::CodeBlock(b) if !b.fenced => {
                    let sp = c.data.borrow().sourcepos;
                    sp.start.line <= line_1_based && line_1_based <= sp.end.line
                }
                _ => false,
            });
            let text = &self.buffer[node_line];
            if line_has_code_block_child {
                // Indented code block line — leave stripping to
                // `code_block.rs`.
            } else {
                // Per-level continuation indent. `has_deeper` tests raw
                // sourcepos, not `node_range`/`node_line`, which recurse
                // back into the `line_prefix_len` being computed here.
                let has_deeper = node.descendants().skip(1).any(|d| {
                    self.is_gutter_level(d) && {
                        let sp = d.data.borrow().sourcepos;
                        sp.start.line <= line_1_based && line_1_based <= sp.end.line
                    }
                });
                if has_deeper {
                    // Claim only this level's columns (ceil keeps a
                    // straddling tab); the deeper level takes the rest.
                    result += consume_indent_columns_ceil(text, marker_width_including_spaces);
                } else {
                    // Deepest gutter item — take the rest so content
                    // isn't over-indented.
                    result += consume_indent_columns(text, usize::MAX);
                }
            }
        }

        // marker_width_including_spaces reports the width _with_ spaces even
        // when they're not present
        Some(result.min(node_line.len()))
    }

    pub fn compute_bounds_item(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);

            self.bounds.inline_paragraphs.push(line_content);
        }
    }

    pub fn item_contents(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        // contents start at the end of the first child, which acts as a sort of section title
        // if no children, start at end of node first line
        let mut contents: (Grapheme, Grapheme) = if let Some(first_child) = node.children().next() {
            self.node_range(first_child).end().into_range()
        } else {
            self.node_first_line(node).end().into_range()
        };

        // Contents end at the last child's last line — the end of the
        // hidden subtree. Blank lines past that render as visible
        // spacing rows, so they're boundary, not contents.
        if let Some(last_child) = node.children().last() {
            contents.1 = contents.1.max(self.node_last_line(last_child).end());
        }

        contents
    }

    /// Returns true if the item contents should be revealed whether the item is folded or not
    pub fn item_fold_reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        self.range_contains_fold_revealed(self.item_contents(node), false, true)
    }

    /// Returns true if the item is selected for folding; specialized adaptation of self.selected_block()
    pub fn selected_fold_item(&self, node: &'ast AstNode<'ast>) -> bool {
        // any items selected -> those items selected for fold
        let root = node.ancestors().last().unwrap();
        for descendent in root.descendants() {
            if self.selected_block(descendent)
                && matches!(descendent.data().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
            {
                return self.selected_block(node);
            }
        }

        // else -> parent item of any selected items selected for fold
        node.first_child()
            .map(|c| self.selected_block(c))
            .unwrap_or_default()
    }
}
