use comrak::nodes::{AstNode, NodeValue};
use egui::text::LayoutJob;
use egui::{FontFamily, FontId, Pos2, Rect, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, Graphemes, RangeExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Format;

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_footnote_definition(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { color: self.ctx.get_lb_theme().neutral_fg_secondary(), ..parent_text_format }
    }

    pub fn height_footnote_definition(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    /// Paint the footnote definition's number marker into the
    /// annotation rect.
    pub(crate) fn chrome_footnote_definition(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, annotation: Rect,
    ) {
        let color = self.ctx.get_lb_theme().neutral_fg_secondary();
        let text = format!("{}.", self.definition_number(node));
        let layout_job = LayoutJob::single_section(
            text,
            TextFormat {
                font_id: FontId::new(self.layout.row_height, FontFamily::Monospace),
                color,
                ..Default::default()
            },
        );
        let galley = ui.fonts(|fonts| fonts.layout_job(layout_job));
        ui.painter()
            .galley(annotation.left_top(), galley, Default::default());
    }

    pub fn show_footnote_definition(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let annotation = Rect::from_min_size(
            top_left,
            Vec2 { x: self.layout.indent, y: self.layout.row_height },
        );
        self.chrome_footnote_definition(ui, node, annotation);
        top_left.x += annotation.width();
        self.show_block_children(ui, node, top_left);
    }

    // A clean spec is not available because these are a GFM extension, so
    // assumptions are made with experimental verification:
    // * 0-3 spaces indentation
    // * space between the syntax and first child are part of this
    // node (e.g. they don't affect indentation requirements for nested list
    // items)
    pub fn own_prefix_len_footnote_definition(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<Graphemes> {
        // todo: change paramater type of `line` to `usize` (index instead of
        // value) here and elsewhere
        // todo: unsure of lazy continuation line behavior in footnote
        // definitions
        Some(if line == self.node_first_line(node) {
            self.prefix_range(node).unwrap().len()
        } else {
            Graphemes(2).min(line.len())
        })
    }

    pub fn compute_bounds_footnote_definition(&mut self, node: &'ast AstNode<'ast>) {
        self.compute_bounds_block_children(node);
    }

    /// Footnote definitions are usually rendered in the order in which they're
    /// referenced, rather than the order in which they're written ('source
    /// order'). From where I'm sitting, I just don't see how that works in an
    /// interactive editor, even though I'm quite committed to rendering to
    /// spec, so we render them in source order.
    ///
    /// One aspect of footnote rendering we can support, though, is rendering
    /// footnote references and the labels for definitions as a number
    /// representing the order that they're referenced. So, the first
    /// _reference_ will always be rendered with a superscript '1', the next
    /// with a '2', etc, regardless the order the _definitions_ are written.
    /// This creates a unique editing experience because the rendered reference
    /// number is not from the source text; it changes to the reference text
    /// only when you select it.
    ///
    /// In the AST, it turns out, the nodes are not presented in source order
    /// when it comes to footnote definitions. Instead, they're presented in
    /// reference order. At first, I
    /// [thought](https://github.com/kivikakk/comrak/issues/554) this a bug, but
    /// it's intended behavior. This is why [`Self::sorted_siblings`] exists.
    /// Anyway, we leverage this behavior to determine the number we should
    /// render for the definition, since the node itself does not contain it.
    fn definition_number(&self, node: &'ast AstNode<'ast>) -> usize {
        let mut result = 0;
        let document = node.ancestors().last().expect("There is always a document");
        for descendant in document.descendants() {
            if matches!(descendant.data.borrow().value, NodeValue::FootnoteDefinition(_)) {
                result += 1;
            }
            if descendant.data.borrow().sourcepos == node.data.borrow().sourcepos {
                return result;
            }
        }
        unreachable!("All nodes are somewhere in the document");
    }
}
