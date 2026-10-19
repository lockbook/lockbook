//! The rendered clipboard flavors: the selection as the editor draws it,
//! and the same markdown as HTML.

use comrak::nodes::{AstNode, NodeCodeBlock, NodeHeading, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;

impl<'ast> MdRender {
    /// `range` as drawn with nothing revealed: inline syntax, heading
    /// marks, quote bars, and code fences are gone; list markers stay
    /// because they are drawn as text.
    pub fn rendered_text(
        &mut self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) -> String {
        let reveal = (
            self.reveal_selection.take(),
            self.find_current_match.take(),
            self.preview_match.take(),
            self.entered_atom.take(),
        );
        let text = self.rendered_text_unrevealed(root, range);
        (self.reveal_selection, self.find_current_match, self.preview_match, self.entered_atom) =
            reveal;
        text
    }

    /// A single paragraph comes unwrapped so it pastes inline, not as a block
    /// of its own.
    pub fn rendered_html(&self, range: (Grapheme, Grapheme)) -> String {
        let mut options = Self::comrak_options();
        options.parse.smart = false; // the plain flavor and the screen show the source characters
        let html = comrak::markdown_to_html(&self.buffer[range], &options);
        match html
            .strip_prefix("<p>")
            .and_then(|h| h.strip_suffix("</p>\n"))
        {
            Some(inner) if !inner.contains("<p>") => inner.to_string(),
            _ => html,
        }
    }

    fn rendered_text_unrevealed(
        &self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) -> String {
        let (first, end) = self.range_lines(range);
        let mut blocks = root.children().peekable();
        let mut lines = Vec::new();
        for line_idx in first..end {
            while blocks
                .peek()
                .is_some_and(|b| self.node_lines(b).1 <= line_idx)
            {
                blocks.next();
            }
            let block = blocks.peek().filter(|b| self.node_lines(b).0 <= line_idx);
            let line = self.bounds.source_lines[line_idx];
            let text = match block {
                Some(block) => self.rendered_line(block, line_idx, line.trim(&range)),
                None => Some(String::new()), // a blank line between blocks
            };
            if let Some(text) = text {
                lines.push(text);
            }
        }
        lines.join("\n")
    }

    /// One source line of `block`; `None` for lines drawn as pure decoration
    /// (code fences, setext underlines, table delimiter rows).
    fn rendered_line(
        &self, block: &'ast AstNode<'ast>, line_idx: usize, clip: (Grapheme, Grapheme),
    ) -> Option<String> {
        let line = self.bounds.source_lines[line_idx];
        let node = self.block_at_line(block, line_idx);
        let byte_of = |g| self.offset_to_byte(g).0;

        let mut markers: Vec<_> = node
            .ancestors()
            .filter(|a| {
                matches!(a.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
            })
            .map(|a| self.line_own_prefix(a, line).trim(&clip))
            .collect();
        markers.sort();
        let mut out: String = markers.into_iter().map(|m| &self.buffer[m]).collect();

        let node_line = self.node_line(node, line);
        let content = match &node.data.borrow().value {
            NodeValue::Paragraph => self
                .layout_paragraph_line(node, node_line)
                .visible_in(clip, byte_of),
            NodeValue::Heading(NodeHeading { setext: false, .. }) => self
                .layout_atx_heading_line(node, node_line, false)
                .visible_in(clip, byte_of),
            NodeValue::Heading(NodeHeading { setext: true, .. }) => {
                if line_idx == self.node_last_line_idx(node) {
                    return None;
                }
                self.layout_setext_heading_line(node, node_line, false)
                    .visible_in(clip, byte_of)
            }
            NodeValue::CodeBlock(code_block) => {
                self.rendered_code_line(node, code_block, line_idx, clip, byte_of)?
            }
            NodeValue::FrontMatter(_) => self.rendered_code_line(
                node,
                &Self::frontmatter_as_code_block(),
                line_idx,
                clip,
                byte_of,
            )?,
            NodeValue::HtmlBlock(_) => self.buffer[node_line.trim(&clip)].to_string(),
            NodeValue::TableRow(_) => node
                .children()
                .filter(|cell| self.node_range(cell).intersects(&clip, false))
                .map(|cell| {
                    self.layout_table_cell(cell)
                        .visible_in(self.node_range(cell).trim(&clip), byte_of)
                })
                .collect::<Vec<_>>()
                .join("\t"),
            NodeValue::Table(_) => return None,
            _ => String::new(),
        };
        out.push_str(&content);
        Some(out)
    }

    fn rendered_code_line(
        &self, node: &'ast AstNode<'ast>, code_block: &NodeCodeBlock, line_idx: usize,
        clip: (Grapheme, Grapheme), byte_of: impl Fn(Grapheme) -> usize,
    ) -> Option<String> {
        let line = self.bounds.source_lines[line_idx];
        if code_block.fenced {
            let first = self.node_first_line_idx(node);
            let last = self.node_last_line_idx(node);
            if line_idx == first
                || (line_idx == last && self.is_closing_fence(node, code_block, line))
            {
                return None;
            }
        }
        let layout = self.layout_code_block_line(node, code_block, line, false);
        Some(layout.visible_in(clip, byte_of))
    }

    /// The deepest block under `block` whose lines include `line_idx`. Stops
    /// at a table row, whose cells share the line.
    fn block_at_line(&self, block: &'ast AstNode<'ast>, line_idx: usize) -> &'ast AstNode<'ast> {
        let mut node = block;
        'descend: while !matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            for child in node.children() {
                let value = &child.data.borrow().value;
                if !value.block() && !matches!(value, NodeValue::FrontMatter(_)) {
                    break 'descend;
                }
                let (first, end) = self.node_lines(child);
                if (first..end).contains(&line_idx) {
                    node = child;
                    continue 'descend;
                }
            }
            break;
        }
        node
    }
}
