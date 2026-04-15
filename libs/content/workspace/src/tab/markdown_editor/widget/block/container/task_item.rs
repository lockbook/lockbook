use comrak::nodes::{AstNode, NodeTaskItem};
use egui::{CursorIcon, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_editor::{Editor, Event};
use crate::theme::palette_v2::ThemeExt;

impl<'ast> Editor {
    pub fn height_task_item(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_task_item(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_task_item: &NodeTaskItem, siblings: &[&'ast AstNode<'ast>],
    ) {
        let theme = self.ctx.get_lb_theme();
        let maybe_check = node_task_item.symbol;
        let checked = maybe_check.is_some();

        let first_line = self.node_first_line(node);
        let row_height = self.node_line_row_height(node, first_line);

        let annotation_size = Vec2 { x: self.layout.indent, y: row_height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        let checkbox_space = Rect::from_center_size(
            annotation_space.center(),
            Vec2::splat(annotation_space.size().min_elem()),
        );

        // allocate a slightly larger clickable space that makes the checkbox
        // easier to tap without overflowing the annotation space horizontally
        // or overlapping with another row vertically.
        let extra_width = (annotation_space.width() - checkbox_space.width()) / 2.;
        let extra_height = self.layout.row_spacing / 2.;
        let clickable_space = checkbox_space.expand(extra_width.min(extra_height));

        let checkbox_response =
            // ui.id().with() instead of Id::new() so two views of the same document
            // get distinct checkbox IDs
            ui.interact(clickable_space, ui.id().with(self.node_range(node)), Sense::click());
        if checkbox_response.clicked() {
            let check_offset = self.check_offset(node);
            let new_check = if checked { ' ' } else { 'x' };
            self.event.internal_events.push(Event::Replace {
                region: (check_offset, check_offset + 1).into(),
                text: new_check.into(),
                advance_cursor: false,
            });
        }
        if checkbox_response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        self.touch_consuming_rects.push(clickable_space);

        // draw checkbox
        ui.painter().rect(
            checkbox_space,
            2,
            if checkbox_response.hovered() {
                theme.neutral()
            } else {
                theme.neutral_bg_secondary()
            },
            Stroke::new(1., theme.neutral()),
            StrokeKind::Inside,
        );

        // draw check
        if checked {
            let check_space = checkbox_space.shrink(3.);
            ui.painter().add(Shape::line(
                vec![
                    egui::pos2(check_space.left(), check_space.center().y),
                    egui::pos2(check_space.center().x, check_space.bottom()),
                    egui::pos2(check_space.right(), check_space.top()),
                ],
                Stroke::new(1.5, theme.neutral_fg_secondary()),
            ));
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

            let mut wrap = self.new_wrap(self.width(node));
            let resp = self.show_section(
                ui,
                top_left + self.layout.indent * Vec2::X,
                &mut wrap,
                line_content,
                self.text_format_document(),
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);

            resp.hovered
        };

        // fold button
        // todo: proper hit-testing (this ignores anything covering the space)
        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());

        let (fold_button_size, fold_button_icon_size, fold_button_space) =
            Self::fold_button_size_icon_size_space(top_left, row_height, self.layout.indent);
        let show_fold_button = self.touch_mode
            || hovered
            || fold_button_space.contains(pointer)
            || annotation_space.contains(pointer)
            || self.fold(node).is_some()
            || self.selected_fold_item(node);
        if !show_fold_button {
            return;
        }

        self.show_fold_button(
            ui,
            node,
            (fold_button_size, fold_button_icon_size, fold_button_space),
            self.item_contents(node, siblings),
            self.item_fold_reveal(node, siblings),
        );
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
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            let line = self.node_first_line(node);
            let line_content = self.line_content(node, line);
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
