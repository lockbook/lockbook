use comrak::nodes::{AstNode, NodeFootnoteReference, NodeValue};
use egui::{Pos2, Sense, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

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

impl<'ast> Editor {
    pub fn span(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => unimplemented!("not an inline"),
            NodeValue::BlockQuote => unimplemented!("not an inline"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => unimplemented!("not an inline"),
            NodeValue::FootnoteDefinition(_) => unimplemented!("not an inline"),
            NodeValue::Item(_) => unimplemented!("not an inline"),
            NodeValue::List(_) => unimplemented!("not an inline"),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),

            // inline
            NodeValue::Code(_) => self.span_code(node, wrap, range),
            NodeValue::Emph => self.span_emph(node, wrap, range),
            NodeValue::Escaped => self.span_escaped(node, wrap, range),
            NodeValue::EscapedTag(_) => self.span_escaped_tag(node, wrap, range),
            NodeValue::FootnoteReference(node_footnote_reference) => {
                let NodeFootnoteReference { ix, .. } = &**node_footnote_reference;
                self.span_footnote_reference(node, wrap, *ix, range)
            }
            NodeValue::Highlight => self.span_highlight(node, wrap, range),
            NodeValue::HtmlInline(_) => self.span_html_inline(node, wrap, range),
            NodeValue::Image(_) => self.span_image(node, wrap, range),
            NodeValue::LineBreak => self.span_line_break(node, wrap, range),
            NodeValue::Link(_) => self.span_link(node, wrap, range),
            NodeValue::Math(_) => self.span_math(node, wrap, range),
            NodeValue::ShortCode(node_short_code) => {
                self.span_short_code(node, wrap, range, node_short_code)
            }
            NodeValue::SoftBreak => self.span_soft_break(node, wrap, range),
            NodeValue::SpoileredText => self.span_spoilered_text(node, wrap, range),
            NodeValue::Strikethrough => self.span_strikethrough(node, wrap, range),
            NodeValue::Strong => self.span_strong(node, wrap, range),
            NodeValue::Subscript => self.span_subscript(node, wrap, range),
            NodeValue::Subtext => unimplemented!("extension disabled"),
            NodeValue::Superscript => self.span_superscript(node, wrap, range),
            NodeValue::Text(_) => self.span_text(node, wrap, range),
            NodeValue::Underline => self.span_underline(node, wrap, range),
            NodeValue::WikiLink(_) => self.span_wikilink(node, wrap, range),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not an inline"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not an inline"),
            NodeValue::HtmlBlock(_) => unimplemented!("not an inline"),
            NodeValue::Paragraph => unimplemented!("not an inline"),
            NodeValue::TableCell => unimplemented!("not an inline"),
            NodeValue::TaskItem(_) => unimplemented!("not an inline"),
            NodeValue::ThematicBreak => unimplemented!("not an inline"),
        }
    }

    pub fn show_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let ui = &mut self.node_ui(ui, node);

        let span = self.span(node, wrap, range);
        let pre_offset = wrap.offset;

        let response = match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => Default::default(),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => unimplemented!("not an inline"),
            NodeValue::BlockQuote => unimplemented!("not an inline"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => unimplemented!("not an inline"),
            NodeValue::FootnoteDefinition(_) => unimplemented!("not an inline"),
            NodeValue::Item(_) => unimplemented!("not an inline"),
            NodeValue::List(_) => unimplemented!("not an inline"),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),
            NodeValue::TaskItem(_) => unimplemented!("not an inline"),

            // inline
            NodeValue::Code(_) => self.show_code(ui, node, top_left, wrap, range),
            NodeValue::Emph => self.show_emph(ui, node, top_left, wrap, range),
            NodeValue::Escaped => self.show_escaped(ui, node, top_left, wrap, range),
            NodeValue::EscapedTag(_) => self.show_escaped_tag(ui, node, top_left, wrap, range),
            NodeValue::FootnoteReference(node_footnote_reference) => {
                let NodeFootnoteReference { ix, .. } = &**node_footnote_reference;
                self.show_footnote_reference(ui, node, top_left, wrap, *ix, range)
            }
            NodeValue::Highlight => self.show_highlight(ui, node, top_left, wrap, range),
            NodeValue::HtmlInline(_) => self.show_html_inline(ui, node, top_left, wrap, range),
            NodeValue::Image(node_link) => {
                self.show_image(ui, node, top_left, wrap, node_link, range)
            }
            NodeValue::LineBreak => self.show_line_break(node, wrap, range),
            NodeValue::Link(node_link) => {
                self.show_link(ui, node, top_left, wrap, node_link, range)
            }
            NodeValue::Math(_) => self.show_math(ui, node, top_left, wrap, range),
            NodeValue::ShortCode(node_short_code) => {
                self.show_short_code(ui, node, top_left, wrap, range, node_short_code)
            }
            NodeValue::SoftBreak => self.show_soft_break(node, wrap, range),
            NodeValue::SpoileredText => self.show_spoilered_text(ui, node, top_left, wrap, range),
            NodeValue::Strikethrough => self.show_strikethrough(ui, node, top_left, wrap, range),
            NodeValue::Strong => self.show_strong(ui, node, top_left, wrap, range),
            NodeValue::Subscript => self.show_subscript(ui, node, top_left, wrap, range),
            NodeValue::Subtext => unimplemented!("extension disabled"),
            NodeValue::Superscript => self.show_superscript(ui, node, top_left, wrap, range),
            NodeValue::Text(_) => self.show_text(ui, node, top_left, wrap, range),
            NodeValue::Underline => self.show_underline(ui, node, top_left, wrap, range),
            NodeValue::WikiLink(node_wiki_link) => {
                self.show_wikilink(ui, node, top_left, wrap, node_wiki_link, range)
            }

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not an inline"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not an inline"),
            NodeValue::HtmlBlock(_) => unimplemented!("not an inline"),
            NodeValue::Paragraph => unimplemented!("not an inline"),
            NodeValue::TableCell => unimplemented!("not an inline"),
            NodeValue::ThematicBreak => unimplemented!("not an inline"),
        };

        let post_offset = wrap.offset;
        if (span - (post_offset - pre_offset)).abs() > 0.01 && self.debug {
            println!(
                "SPAN MISMATCH: {:?} vs {:?} {:?}",
                span,
                post_offset - pre_offset,
                node.data.borrow().value
            );
        }

        response
    }

    #[allow(clippy::only_used_in_recursion)]
    pub fn sense_inline(&self, ui: &Ui, node: &'ast AstNode<'ast>) -> Sense {
        match &node.data.borrow().value {
            NodeValue::Link(_) | NodeValue::WikiLink(_) | NodeValue::Image(_) => {
                let is_mobile = ui.ctx().os() == egui::os::OperatingSystem::Android
                    || ui.ctx().os() == egui::os::OperatingSystem::IOS;
                let clickable = if is_mobile { false } else { ui.input(|i| i.modifiers.command) };
                Sense { click: clickable, drag: false, focusable: false }
            }
            _ => {
                if let Some(parent) = node.parent() {
                    self.sense_inline(ui, parent)
                } else {
                    Sense { click: false, drag: false, focusable: false }
                }
            }
        }
    }

    // the span of an inline that contains inlines is the sum of the spans of
    // the inlines
    pub fn inline_children_span(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let mut tmp_wrap = wrap.clone();
        for child in node.children() {
            tmp_wrap.offset += self.span(child, &tmp_wrap, range);
        }
        tmp_wrap.offset - wrap.offset
    }

    // inlines are stacked horizontally and wrapped
    pub fn show_inline_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let mut response = Default::default();
        for child in node.children() {
            response |= self.show_inline(ui, child, top_left, wrap, range);
        }
        response
    }

    /// Returns the range between the start of the node and the start of its
    /// first child, if there is one.
    pub fn prefix_range(
        &self, node: &'ast AstNode<'ast>,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let range = self.node_range(node);
        let first_child = node.children().next()?;
        let first_child_range = self.node_range(first_child);
        Some((range.start(), first_child_range.start()))
    }

    pub fn prefix_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        if let Some(prefix_range) = self.prefix_range(node) {
            self.span_section(wrap, prefix_range, self.text_format_syntax(node))
        } else {
            0.
        }
    }

    /// Returns the range between the end of the node's last child if there is
    /// one, and the end of the node.
    pub fn postfix_range(
        &self, node: &'ast AstNode<'ast>,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let range = self.node_range(node);
        let last_child = node.children().last()?;
        let last_child_range = self.node_range(last_child);
        Some((last_child_range.end(), range.end()))
    }

    pub fn postfix_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        if let Some(postfix_range) = self.postfix_range(node) {
            self.span_section(wrap, postfix_range, self.text_format_syntax(node))
        } else {
            0.
        }
    }

    /// Returns the range between the start of the node's first child and the
    /// end of it's last child, if there are any children. For many nodes, this
    /// is the content in the node.
    pub fn infix_range(&self, node: &'ast AstNode<'ast>) -> Option<(DocCharOffset, DocCharOffset)> {
        let first_child = node.children().next()?;
        let first_child_range = self.node_range(first_child);
        let last_child = node.children().last()?;
        let last_child_range = self.node_range(last_child);
        Some((first_child_range.start(), last_child_range.end()))
    }

    pub fn circumfix_span(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let mut tmp_wrap = wrap.clone();

        let any_children = node.children().next().is_some();
        if any_children {
            let reveal = self.node_intersects_selection(node);
            if reveal {
                if let Some(prefix_range) = self.prefix_range(node) {
                    if !prefix_range.trim(&range).is_empty() {
                        tmp_wrap.offset += self.prefix_span(node, &tmp_wrap);
                    }
                }
            }
            if let Some(infix_range) = self.infix_range(node) {
                if !infix_range.trim(&range).is_empty() {
                    tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap, range);
                }
            }
            if reveal {
                if let Some(postfix_range) = self.postfix_range(node) {
                    if !postfix_range.trim(&range).is_empty() {
                        tmp_wrap.offset += self.postfix_span(node, &tmp_wrap);
                    }
                }
            }
        } else {
            let node_range = self.node_range(node);
            if !node_range.trim(&range).is_empty() {
                tmp_wrap.offset +=
                    self.span_section(wrap, node_range, self.text_format_syntax(node))
            }
        }

        tmp_wrap.offset - wrap.offset
    }

    pub fn show_circumfix(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let mut response = Default::default();
        let any_children = node.children().next().is_some();
        if any_children {
            let reveal = self.node_intersects_selection(node);
            if let Some(prefix_range) = self.prefix_range(node) {
                if !prefix_range.trim(&range).is_empty() {
                    if reveal {
                        self.show_section(
                            ui,
                            top_left,
                            wrap,
                            prefix_range,
                            self.text_format_syntax(node),
                            false,
                        );
                    } else {
                        // when syntax is captured, show an empty range
                        // representing the beginning of the prefix, so that clicking
                        // at the start of the circumfix places the cursor before
                        // the syntax
                        self.show_section(
                            ui,
                            top_left,
                            wrap,
                            prefix_range.start().into_range(),
                            self.text_format_syntax(node),
                            false,
                        );
                    }
                }
            }
            if let Some(infix_range) = self.infix_range(node) {
                if !infix_range.trim(&range).is_empty() {
                    response |= self.show_inline_children(ui, node, top_left, wrap, range);
                }
            }
            if let Some(postfix_range) = self.postfix_range(node) {
                if !postfix_range.trim(&range).is_empty() {
                    if reveal {
                        self.show_section(
                            ui,
                            top_left,
                            wrap,
                            postfix_range,
                            self.text_format_syntax(node),
                            false,
                        );
                    } else {
                        // when syntax is captured, show an empty range
                        // representing the end of the postfix, so that clicking
                        // at the end of the circumfix places the cursor after
                        // the syntax
                        self.show_section(
                            ui,
                            top_left,
                            wrap,
                            postfix_range.end().into_range(),
                            self.text_format_syntax(node),
                            false,
                        );
                    }
                }
            }
        } else {
            let node_range = self.node_range(node);
            if range.contains_range(&node_range, true, true) {
                response |= self.show_section(
                    ui,
                    top_left,
                    wrap,
                    node_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
        }

        response
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
}
