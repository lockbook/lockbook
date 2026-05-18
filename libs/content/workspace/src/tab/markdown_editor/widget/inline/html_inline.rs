use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;

pub const FOLD_TAG: &str = "<!-- {\"fold\":true} -->";

impl<'ast> MdRender {
    pub fn text_format_html_inline(&self, parent: &AstNode<'_>) -> Format {
        Format {
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            background: self.ctx.get_lb_theme().neutral_bg_secondary(),
            ..self.text_format_code_block(parent)
        }
    }

    pub fn layout_html_inline(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node).trim(&range);
        if node_range.is_empty() {
            return;
        }
        // Reveal when cursor is on the range OR when the html-inline
        // is not a fold marker (regular `<sup>` etc. always show source).
        // Uses range_revealed to honor reveal_selection (NAVIGATION_NOTES
        // Phase 3 — no direct selection reads).
        let reveal = self.range_revealed(node_range, true) || self.foldee(node).is_none();
        if reveal {
            layout.push_source(node_range, &self.buffer[node_range], self.text_format_syntax());
        } else {
            layout.push_override(
                node_range,
                "",
                self.text_format_html_inline(node.parent().unwrap()),
            );
        }
    }
}
