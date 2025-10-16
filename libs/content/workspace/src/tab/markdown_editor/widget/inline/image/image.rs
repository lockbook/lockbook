use std::f32;

use comrak::nodes::{AstNode, NodeLink};
use egui::{self, Pos2, Rect, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn text_format_image(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_link(parent)
    }

    pub fn span_image(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_image(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        node_link: &NodeLink, range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        self.show_link(ui, node, top_left, wrap, node_link, range)
    }

    pub fn height_image(&self, node: &'ast AstNode<'ast>, url: &str) -> f32 {
        let max_size = Vec2::new(self.width(node), self.height);
        self.embed_resolver.height(url, max_size)
    }

    pub fn show_image_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, url: &str,
    ) {
        let max_size = Vec2::new(self.width(node), self.height);
        let rect = Rect::from_min_size(top_left, max_size);
        self.embed_resolver.show(url, rect, &self.theme, ui);
    }
}
