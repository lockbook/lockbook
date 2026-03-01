use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, TextFormat, Vec2};
use egui_wgpu_renderer::egui_wgpu;
use lb_rs::model::text::offset_types::RangeIterExt as _;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;
use crate::tab::markdown_editor::widget::{ROW_HEIGHT, ROW_SPACING};
use crate::{GlyphonRendererCallback, TextBufferArea};

impl<'ast> Editor {
    pub fn text_format_document(&self) -> TextFormat {
        let parent_text_format = TextFormat::default();
        TextFormat {
            color: self.theme.fg().neutral_secondary,
            font_id: FontId {
                size: parent_text_format.font_id.size * ROW_HEIGHT
                    / self
                        .ctx
                        .fonts(|fonts| fonts.row_height(&parent_text_format.font_id)),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn height_document(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);

        let any_children = node.children().next().is_some();
        if any_children && !self.plaintext_mode {
            self.block_children_height(node)
        } else {
            let mut result = 0.;
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                result +=
                    self.height_section(&mut Wrap::new(width), line, self.text_format_syntax(node));
                result += ROW_SPACING;
            }
            result
        }
    }

    pub fn show_document(
        &mut self, ui: &mut egui::Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let width = self.width(node);

        // let any_children = node.children().next().is_some();
        // if any_children && !self.plaintext_mode {
        //     self.show_block_children(ui, node, top_left);
        // } else {
        //     for line_idx in self.node_lines(node).iter() {
        //         let line = self.bounds.source_lines[line_idx];

        //         let mut wrap = Wrap::new(width);
        //         self.show_section(
        //             ui,
        //             top_left,
        //             &mut wrap,
        //             line,
        //             self.text_format_syntax(node),
        //             false,
        //         );
        //         top_left.y += wrap.height();
        //         top_left.y += ROW_SPACING;
        //         self.bounds.wrap_lines.extend(wrap.row_ranges);
        //     }
        // }

        {
            let mut font_system = self.font_system.lock().unwrap();
            let mut buffer = self.glyphon_buffer.write().unwrap();
            buffer.set_metrics(&mut font_system, glyphon::Metrics::new(35., 35.));
            buffer.set_size(&mut font_system, Some(16. * 35.), Some(9. * 35.));
            buffer.shape_until_scroll(&mut font_system, false);
        }

        let rect = Rect::from_min_size(top_left, Vec2::new(width, 100.));
        let buffers: Vec<TextBufferArea> = vec![TextBufferArea::new(
            self.glyphon_buffer.clone(),
            rect,
            glyphon::Color::rgb(255, 255, 255),
            ui.ctx(),
        )];
        ui.painter()
            .rect_stroke(rect, 2., egui::Stroke::new(1., egui::Color32::LIGHT_BLUE));
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            ui.max_rect(),
            GlyphonRendererCallback { buffers },
        ));
    }

    pub fn compute_bounds_document(&mut self, node: &'ast AstNode<'ast>) {
        let any_children = node.children().next().is_some();
        if any_children {
            self.compute_bounds_block_children(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                self.bounds.inline_paragraphs.push(line);
            }
        }
    }
}
