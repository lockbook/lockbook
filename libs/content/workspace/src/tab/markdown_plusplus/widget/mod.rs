use comrak::nodes::{
    AstNode, NodeFootnoteReference, NodeHeading, NodeHtmlBlock, NodeLink, NodeValue,
};
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use super::MarkdownPlusPlus;

pub mod container_block;
pub mod inline;
pub mod layout;
pub mod leaf_block;
pub mod spacing;
pub mod syntax;
pub mod utils;

pub const MARGIN: f32 = 20.0; // space between the editor and window border; must be large enough to accommodate bordered elements e.g. code blocks
pub const MAX_WIDTH: f32 = 800.0; // the maximum width of the editor before it starts adding padding

pub const INLINE_PADDING: f32 = 5.0; // the extra space granted to inline code for a border (both sides)
pub const ROW_HEIGHT: f32 = 20.0; // ...at default font size
pub const BLOCK_PADDING: f32 = 10.0; // between a table cell / code block and its contents (all sides)
pub const INDENT: f32 = 25.0; // enough space for two digits in a numbered list
pub const BULLET_RADIUS: f32 = 2.0;
pub const ROW_SPACING: f32 = 5.0; // must be large enough to accommodate bordered elements e.g. inline code
pub const BLOCK_SPACING: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct Wrap {
    pub offset: f32,
    pub width: f32,
    pub row_height: f32, // overridden by headings
}

impl Wrap {
    pub fn new(width: f32) -> Self {
        Self { offset: 0.0, width, row_height: ROW_HEIGHT }
    }

    /// The index of the current row
    pub fn row(&self) -> usize {
        (self.offset / self.width) as _
    }

    /// The start of the current row
    pub fn row_start(&self) -> f32 {
        self.row() as f32 * self.width
    }

    /// The end of the current row
    pub fn row_end(&self) -> f32 {
        self.row_start() + self.width
    }

    /// The offset from the start of the row
    pub fn row_offset(&self) -> f32 {
        self.offset - self.row_start()
    }

    /// The remaining space on the row
    pub fn row_remaining(&self) -> f32 {
        self.row_end() - self.offset
    }

    /// The height of the wrapped text; always at least [`Self::row_height`]
    pub fn height(&self) -> f32 {
        ((self.offset / self.width).ceil() * self.row_height).max(self.row_height)
    }
}

impl<'ast> MarkdownPlusPlus {
    pub fn text_format(&self, node: &AstNode<'_>) -> TextFormat {
        let parent = || node.parent().unwrap();
        let parent_text_format = || self.text_format(parent());

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => parent_text_format(),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.text_format_alert(parent(), node_alert),
            NodeValue::BlockQuote => self.text_format_block_quote(parent()),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.text_format_document(),
            NodeValue::FootnoteDefinition(_) => self.text_format_footnote_definition(parent()),
            NodeValue::Item(_) => parent_text_format(),
            NodeValue::List(_) => parent_text_format(),
            NodeValue::MultilineBlockQuote(_) => self.text_format_multiline_block_quote(parent()),
            NodeValue::Table(_) => parent_text_format(),
            NodeValue::TableRow(is_header_row) => {
                self.text_format_table_row(parent(), *is_header_row)
            }

            // inline
            NodeValue::Image(_) => self.text_format_image(parent()),
            NodeValue::Code(_) => self.text_format_code(parent()),
            NodeValue::Emph => self.text_format_emph(parent()),
            NodeValue::Escaped => self.text_format_escaped(parent()),
            NodeValue::EscapedTag(_) => self.text_format_escaped_tag(parent()),
            NodeValue::FootnoteReference(_) => self.text_format_footnote_reference(parent()),
            NodeValue::HtmlInline(_) => self.text_format_html_inline(parent()),
            NodeValue::LineBreak => parent_text_format(),
            NodeValue::Link(_) => self.text_format_link(parent()),
            NodeValue::Math(_) => self.text_format_math(parent()),
            NodeValue::SoftBreak => parent_text_format(),
            NodeValue::SpoileredText => self.text_format_spoilered_text(parent()),
            NodeValue::Strikethrough => self.text_format_strikethrough(parent()),
            NodeValue::Strong => self.text_format_strong(parent()),
            NodeValue::Subscript => self.text_format_subscript(parent()),
            NodeValue::Superscript => self.text_format_superscript(parent()),
            NodeValue::Text(_) => parent_text_format(),
            NodeValue::Underline => self.text_format_underline(parent()),
            NodeValue::WikiLink(_) => self.text_format_wiki_link(parent()),

            // leaf_block
            NodeValue::CodeBlock(_) => self.text_format_code_block(parent()),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, .. }) => {
                self.text_format_heading(parent(), *level)
            }
            NodeValue::HtmlBlock(_) => self.text_format_html_block(parent()),
            NodeValue::Paragraph => parent_text_format(),
            NodeValue::TableCell => parent_text_format(),
            NodeValue::TaskItem(_) => parent_text_format(),
            NodeValue::ThematicBreak => parent_text_format(),
        }
    }

    fn row_height(&self, node: &AstNode<'_>) -> f32 {
        let text_format = self.text_format(node).font_id;
        self.ctx.fonts(|fonts| fonts.row_height(&text_format))
    }

    pub fn height(&self, node: &'ast AstNode<'ast>) -> f32 {
        if let Some(cached) = self.get_cached_node_height(node) {
            return cached;
        }

        // container blocks: if revealed, show source lines instead
        if node.parent().is_some()
            && !node.data.borrow().value.contains_inlines()
            && !matches!(node.data.borrow().value, NodeValue::CodeBlock(_))
            && self.reveal(node)
        {
            let mut height = 0.;

            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
                let postfix = (line.start() + prefix_len, line.end());

                let mut wrap = Wrap::new(self.width(node));
                wrap.offset += self.span_text_line(&wrap, postfix, self.text_format_syntax(node)); // todo: match row height?

                height += wrap.height();
                height += BLOCK_SPACING;
            }
            if height > 0. {
                height -= BLOCK_SPACING;
            }

            return height;
        }

        let height = match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => self.height_alert(node),
            NodeValue::BlockQuote => self.height_block_quote(node),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.block_children_height(node),
            NodeValue::FootnoteDefinition(_) => self.height_footnote_definition(node),
            NodeValue::Item(_) => self.height_item(node),
            NodeValue::List(_) => self.block_children_height(node),
            NodeValue::MultilineBlockQuote(_) => self.height_multiline_block_quote(node),
            NodeValue::Table(_) => self.block_children_height(node),
            NodeValue::TableRow(_) => self.height_table_row(node),
            NodeValue::TaskItem(_) => self.block_children_height(node),

            // inline
            NodeValue::Image(NodeLink { url, .. }) => self.height_image(node, url), // used when rendering the image itself
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Subscript => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => self.height_code_block(node, node_code_block),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext, .. }) => {
                self.height_heading(node, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => self.height_html_block(node),
            NodeValue::Paragraph => self.height_paragraph(node),
            NodeValue::TableCell => self.height_table_cell(node),
            NodeValue::ThematicBreak => self.height_thematic_break(),
        };

        self.set_cached_node_height(node, height);

        height
    }

    // the height of a block that contains blocks is the sum of the heights of the blocks it contains
    fn block_children_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let mut height_sum = 0.0;
        for child in node.children() {
            height_sum += self.block_pre_spacing_height(child);
            height_sum += self.height(child);
            height_sum += self.block_post_spacing_height(child);
        }
        height_sum
    }

    pub fn indent(&self, node: &'ast AstNode<'ast>) -> f32 {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => INDENT,
            NodeValue::BlockQuote => INDENT,
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => 0.,
            NodeValue::FootnoteDefinition(_) => INDENT,
            NodeValue::Item(_) => INDENT,
            NodeValue::List(_) => 0., // indentation handled by items
            NodeValue::MultilineBlockQuote(_) => INDENT,
            NodeValue::Table(_) => 0.,
            NodeValue::TableRow(_) => 0.,
            NodeValue::TaskItem(_) => INDENT,

            // inline
            NodeValue::Image(_) => unimplemented!("not a block"),
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Subscript => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not a container block"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block"),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block"),
            NodeValue::Paragraph => unimplemented!("not a container block"),
            NodeValue::TableCell => unimplemented!("not a container block"),
            NodeValue::ThematicBreak => unimplemented!("not a container block"),
        }
    }

    pub fn width(&self, node: &'ast AstNode<'ast>) -> f32 {
        let parent = || node.parent().unwrap();
        let parent_width = || self.width(parent());
        let parent_indent = || self.indent(parent());
        let indented_width = || parent_width() - parent_indent();

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(_) => indented_width(),
            NodeValue::BlockQuote => indented_width(),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.width,
            NodeValue::FootnoteDefinition(_) => indented_width(),
            NodeValue::Item(_) => indented_width(),
            NodeValue::List(_) => indented_width(), // indentation handled by items
            NodeValue::MultilineBlockQuote(_) => indented_width(),
            NodeValue::Table(_) => indented_width(),
            NodeValue::TableRow(_) => indented_width(),
            NodeValue::TaskItem(_) => indented_width(),

            // inline
            NodeValue::Image(_) => unimplemented!("not a block"),
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Subscript => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(_) => indented_width(),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => indented_width(),
            NodeValue::HtmlBlock(_) => indented_width(),
            NodeValue::Paragraph => indented_width(),
            NodeValue::TableCell => 1.0 / node.parent().unwrap().children().count() as f32,
            NodeValue::ThematicBreak => indented_width(),
        }
    }

    pub(crate) fn show_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        // container blocks: if revealed, show source lines instead
        if node.parent().is_some()
            && !node.data.borrow().value.contains_inlines()
            && !matches!(node.data.borrow().value, NodeValue::CodeBlock(_))
            && self.reveal(node)
        {
            for line in self.node_lines(node).iter() {
                let parent = node.parent().unwrap();

                let line = self.bounds.source_lines[line];
                let prefix_len = self.line_prefix_len(parent, line);
                let postfix = (line.start() + prefix_len, line.end());

                let mut wrap = Wrap::new(self.width(node));
                self.show_text_line(
                    ui,
                    top_left,
                    &mut wrap,
                    postfix,
                    self.text_format_syntax(node),
                    false,
                );

                self.bounds.paragraphs.push(postfix);

                top_left.y += wrap.height();
                top_left.y += BLOCK_SPACING;
            }

            return;
        }

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => {}
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.show_alert(ui, node, top_left, node_alert),
            NodeValue::BlockQuote => self.show_block_quote(ui, node, top_left),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.show_document(ui, node, top_left),
            NodeValue::FootnoteDefinition(_) => self.show_footnote_definition(ui, node, top_left),
            NodeValue::Item(_) => self.show_item(ui, node, top_left),
            NodeValue::List(_) => self.show_block_children(ui, node, top_left),
            NodeValue::MultilineBlockQuote(_) => {
                self.show_multiline_block_quote(ui, node, top_left)
            }
            NodeValue::Table(_) => self.show_table(ui, node, top_left),
            NodeValue::TableRow(is_header_row) => {
                self.show_table_row(ui, node, top_left, *is_header_row)
            }
            NodeValue::TaskItem(maybe_check) => {
                self.show_task_item(ui, node, top_left, *maybe_check)
            }

            // inline
            NodeValue::Image(NodeLink { url, .. }) => {
                self.show_image_block(ui, node, top_left, url)
            }
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Subscript => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => {
                self.show_code_block(ui, node, top_left, node_code_block)
            }
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext }) => {
                self.show_heading(ui, node, top_left, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => self.show_html_block(ui, node, top_left),
            NodeValue::Paragraph => self.show_paragraph(ui, node, top_left),
            NodeValue::TableCell => self.show_table_cell(ui, node, top_left),
            NodeValue::ThematicBreak => self.show_thematic_break(ui, node, top_left),
        }
    }

    // blocks are stacked vertically
    fn show_block_children(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let mut children: Vec<_> = node.children().collect();
        children.sort_by_key(|child| child.data.borrow().sourcepos);
        for child in children {
            // add pre-spacing
            let pre_spacing = self.block_pre_spacing_height(child);
            self.show_block_pre_spacing(ui, child, top_left);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(
            //         top_left,
            //         egui::Vec2::new(self.width(child), pre_spacing),
            //     ),
            //     2.,
            //     egui::Stroke::new(pre_spacing.min(1.), self.theme.bg().neutral_quarternary),
            // );
            // println!("{}pre_spacing: {}", "  ".repeat(node.ancestors().count() - 1), pre_spacing);

            top_left.y += pre_spacing;

            // add block
            let child_height = self.height(child);
            self.show_block(ui, child, top_left);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(
            //         top_left,
            //         egui::Vec2::new(self.width(child), child_height),
            //     ),
            //     2.,
            //     egui::Stroke::new(1., self.theme.bg().green),
            // );
            // println!("{}child_height: {}", "  ".repeat(node.ancestors().count() - 1), child_height);

            top_left.y += child_height;

            // add post-spacing
            let post_spacing = self.block_post_spacing_height(child);
            self.show_block_post_spacing(ui, child, top_left);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(
            //         top_left,
            //         egui::Vec2::new(self.width(child), post_spacing),
            //     ),
            //     2.,
            //     egui::Stroke::new(post_spacing.min(1.), self.theme.bg().neutral_quarternary),
            // );
            // println!("{}post_spacing: {}", "  ".repeat(node.ancestors().count() - 1), post_spacing);

            top_left.y += post_spacing;
        }

        // debug
        // ui.painter()
        //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.theme.bg().tertiary));
    }

    fn span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
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
            NodeValue::MultilineBlockQuote(_) => unimplemented!("not an inline"),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),

            // inline
            NodeValue::Image(_) => self.span_image(node, wrap),
            NodeValue::Code(_) => self.span_code(node, wrap),
            NodeValue::Emph => self.span_emph(node, wrap),
            NodeValue::Escaped => self.span_escaped(node, wrap),
            NodeValue::EscapedTag(_) => self.span_escaped_tag(node, wrap),
            NodeValue::FootnoteReference(NodeFootnoteReference { ix, .. }) => {
                self.span_footnote_reference(node, wrap, *ix)
            }
            NodeValue::HtmlInline(html) => self.span_html_inline(node, wrap, html),
            NodeValue::LineBreak => self.span_line_break(wrap),
            NodeValue::Link(_) => self.span_link(node, wrap),
            NodeValue::Math(_) => self.span_math(node, wrap),
            NodeValue::SoftBreak => self.span_soft_break(wrap),
            NodeValue::SpoileredText => self.span_spoilered_text(node, wrap),
            NodeValue::Strikethrough => self.span_strikethrough(node, wrap),
            NodeValue::Strong => self.span_strong(node, wrap),
            NodeValue::Subscript => self.span_subscript(node, wrap),
            NodeValue::Superscript => self.span_superscript(node, wrap),
            NodeValue::Text(text) => self.span_node_text_line(node, wrap, text),
            NodeValue::Underline => self.span_underline(node, wrap),
            NodeValue::WikiLink(_) => self.span_wikilink(node, wrap),

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

    // the span of an inline that contains inlines is the sum of the spans of
    // the inlines
    fn inline_children_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        let mut tmp_wrap = wrap.clone();
        for child in node.children() {
            tmp_wrap.offset += self.span(child, &tmp_wrap);
        }
        tmp_wrap.offset - wrap.offset
    }

    // the size of a block that contains inlines is the span of the inlines
    // divided by the wrap width (rounded up), times the row height (plus
    // spacing)
    fn inline_children_height(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        let children_span = self.inline_children_span(node, &Wrap::new(width));
        let rows = (children_span / width).ceil();
        rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
    }

    fn show_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => {}
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
            NodeValue::MultilineBlockQuote(_) => unimplemented!("not an inline"),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),
            NodeValue::TaskItem(_) => unimplemented!("not an inline"),

            // inline
            NodeValue::Image(_) => self.show_image(ui, node, top_left, wrap),
            NodeValue::Code(_) => self.show_code(ui, node, top_left, wrap),
            NodeValue::Emph => self.show_emph(ui, node, top_left, wrap),
            NodeValue::Escaped => self.show_escaped(ui, node, top_left, wrap),
            NodeValue::EscapedTag(_) => self.show_escaped_tag(ui, node, top_left, wrap),
            NodeValue::FootnoteReference(NodeFootnoteReference { ix, .. }) => {
                self.show_footnote_reference(ui, node, top_left, wrap, *ix)
            }
            NodeValue::HtmlInline(_) => self.show_html_inline(ui, node, top_left, wrap),
            NodeValue::LineBreak => self.show_line_break(wrap),
            NodeValue::Link(_) => self.show_link(ui, node, top_left, wrap),
            NodeValue::Math(_) => self.show_math(ui, node, top_left, wrap),
            NodeValue::SoftBreak => self.show_soft_break(wrap),
            NodeValue::SpoileredText => self.show_spoilered_text(ui, node, top_left, wrap),
            NodeValue::Strikethrough => self.show_strikethrough(ui, node, top_left, wrap),
            NodeValue::Strong => self.show_strong(ui, node, top_left, wrap),
            NodeValue::Subscript => self.show_subscript(ui, node, top_left, wrap),
            NodeValue::Superscript => self.show_superscript(ui, node, top_left, wrap),
            NodeValue::Text(_) => self.show_text(ui, node, top_left, wrap),
            NodeValue::Underline => self.show_underline(ui, node, top_left, wrap),
            NodeValue::WikiLink(_) => self.show_wikilink(ui, node, top_left, wrap),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not an inline"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not an inline"),
            NodeValue::HtmlBlock(_) => unimplemented!("not an inline"),
            NodeValue::Paragraph => unimplemented!("not an inline"),
            NodeValue::TableCell => unimplemented!("not an inline"),
            NodeValue::ThematicBreak => unimplemented!("not an inline"),
        }
    }

    // inlines are stacked horizontally and wrapped
    fn show_inline_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        for child in node.children() {
            self.show_inline(ui, child, top_left, wrap);
        }
    }

    /// Returns the range between the start of the node and the start of its
    /// first child, if there is one.
    fn prefix_range(&self, node: &'ast AstNode<'ast>) -> Option<(DocCharOffset, DocCharOffset)> {
        let range = self.node_range(node);
        let first_child = node.children().next()?;
        let first_child_range = self.sourcepos_to_range(first_child.data.borrow().sourcepos);
        Some((range.start(), first_child_range.start()))
    }

    /// Returns the range between the end of the node's last child if there is
    /// one, and the end of the node.
    fn postfix_range(&self, node: &'ast AstNode<'ast>) -> Option<(DocCharOffset, DocCharOffset)> {
        let range = self.node_range(node);
        let last_child = node.children().last()?;
        let last_child_range = self.sourcepos_to_range(last_child.data.borrow().sourcepos);
        Some((last_child_range.end(), range.end()))
    }

    /// Returns the range between the start of the node's first child and the
    /// end of it's last child, if there are any children. For many nodes, this
    /// is the content in the node.
    fn infix_range(&self, node: &'ast AstNode<'ast>) -> Option<(DocCharOffset, DocCharOffset)> {
        let first_child = node.children().next()?;
        let first_child_range = self.sourcepos_to_range(first_child.data.borrow().sourcepos);
        let last_child = node.children().last()?;
        let last_child_range = self.sourcepos_to_range(last_child.data.borrow().sourcepos);
        Some((first_child_range.start(), last_child_range.end()))
    }

    /// Returns the portion of the line that's within the node, exluding line
    /// prefixes due to parent nodes.
    pub fn node_line(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> (DocCharOffset, DocCharOffset) {
        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        (line.start() + parent_prefix_len, line.end())
    }

    /// Returns 5 ranges representing the pre-node line, pre-first-child section,
    /// inter-children section, post-last-child section, and post-node line.
    /// Returns None if there are no children on this line.
    #[allow(clippy::type_complexity)]
    fn line_ranges(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> Option<(
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
    )> {
        let children = self.children_in_line(node, line);
        if children.is_empty() {
            return None;
        }

        let node_line = self.node_line(node, line);

        let node_range = self.node_range(node);
        let node_range =
            (node_range.start().max(node_line.start()), node_range.end().min(node_line.end()));

        let first_child = children.first().unwrap();
        let first_child_range = self.node_range(first_child);
        let last_child = children.last().unwrap();
        let last_child_range = self.node_range(last_child);

        let pre_node_line = (node_line.start(), node_range.start());
        let pre_first_child = (node_range.start(), first_child_range.start());
        let inter_children = (first_child_range.start(), last_child_range.end());
        let post_last_child = (last_child_range.end(), node_range.end());
        let post_node_line = (node_range.end(), node_line.end());

        Some((pre_node_line, pre_first_child, inter_children, post_last_child, post_node_line))
    }

    fn prefix_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        if let Some(prefix_range) = self.prefix_range(node) {
            self.span_text_line(wrap, prefix_range, self.text_format_syntax(node))
        } else {
            0.
        }
    }

    fn postfix_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        if let Some(postfix_range) = self.postfix_range(node) {
            self.span_text_line(wrap, postfix_range, self.text_format_syntax(node))
        } else {
            0.
        }
    }

    fn circumfix_span(&self, node: &'ast AstNode<'ast>, wrap: &Wrap) -> f32 {
        let mut tmp_wrap = wrap.clone();
        if self.node_intersects_selection(node) {
            tmp_wrap.offset += self.prefix_span(node, &tmp_wrap);
        }
        tmp_wrap.offset += self.inline_children_span(node, &tmp_wrap);
        if self.node_intersects_selection(node) {
            tmp_wrap.offset += self.postfix_span(node, &tmp_wrap);
        }
        tmp_wrap.offset - wrap.offset
    }

    fn show_circumfix(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
    ) {
        if self.node_intersects_selection(node) {
            if let Some(prefix_range) = self.prefix_range(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    prefix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
        }

        self.show_inline_children(ui, node, top_left, wrap);

        if self.node_intersects_selection(node) {
            if let Some(postfix_range) = self.postfix_range(node) {
                self.show_text_line(
                    ui,
                    top_left,
                    wrap,
                    postfix_range,
                    self.text_format_syntax(node),
                    false,
                );
            }
        }
    }

    /// How many leading characters on this line belong to the given node and
    /// its ancestors?
    // "It is tempting to think of this in terms of columns: the continuation
    // blocks must be indented at least to the column of the first
    // non-whitespace character after the list marker. However, that is not
    // quite right. The spaces after the list marker determine how much relative
    // indentation is needed. Which column this indentation reaches will depend
    // on how the list item is embedded in other constructions, as shown by this
    // example:
    //
    //    > > 1.  one
    // >>
    // >>     two
    //
    // Here two occurs in the same column as the list marker 1., but is actually
    // contained in the list item, because there is sufficient indentation after
    // the last containing blockquote marker."
    //
    // https://github.github.com/gfm/#list-items
    fn line_prefix_len(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        let parent = || node.parent().unwrap();
        let parent_line_prefix_len = || self.line_prefix_len(parent(), line);

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => unimplemented!("not a block"),
            NodeValue::Raw(_) => unimplemented!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.line_prefix_len_alert(node, line, node_alert),
            NodeValue::BlockQuote => self.line_prefix_len_block_quote(node, line),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => 0.into(),
            NodeValue::FootnoteDefinition(_) => {
                self.line_prefix_len_footnote_definition(node, line)
            }
            NodeValue::Item(node_list) => self.line_prefix_len_item(node, line, node_list),
            NodeValue::List(_) => parent_line_prefix_len(),
            NodeValue::MultilineBlockQuote(node_multiline_block_quote) => {
                self.line_prefix_len_multiline_block_quote(node, node_multiline_block_quote, line)
            }
            NodeValue::Table(_) => parent_line_prefix_len(),
            NodeValue::TableRow(_) => self.line_prefix_len_table_row(node, line),
            NodeValue::TaskItem(_) => self.line_prefix_len_task_item(node, line),

            // inline
            NodeValue::Image(NodeLink { .. }) => unimplemented!("not a block"),
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Subscript => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not a container block"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a container block"),
            NodeValue::HtmlBlock(_) => unimplemented!("not a container block"),
            NodeValue::Paragraph => unimplemented!("not a container block"),
            NodeValue::TableCell => unimplemented!("not a container block"),
            NodeValue::ThematicBreak => unimplemented!("not a container block"),
        }
    }

    fn line_prefix_intersects_selection(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> bool {
        let line_prefix = (line.start(), line.start() + self.line_prefix_len(node, line));
        if line_prefix.is_empty() {
            return false;
        }

        let selection = self.buffer.current.selection;

        line_prefix.intersects(&selection, false) || selection.end() == line_prefix.start()
    }

    /// returns true if the syntax for a container block should be revealed
    fn reveal(&self, node: &'ast AstNode<'ast>) -> bool {
        for line in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line];
            if self.line_prefix_intersects_selection(node, line) {
                return true;
            }
        }
        false
    }
}
