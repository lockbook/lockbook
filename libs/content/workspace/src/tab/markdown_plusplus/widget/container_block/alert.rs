use comrak::nodes::{AlertType, AstNode, NodeAlert};
use egui::{Pos2, Rect, Stroke, TextFormat, TextStyle, TextWrapMode, Ui, Vec2, WidgetText};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt as _, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_plusplus::widget::{Wrap, BLOCK_SPACING, INDENT, ROW_HEIGHT};
use crate::tab::markdown_plusplus::MarkdownPlusPlus;
use crate::theme::icons::Icon;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_alert(&self, parent: &AstNode<'_>, node_alert: &NodeAlert) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            color: match node_alert.alert_type {
                AlertType::Note => self.theme.fg().blue,
                AlertType::Tip => self.theme.fg().green,
                AlertType::Important => self.theme.fg().magenta,
                AlertType::Warning => self.theme.fg().yellow,
                AlertType::Caution => self.theme.fg().red,
            },
            ..parent_text_format
        }
    }

    pub fn height_alert(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_block_quote(node)
    }

    pub fn show_alert(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_alert: &NodeAlert,
    ) {
        let height = self.height_alert(node);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.text_format(node).color),
        );
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            let prefix_len = self.line_prefix_len(node, line);
            let parent_prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
            let prefix = (line.start() + parent_prefix_len, line.start() + prefix_len);

            self.bounds.paragraphs.push(prefix);
        }

        top_left.x += annotation_space.width();

        let line = self.node_first_line(node);
        let node_line = (line.start() + self.line_prefix_len_block_quote(node, line), line.end());
        if node_line.intersects(&self.buffer.current.selection, true) {
            // note and title line are revealed separately from block syntax as
            // if they're a child block
            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(self.width(node) - annotation_space.width()),
                node_line,
                self.text_format_syntax(node),
                false,
            );
        } else {
            let icon_space = Rect::from_min_size(top_left, Vec2::splat(ROW_HEIGHT));
            let display_text_top_left = top_left + Vec2::X * INDENT;

            // icon
            {
                // debug
                // ui.painter().rect_stroke(
                //     icon_space,
                //     2.,
                //     egui::Stroke::new(1., self.text_format(node).color),
                // );

                let icon = &match node_alert.alert_type {
                    AlertType::Note => Icon::INFO,
                    AlertType::Tip => Icon::LIGHT_BULB,
                    AlertType::Important => Icon::FEEDBACK,
                    AlertType::Warning => Icon::WARNING_2,
                    AlertType::Caution => Icon::REPORT,
                };
                let draw_pos =
                    icon_space.center() - Vec2::splat(icon.size) / 2. + Vec2::new(0., 1.5);

                let icon_text: WidgetText = icon.into();
                let galley =
                    icon_text.into_galley(ui, Some(TextWrapMode::Extend), 0., TextStyle::Body);
                ui.painter()
                    .galley(draw_pos, galley, self.text_format(node).color);
            }

            let (_type, title) = self.alert_type_title_ranges(node);
            if node_alert.title.is_some() {
                self.show_text_line(
                    ui,
                    display_text_top_left,
                    &mut Wrap::new(self.width(node) - annotation_space.width()),
                    title,
                    self.text_format(node),
                    false,
                );
            } else {
                let type_display_text = match node_alert.alert_type {
                    AlertType::Note => "Note",
                    AlertType::Tip => "Tip",
                    AlertType::Important => "Important",
                    AlertType::Warning => "Warning",
                    AlertType::Caution => "Caution",
                };
                self.show_override_text_line(
                    ui,
                    display_text_top_left,
                    &mut Wrap::new(self.width(node) - annotation_space.width()),
                    (node_line.end() - 1).into_range(),
                    self.text_format(node),
                    false,
                    Some(type_display_text),
                );
            }
        }

        self.bounds.paragraphs.push(node_line);

        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
        } else {
            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let node_line = (line.start() + self.line_prefix_len(node, line), line.end());
                let node_line_start = node_line.start().into_range();

                self.show_text_line(
                    ui,
                    top_left,
                    &mut Wrap::new(self.width(node)),
                    node_line_start,
                    self.text_format_document(),
                    false,
                );
                top_left.y += ROW_HEIGHT + BLOCK_SPACING;
            }
        }
    }

    pub fn line_prefix_len_alert(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        node_alert: &NodeAlert,
    ) -> RelCharOffset {
        let NodeAlert { multiline, .. } = node_alert;
        if *multiline {
            self.line_prefix_len_multiline_block_quote(node, line)
        } else {
            self.line_prefix_len_block_quote(node, line)
        }
    }

    // todo: multiline
    fn alert_type_title_ranges(
        &self, node: &'ast AstNode<'ast>,
    ) -> ((DocCharOffset, DocCharOffset), (DocCharOffset, DocCharOffset)) {
        let line = self.node_first_line(node);
        let node_line = (line.start() + self.line_prefix_len_block_quote(node, line), line.end());
        let node_line_text = &self.buffer[node_line].to_uppercase();

        let type_len = if node_line_text.starts_with("[!NOTE]") {
            "[!NOTE]".len()
        } else if node_line_text.starts_with("[!TIP]") {
            "[!TIP]".len()
        } else if node_line_text.starts_with("[!IMPORTANT]") {
            "[!IMPORTANT]".len()
        } else if node_line_text.starts_with("[!WARNING]") {
            "[!WARNING]".len()
        } else if node_line_text.starts_with("[!CAUTION]") {
            "[!CAUTION]".len()
        } else {
            unreachable!("supported alert types are note, tip, important, warning, caution")
        };
        let _type = (node_line.start(), node_line.start() + type_len);

        // todo: empty title
        let untrimmed_title = (node_line.start() + type_len, node_line.end());
        let untrimmed_title_text = &self.buffer[untrimmed_title];
        let title = if untrimmed_title_text.trim().is_empty() {
            untrimmed_title
        } else {
            let title_leading_whitespace_len = untrimmed_title_text.chars().count()
                - untrimmed_title_text.trim_start().chars().count();
            let title_trailing_whitespace_len = untrimmed_title_text.chars().count()
                - untrimmed_title_text.trim_end().chars().count();
            (
                untrimmed_title.start() + title_leading_whitespace_len,
                untrimmed_title.end() - title_trailing_whitespace_len,
            )
        };

        (_type, title)
    }
}
