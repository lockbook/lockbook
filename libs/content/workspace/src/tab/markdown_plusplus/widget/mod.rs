use comrak::nodes::{
    AstNode, NodeCode, NodeHeading, NodeHtmlBlock, NodeLink, NodeList, NodeMath, NodeValue,
};
use egui::{Pos2, TextFormat, Ui};
use inline::text;

use super::MarkdownPlusPlus;

pub(crate) mod container_block;
pub(crate) mod inline;
pub(crate) mod leaf_block;
pub(crate) mod spacing;

pub const MARGIN: f32 = 20.0; // space between the editor and window border; must be large enough to accomodate bordered elements e.g. code blocks
pub const MAX_WIDTH: f32 = 800.0; // the maximum width of the editor before it starts adding padding

pub const INLINE_PADDING: f32 = 5.0; // the extra space granted to inline code for a border (both sides)
pub const ROW_HEIGHT: f32 = 20.0; // ...at default font size
pub const BLOCK_PADDING: f32 = 10.0; // between a table cell / code block and its contents (all sides)
pub const INDENT: f32 = 25.0; // enough space for two digits in a numbered list
pub const BULLET_RADIUS: f32 = 2.0;
pub const ROW_SPACING: f32 = 5.0; // must be large enough to accomodate bordered elements e.g. inline code
pub const BLOCK_SPACING: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct WrapContext {
    pub offset: f32,
    pub width: f32,
}

impl WrapContext {
    pub fn new(width: f32) -> Self {
        Self { offset: 0.0, width }
    }

    /// The index of the current line
    pub fn line(&self) -> usize {
        (self.offset / self.width) as _
    }

    /// The start of the current line
    pub fn line_start(&self) -> f32 {
        self.line() as f32 * self.width
    }

    /// The end of the current line
    pub fn line_end(&self) -> f32 {
        self.line_start() + self.width
    }

    /// The offset from the start of the line
    pub fn line_offset(&self) -> f32 {
        self.offset - self.line_start()
    }

    /// The remaining space on the line
    pub fn line_remaining(&self) -> f32 {
        self.line_end() - self.offset
    }
}

impl<'ast> MarkdownPlusPlus {
    pub fn text_format(&self, node: &AstNode<'_>) -> TextFormat {
        // lazy fields that are not invoked for document node which has no parent
        let parent = || node.parent().unwrap();
        let parent_text_format = || self.text_format(parent());

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => parent_text_format(),

            // container_block
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
            NodeValue::Escaped => parent_text_format(),
            NodeValue::EscapedTag(_) => parent_text_format(),
            NodeValue::FootnoteReference(_) => self.text_format_footnote_reference(parent()),
            NodeValue::HtmlInline(_) => self.text_format_html_inline(parent()),
            NodeValue::LineBreak => parent_text_format(),
            NodeValue::Link(_) => self.text_format_link(parent()),
            NodeValue::Math(_) => self.text_format_math(parent()),
            NodeValue::SoftBreak => parent_text_format(),
            NodeValue::SpoileredText => self.text_format_spoilered_text(parent()),
            NodeValue::Strikethrough => self.text_format_strikethrough(parent()),
            NodeValue::Strong => self.text_format_strong(parent()),
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

    pub fn height(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,

            // container_block
            NodeValue::BlockQuote => self.height_block_quote(node, width),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.block_children_height(node, width),
            NodeValue::FootnoteDefinition(_) => self.height_footnote_definition(node, width),
            NodeValue::Item(_) => self.height_item(node, width),
            NodeValue::List(_) => self.block_children_height(node, width),
            NodeValue::MultilineBlockQuote(_) => self.height_multiline_block_quote(node, width),
            NodeValue::Table(_) => self.block_children_height(node, width),
            NodeValue::TableRow(_) => self.height_table_row(node, width),
            NodeValue::TaskItem(_) => self.block_children_height(node, width),

            // inline
            NodeValue::Image(NodeLink { url, .. }) => self.height_image(width, url), // used when rendering the image itself
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
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => {
                self.height_code_block(node, width, node_code_block)
            }
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext, .. }) => {
                self.height_heading(node, width, *level, *setext)
            }
            NodeValue::HtmlBlock(NodeHtmlBlock { literal, .. }) => {
                self.height_html_block(node, width, literal)
            }
            NodeValue::Paragraph => self.height_paragraph(node, width),
            NodeValue::TableCell => self.height_table_cell(node, width),
            NodeValue::ThematicBreak => self.height_thematic_break(),
        }
    }

    // the height of a block that contains blocks is the sum of the heights of the blocks it contains
    fn block_children_height(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        let mut height_sum = 0.0;
        for child in node.children() {
            height_sum += self.block_pre_spacing_height(child);
            height_sum += self.height(child, width);
            height_sum += self.block_post_spacing_height(child);
        }
        height_sum
    }

    pub(crate) fn show_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
    ) {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => {}

            // container_block
            NodeValue::BlockQuote => self.show_block_quote(ui, node, top_left, width),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.show_document(ui, node, top_left, width),
            NodeValue::FootnoteDefinition(_) => {
                self.show_footnote_definition(ui, node, top_left, width)
            }
            NodeValue::Item(NodeList { list_type, start, .. }) => {
                self.show_item(ui, node, top_left, width, *list_type, *start)
            }
            NodeValue::List(_) => self.show_block_children(ui, node, top_left, width),
            NodeValue::MultilineBlockQuote(_) => {
                self.show_multiline_block_quote(ui, node, top_left, width)
            }
            NodeValue::Table(_) => self.show_table(ui, node, top_left, width),
            NodeValue::TableRow(is_header_row) => {
                self.show_table_row(ui, node, top_left, width, *is_header_row)
            }
            NodeValue::TaskItem(maybe_check) => {
                self.show_task_item(ui, node, top_left, width, *maybe_check)
            }

            // inline
            NodeValue::Image(NodeLink { url, .. }) => {
                self.show_image_block(ui, top_left, width, url)
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
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),

            // leaf_block
            NodeValue::CodeBlock(node_code_block) => {
                self.show_code_block(ui, node, top_left, width, node_code_block)
            }
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext }) => {
                self.show_heading(ui, node, top_left, width, *level, *setext)
            }
            NodeValue::HtmlBlock(NodeHtmlBlock { literal, .. }) => {
                self.show_html_block(ui, node, top_left, width, literal)
            }
            NodeValue::Paragraph => {
                self.show_paragraph(ui, node, top_left, &mut WrapContext::new(width))
            }
            NodeValue::TableCell => {
                self.show_table_cell(ui, node, top_left, &mut WrapContext::new(width))
            }
            NodeValue::ThematicBreak => self.show_thematic_break(ui, top_left, width),
        }
    }

    // blocks are stacked vertically
    fn show_block_children(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
    ) {
        let mut children: Vec<_> = node.children().collect();
        children.sort_by_key(|child| child.data.borrow().sourcepos);
        for child in children {
            // add pre-spacing
            let pre_spacing = self.block_pre_spacing_height(child);
            self.show_block_pre_spacing(ui, child, top_left, width);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(top_left, egui::Vec2::new(width, pre_spacing)),
            //     2.,
            //     egui::Stroke::new(pre_spacing.min(1.), self.theme.bg().neutral_quarternary),
            // );
            // println!("{}pre_spacing: {}", "  ".repeat(node.ancestors().count() - 1), pre_spacing);

            top_left.y += pre_spacing;

            // add block
            let child_height = self.height(child, width);
            self.show_block(ui, child, top_left, width);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(top_left, egui::Vec2::new(width, child_height)),
            //     2.,
            //     egui::Stroke::new(1., self.theme.bg().green),
            // );
            // println!("{}child_height: {}", "  ".repeat(node.ancestors().count() - 1), child_height);

            top_left.y += child_height;

            // add post-spacing
            let post_spacing = self.block_post_spacing_height(child);
            self.show_block_post_spacing(ui, child, top_left, width);

            // debug
            // ui.painter().rect_stroke(
            //     egui::Rect::from_min_size(top_left, egui::Vec2::new(width, post_spacing)),
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

    fn span(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => 0.,

            // container_block
            NodeValue::BlockQuote => unimplemented!("not a block"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => unimplemented!("not a block"),
            NodeValue::FootnoteDefinition(_) => unimplemented!("not a block"),
            NodeValue::Item(_) => unimplemented!("not a block"),
            NodeValue::List(_) => unimplemented!("not a block"),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("not a block"),
            NodeValue::Table(_) => unimplemented!("not a block"),
            NodeValue::TableRow(_) => unimplemented!("not a block"),

            // inline
            NodeValue::Image(_) => self.inline_children_span(node, wrap),
            NodeValue::Code(NodeCode { literal, .. }) => {
                self.span_node_text_line(node, wrap, literal)
            }
            NodeValue::Emph => self.inline_children_span(node, wrap),
            NodeValue::Escaped => self.inline_children_span(node, wrap),
            NodeValue::EscapedTag(_) => self.inline_children_span(node, wrap),
            NodeValue::FootnoteReference(_) => self.inline_children_span(node, wrap),
            NodeValue::HtmlInline(html) => self.span_node_text_line(node, wrap, html),
            NodeValue::LineBreak => self.span_line_break(wrap),
            NodeValue::Link(_) => self.inline_children_span(node, wrap),
            NodeValue::Math(NodeMath { literal, .. }) => {
                self.span_node_text_line(node, wrap, literal)
            }
            NodeValue::SoftBreak => self.span_soft_break(wrap),
            NodeValue::SpoileredText => self.inline_children_span(node, wrap),
            NodeValue::Strikethrough => self.inline_children_span(node, wrap),
            NodeValue::Strong => self.inline_children_span(node, wrap),
            NodeValue::Superscript => self.inline_children_span(node, wrap),
            NodeValue::Text(text) => self.span_node_text_line(node, wrap, text),
            NodeValue::Underline => self.inline_children_span(node, wrap),
            NodeValue::WikiLink(_) => self.inline_children_span(node, wrap),

            // leaf_block
            NodeValue::CodeBlock(_) => unimplemented!("not a block"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => unimplemented!("not a block"),
            NodeValue::HtmlBlock(_) => unimplemented!("not a block"),
            NodeValue::Paragraph => unimplemented!("not a block"),
            NodeValue::TableCell => unimplemented!("not a block"),
            NodeValue::TaskItem(_) => unimplemented!("not a block"),
            NodeValue::ThematicBreak => unimplemented!("not a block"),
        }
    }

    // the span of an inline that contains inlines is the sum of the spans of
    // the inlines
    fn inline_children_span(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
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
        let children_span = self.inline_children_span(node, &WrapContext::new(width));
        let rows = (children_span / width).ceil();
        rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
    }

    // the height of possibly multiple lines of wrapped text; used for code
    // blocks and other situations where text isn't in inlines
    fn text_height(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext, text: &str) -> f32 {
        let mut tmp_wrap = wrap.clone();
        for (i, line) in text.lines().enumerate() {
            tmp_wrap.offset += self.span_node_text_line(node, wrap, line);

            // all lines except the last one end in a newline...
            if i < text.lines().count() - 1 {
                tmp_wrap.offset = tmp_wrap.line_end();
            }
        }

        // ...and sometimes the last one also ends with a newline
        if text::ends_with_newline(text) {
            tmp_wrap.offset = tmp_wrap.line_end();
        }

        let span = tmp_wrap.offset - wrap.offset;
        let rows = (span / wrap.width).ceil();
        rows * self.row_height(node) + (rows - 1.) * ROW_SPACING
    }

    fn show_inline(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        let sourcepos = node.data.borrow().sourcepos; // todo: character uncapture
        let range = self.sourcepos_to_range(sourcepos);

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => {}

            // container_block
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
            NodeValue::Image(_) => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Code(_) => self.show_node_text_line(ui, node, top_left, wrap, range),
            NodeValue::Emph => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Escaped => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::EscapedTag(_) => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::FootnoteReference(_) => {
                self.show_footnote_reference(ui, node, top_left, wrap)
            }
            NodeValue::HtmlInline(_) => self.show_node_text_line(ui, node, top_left, wrap, range),
            NodeValue::LineBreak => self.show_line_break(wrap),
            NodeValue::Link(_) => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Math(_) => self.show_node_text_line(ui, node, top_left, wrap, range),
            NodeValue::SoftBreak => self.show_soft_break(wrap),
            NodeValue::SpoileredText => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Strikethrough => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Strong => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Superscript => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::Text(_) => self.show_node_text_line(ui, node, top_left, wrap, range),
            NodeValue::Underline => self.show_inline_children(ui, node, top_left, wrap),
            NodeValue::WikiLink(_) => self.show_inline_children(ui, node, top_left, wrap),

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
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        for child in node.children() {
            self.show_inline(ui, child, top_left, wrap);
        }
    }
}
