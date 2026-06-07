use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, StyleInfo};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_code(&self, parent: &AstNode<'_>) -> Format {
        let theme = self.ctx.get_lb_theme();
        Format {
            color: theme.fg().get_color(theme.prefs().primary),
            // Translucent fill that matches the opaque look over the page
            // but lets a selection behind it show through.
            background: crate::theme::palette_v2::translucent_over(
                theme.neutral_bg_secondary(),
                theme.neutral_bg(),
                0.5,
            ),
            border: theme.neutral_bg_tertiary(),
            bold: false, // SF Mono does not have bold variants for numbers (it does have italic)
            ..self.text_format_code_block(parent)
        }
    }

    /// `Code` has no AST children — the inline content sits in
    /// `buffer[node_range.start+1 .. node_range.end-1]` directly. Wrap
    /// the inner range in `StyleOpen`/`StyleClose` carrying the code
    /// format (bg + monospace + accent color), with backticks as
    /// syntax-formatted prefix/postfix.
    pub fn layout_code(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        // Multi-line paragraphs dispatch every inline child to every
        // source line's layout. Without this guard the code scope's
        // `style_open` / `style_close` still emit, and (because code
        // carries a bg) the walker injects side-padding `Pad`s into
        // lines that don't actually contain the code.
        if node_range.trim(&range).is_empty() {
            return;
        }
        let prefix = (node_range.start(), node_range.start() + 1).trim(&range);
        let infix = (node_range.start() + 1, node_range.end() - 1).trim(&range);
        let postfix = (node_range.end() - 1, node_range.end()).trim(&range);
        let reveal = self.node_revealed(node);

        layout.style_open(StyleInfo {
            format: self.text_format_code(node.parent().unwrap()),
            source_range: node_range,
        });
        if !prefix.is_empty() {
            if reveal {
                layout.push_source(prefix, &self.buffer[prefix], self.text_format_syntax());
            } else {
                layout.push_override(prefix.start().into_range(), "", self.text_format_syntax());
            }
        }
        if !infix.is_empty() {
            layout.push_source(
                infix,
                &self.buffer[infix],
                self.text_format_code(node.parent().unwrap()),
            );
        }
        if !postfix.is_empty() {
            if reveal {
                layout.push_source(postfix, &self.buffer[postfix], self.text_format_syntax());
            } else {
                layout.push_override(postfix.end().into_range(), "", self.text_format_syntax());
            }
        }
        layout.style_close();
    }
}
