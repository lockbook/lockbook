use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, StyleInfo};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn layout_text(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node).trim(&range);
        if node_range.is_empty() {
            return;
        }
        let text_format = self.text_format(node);
        // `scoped`: underline rules are drawn from a style scope
        let highlight = if let Some(marked) = &self.ime_marked {
            Some((marked.range, Format { underline: true, ..text_format.clone() }, true))
        } else {
            self.search_range.map(|r| {
                let theme = self.ctx.get_lb_theme();
                let accent = theme.fg().get_color(theme.prefs().primary);
                (r, Format { color: accent, ..text_format.clone() }, false)
            })
        };
        if let Some((range, highlight_format, scoped)) = highlight {
            let start = range.0.max(node_range.0);
            let end = range.1.min(node_range.1);
            if start < end {
                if node_range.0 < start {
                    let r = (node_range.0, start);
                    layout.push_source(r, &self.buffer[r], text_format.clone());
                }
                let r = (start, end);
                if scoped {
                    layout.style_open(StyleInfo::new(highlight_format.clone(), r));
                }
                layout.push_source(r, &self.buffer[r], highlight_format);
                if scoped {
                    layout.style_close();
                }
                if end < node_range.1 {
                    let r = (end, node_range.1);
                    layout.push_source(r, &self.buffer[r], text_format);
                }
                return;
            }
        }
        layout.push_source(node_range, &self.buffer[node_range], text_format);
    }
}
