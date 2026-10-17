use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn layout_text(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node).trim(&range);
        if node_range.is_empty() {
            return;
        }
        self.layout_source(layout, node, node_range, self.text_format(node), true);
    }

    /// Lays out `range` of the source as it is written, in `format`, with
    /// the completion search term picked out if `search`. In a table cell
    /// a pipe is written `\|`. The parser has no node for the escape, so
    /// its backslash is hidden and revealed here, as an escape's is
    /// anywhere else.
    pub fn layout_source(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
        format: Format, search: bool,
    ) {
        let run = |layout: &mut Layout, range: (Grapheme, Grapheme)| {
            if search {
                self.layout_text_run(layout, range, format.clone());
            } else if !range.is_empty() {
                layout.push_source(range, &self.buffer[range], format.clone());
            }
        };
        let mut start = range.start();
        for escape in self.table_cell_pipe_escapes(node, range) {
            run(layout, (start, escape));
            let escaped_pipe = (escape, escape + 2);
            if self.range_revealed(escaped_pipe, true) {
                let backslash = (escape, escape + 1);
                layout.push_source(backslash, &self.buffer[backslash], self.text_format_syntax());
            } else {
                layout.push_override(escape.into_range(), "", self.text_format_syntax());
            }
            start = escape + 1;
        }
        run(layout, (start, range.end()));
    }

    fn layout_text_run(&self, layout: &mut Layout, range: (Grapheme, Grapheme), format: Format) {
        if range.is_empty() {
            return;
        }
        if let Some(search_range) = self.search_range {
            let start = search_range.0.max(range.0);
            let end = search_range.1.min(range.1);
            if start < end {
                let theme = self.ctx.get_lb_theme();
                let accent = theme.fg().get_color(theme.prefs().primary);
                let accent_format = Format { color: accent, ..format.clone() };
                if range.0 < start {
                    let r = (range.0, start);
                    layout.push_source(r, &self.buffer[r], format.clone());
                }
                let r = (start, end);
                layout.push_source(r, &self.buffer[r], accent_format);
                if end < range.1 {
                    let r = (end, range.1);
                    layout.push_source(r, &self.buffer[r], format);
                }
                return;
            }
        }
        layout.push_source(range, &self.buffer[range], format);
    }

    /// The backslash of each `\|` in `range`, if `node` is in a table cell.
    /// Both characters are ASCII, so each is a grapheme.
    fn table_cell_pipe_escapes(
        &self, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) -> Vec<Grapheme> {
        let text = &self.buffer[range];
        let in_cell = || {
            node.ancestors()
                .any(|ancestor| matches!(ancestor.data.borrow().value, NodeValue::TableCell))
        };
        if !text.contains("\\|") || !in_cell() {
            return Vec::new();
        }

        let mut escapes = Vec::new();
        let bytes = text.as_bytes();
        let start_byte = self.offset_to_byte(range.start()).0;
        let graphemes = &self.buffer.current.segs.grapheme_indexes[range.start().0..range.end().0];
        // a backslash takes the byte after it, whatever it is
        let mut taken = 0;
        for (grapheme, byte) in graphemes.iter().enumerate() {
            let byte = byte.0 - start_byte;
            if byte >= taken && bytes[byte] == b'\\' {
                taken = byte + 2;
                if bytes.get(byte + 1) == Some(&b'|') {
                    escapes.push(range.start() + grapheme);
                }
            }
        }
        escapes
    }
}
