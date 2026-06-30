use comrak::nodes::{AstNode, NodeFootnoteReference, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt, RangeExt as _};

use crate::tab::markdown_editor::MdRender;

pub(crate) mod card;
pub(crate) mod code;
pub(crate) mod emph;
pub(crate) mod escaped;
pub(crate) mod escaped_tag;
pub(crate) mod footnote_reference;
pub(crate) mod highlight;
pub(crate) mod html_inline;
pub(crate) mod image;
pub(crate) mod line_break;
pub(crate) mod link;
pub(crate) mod link_meta;
pub(crate) mod math;
pub(crate) mod short_code;
pub(crate) mod soft_break;
pub(crate) mod spoilered_text;
pub(crate) mod strikethrough;
pub(crate) mod strong;
pub(crate) mod subscript;
pub(crate) mod superscript;
pub(crate) mod text;
pub(crate) mod underline;
pub(crate) mod wiki_link;

#[derive(Default, Debug)]
pub struct Response {
    pub clicked: bool,
    pub hovered: bool,
}

impl std::ops::BitOrAssign for Response {
    fn bitor_assign(&mut self, rhs: Self) {
        self.clicked |= rhs.clicked;
        self.hovered |= rhs.hovered;
    }
}

impl<'ast> MdRender {
    /// Returns the range between the start of the node and the start of its
    /// first child, if there is one.
    pub fn prefix_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        let range = self.node_range(node);
        let first_child = node.children().next()?;
        let first_child_range = self.node_range(first_child);
        Some((range.start(), first_child_range.start()))
    }

    /// Returns the range of the leading syntax characters that define this node
    pub fn head_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        if matches!(node.data.borrow().value, NodeValue::Code(_)) {
            let range = self.node_range(node);
            Some((range.start(), range.start() + 1)) // code has no children; contains text directly
        } else {
            self.prefix_range(node)
        }
    }

    /// Returns the range between the end of the node's last child if there is
    /// one, and the end of the node.
    pub fn postfix_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        let range = self.node_range(node);
        let last_child = node.children().last()?;
        let last_child_range = self.node_range(last_child);
        Some((last_child_range.end(), range.end()))
    }

    /// Returns the range of the trailing syntax characters that define this node
    pub fn tail_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        if matches!(node.data.borrow().value, NodeValue::Code(_)) {
            let range = self.node_range(node);
            Some((range.end() - 1, range.end())) // code has no children; contains text directly
        } else {
            self.postfix_range(node)
        }
    }

    /// Returns the range between the start of the node's first child and the
    /// end of it's last child, if there are any children. For many nodes, this
    /// is the content in the node.
    pub fn infix_range(&self, node: &'ast AstNode<'ast>) -> Option<(Grapheme, Grapheme)> {
        let first_child = node.children().next()?;
        let first_child_range = self.node_range(first_child);
        let last_child = node.children().last()?;
        let last_child_range = self.node_range(last_child);
        Some((first_child_range.start(), last_child_range.end()))
    }

    /// Returns true if the node intersects the current selection. Useful for
    /// checking if syntax should be revealed for an inline node. Block nodes
    /// generally need additional consideration for optional indentation etc.
    pub fn node_intersects_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        self.node_range(node)
            .intersects(&self.buffer.current.selection, true)
    }

    pub fn node_contains_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        self.node_range(node)
            .contains_range(&self.buffer.current.selection, true, true)
    }

    /// Returns true if the node's range intersects any reveal range. Drop-in
    /// replacement for `node_intersects_selection` in reveal contexts.
    pub fn node_revealed(&self, node: &'ast AstNode<'ast>) -> bool {
        self.range_revealed(self.node_range(node), true)
    }

    pub fn reveal_ranges(&self) -> impl Iterator<Item = (Grapheme, Grapheme)> + '_ {
        self.reveal_selection
            .into_iter()
            .chain(self.find_current_match)
            .chain(self.preview_match)
    }

    /// Returns true if `range` intersects any reveal range.
    pub fn range_revealed(&self, range: (Grapheme, Grapheme), allow_empty: bool) -> bool {
        self.reveal_ranges()
            .any(|rr| range.intersects(&rr, allow_empty))
    }

    /// True if any reveal range has an endpoint *strictly inside* `range`.
    /// Unlike [`Self::range_revealed`], bordering or wholly enclosing it
    /// (tap-select, select-all) doesn't count — only a cursor/end in the interior.
    pub fn range_revealed_interior(&self, range: (Grapheme, Grapheme)) -> bool {
        self.reveal_ranges().any(|rr| {
            range.contains(rr.start(), false, false) || range.contains(rr.end(), false, false)
        })
    }

    /// Returns true if `range` contains any reveal range.
    pub fn range_contains_revealed(
        &self, range: (Grapheme, Grapheme), allow_empty_range: bool, allow_empty_selection: bool,
    ) -> bool {
        self.reveal_ranges()
            .any(|rr| range.contains_range(&rr, allow_empty_range, allow_empty_selection))
    }

    /// Returns true if any reveal range start or end falls within `range`.
    pub fn range_contains_reveal_endpoint(
        &self, range: (Grapheme, Grapheme), start_inclusive: bool, end_inclusive: bool,
    ) -> bool {
        self.reveal_ranges().any(|rr| {
            range.contains(rr.start(), start_inclusive, end_inclusive)
                || range.contains(rr.end(), start_inclusive, end_inclusive)
        })
    }
}

// ─── inline dispatcher + circumfix helper ───────────────────────────

use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, StyleInfo};

impl<'ast> MdRender {
    /// Emit `InlineItem`s for one inline AST node into `layout`.
    /// Dispatches on the node's `NodeValue` to a per-kind `layout_X`.
    pub fn layout_inline(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => {}
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            NodeValue::Alert(_)
            | NodeValue::BlockQuote
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionList
            | NodeValue::Document
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::Item(_)
            | NodeValue::List(_)
            | NodeValue::MultilineBlockQuote(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::TaskItem(_)
            | NodeValue::CodeBlock(_)
            | NodeValue::DescriptionDetails
            | NodeValue::DescriptionTerm
            | NodeValue::Heading(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Paragraph
            | NodeValue::TableCell
            | NodeValue::ThematicBreak => {
                unimplemented!("not an inline: {:?}", node.data.borrow().value)
            }
            NodeValue::Subtext => unimplemented!("extension disabled"),

            NodeValue::Code(_) => self.layout_code(layout, node, range),
            NodeValue::Emph => self.layout_emph(layout, node, range),
            NodeValue::Escaped => self.layout_escaped(layout, node, range),
            NodeValue::EscapedTag(_) => self.layout_escaped_tag(layout, node, range),
            NodeValue::FootnoteReference(node_footnote_reference) => {
                let NodeFootnoteReference { ix, .. } = &**node_footnote_reference;
                self.layout_footnote_reference(layout, node, *ix, range)
            }
            NodeValue::Highlight => self.layout_highlight(layout, node, range),
            NodeValue::HtmlInline(_) => self.layout_html_inline(layout, node, range),
            NodeValue::Image(_) => self.layout_image(layout, node, range),
            NodeValue::LineBreak => self.layout_line_break(layout, node, range),
            NodeValue::Link(_) => self.layout_link(layout, node, range),
            NodeValue::Math(_) => self.layout_math(layout, node, range),
            NodeValue::ShortCode(node_short_code) => {
                self.layout_short_code(layout, node, range, node_short_code)
            }
            NodeValue::SoftBreak => self.layout_soft_break(layout, node, range),
            NodeValue::SpoileredText => self.layout_spoilered_text(layout, node, range),
            NodeValue::Strikethrough => self.layout_strikethrough(layout, node, range),
            NodeValue::Strong => self.layout_strong(layout, node, range),
            NodeValue::Subscript => self.layout_subscript(layout, node, range),
            NodeValue::Superscript => self.layout_superscript(layout, node, range),
            NodeValue::Text(_) => self.layout_text(layout, node, range),
            NodeValue::Underline => self.layout_underline(layout, node, range),
            NodeValue::WikiLink(_) => self.layout_wikilink(layout, node, range),
        }
    }

    /// Walk an inline-container node's children, dispatching each.
    /// Counterpart to `show_inline_children`.
    pub fn layout_inline_children(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        for child in node.children() {
            self.layout_inline(layout, child, range);
        }
    }

    /// Emit a circumfix inline (prefix / children / postfix bracketed
    /// by a `StyleOpen` / `StyleClose`). The visible-content `format`
    /// applies to the inner children + glued ranges; prefix/postfix
    /// syntax always uses `text_format_syntax()`. When syntax is
    /// hidden (cursor not on the node), prefix/postfix collapse to
    /// zero-visible `push_override` markers — these become row
    /// anchors so a cursor can still land "at the prefix" / "after
    /// the postfix" via hit-test.
    ///
    /// When the node has no children (comrak collapsed it — e.g. an
    /// empty `**` or stray `==`), the whole node range renders as
    /// syntax.
    pub fn layout_circumfix(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
        format: Format,
    ) {
        let node_range = self.node_range(node);
        // Multi-line paragraphs dispatch every inline child to every
        // source line. Skip out-of-range nodes here, otherwise the
        // unconditional style_open/close below pollutes the line's
        // scope stack (and walker emits stray bg-pad fragments).
        if node_range.trim(&range).is_empty() {
            return;
        }
        let any_children = node.children().next().is_some();
        if !any_children {
            if range.contains_range(&node_range, true, true) {
                let trimmed = node_range.trim(&range);
                if !trimmed.is_empty() {
                    layout.push_source(trimmed, &self.buffer[trimmed], self.text_format_syntax());
                }
            }
            return;
        }
        layout.style_open(StyleInfo::new(format, node_range));
        let reveal = self.node_revealed(node);
        if let Some(prefix_range) = self.prefix_range(node) {
            let trimmed = prefix_range.trim(&range);
            if !trimmed.is_empty() {
                if reveal {
                    layout.push_source(trimmed, &self.buffer[trimmed], self.text_format_syntax());
                } else {
                    layout.push_override(
                        prefix_range.start().into_range(),
                        "",
                        self.text_format_syntax(),
                    );
                }
            }
        }
        if let Some(infix_range) = self.infix_range(node) {
            if !infix_range.trim(&range).is_empty() {
                self.layout_inline_children(layout, node, range);
            }
        }
        if let Some(postfix_range) = self.postfix_range(node) {
            let trimmed = postfix_range.trim(&range);
            if !trimmed.is_empty() {
                if reveal {
                    layout.push_source(trimmed, &self.buffer[trimmed], self.text_format_syntax());
                } else {
                    layout.push_override(
                        postfix_range.end().into_range(),
                        "",
                        self.text_format_syntax(),
                    );
                }
            }
        }
        layout.style_close();
    }
}
