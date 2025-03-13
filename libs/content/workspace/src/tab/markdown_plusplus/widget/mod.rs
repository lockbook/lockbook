use comrak::nodes::{AstNode, NodeValue};
use container_block::{
    BlockQuote, Document, FootnoteDefinition, Item, List, MultilineBlockQuote, Table, TableRow,
};
use egui::{Align, Context, Layout, Pos2, Rect, Sense, TextFormat, Ui, Vec2};
use inline::{
    Code, Emph, Escaped, EscapedTag, FootnoteReference, HtmlInline, Image, LineBreak, Link, Math,
    SoftBreak, SpoileredText, Strikethrough, Strong, Superscript, Text, Underline, WikiLink,
};
use leaf_block::{CodeBlock, Heading, HtmlBlock, Paragraph, TableCell, TaskItem, ThematicBreak};

use super::{theme::Theme, MarkdownPlusPlus};

mod container_block;
mod inline;
mod leaf_block;

pub const MARGIN: f32 = 20.0; // space between the editor and window border; must be large enough to accomodate bordered elements e.g. code blocks
pub const MAX_WIDTH: f32 = 800.0; // the maximum width of the editor before it starts adding padding

pub const INLINE_PADDING: f32 = 5.0; // the extra space granted to inline code for a border (both sides)
pub const ROW_HEIGHT: f32 = 20.0; // ...at default font size
pub const TABLE_PADDING: f32 = 10.0; // between a table cell and its contents (all sides)
pub const INDENT: f32 = 25.0; // enough space for two digits in a numbered list
pub const BULLET_RADIUS: f32 = 2.0;
pub const ROW_SPACING: f32 = 5.0; // must be large enough to accomodate bordered elements e.g. inline code
pub const BLOCK_SPACING: f32 = 10.0;

pub trait Block {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui);
    fn height(&self, width: f32, ctx: &Context) -> f32;
}

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

pub trait Inline {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui);
    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32;
}

pub struct Ast<'a, 't> {
    node: &'a AstNode<'a>,
    text_format: TextFormat,
    children: Vec<Ast<'a, 't>>,
    theme: &'t Theme,
}

impl MarkdownPlusPlus {
    pub fn text_format(&self, node: &AstNode<'_>) -> TextFormat {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => self.text_format(node.parent().unwrap()),

            // container_block
            NodeValue::BlockQuote => self.text_format_block_quote(node.parent().unwrap()),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.text_format_document(),
            NodeValue::FootnoteDefinition(_) => {
                self.text_format_footnote_definition(node.parent().unwrap())
            }
            NodeValue::Item(_) => self.text_format(node.parent().unwrap()),
            NodeValue::List(_) => self.text_format(node.parent().unwrap()),
            NodeValue::MultilineBlockQuote(_) => {
                self.text_format_multiline_block_quote(node.parent().unwrap())
            }
            NodeValue::Table(_) => self.text_format(node.parent().unwrap()),
            NodeValue::TableRow(is_header_row) => {
                self.text_format_table_row(node.parent().unwrap(), *is_header_row)
            }

            // inline
            NodeValue::Image(_) => self.text_format_image(node.parent().unwrap()),
            NodeValue::Code(_) => self.text_format_code(node.parent().unwrap()),
            NodeValue::Emph => self.text_format_emph(node.parent().unwrap()),
            NodeValue::Escaped => self.text_format(node.parent().unwrap()),
            NodeValue::EscapedTag(_) => self.text_format(node.parent().unwrap()),
            NodeValue::FootnoteReference(_) => {
                self.text_format_footnote_reference(node.parent().unwrap())
            }
            NodeValue::HtmlInline(_) => self.text_format_html_inline(node.parent().unwrap()),
            NodeValue::LineBreak => self.text_format(node.parent().unwrap()),
            NodeValue::Link(_) => self.text_format_link(node.parent().unwrap()),
            NodeValue::Math(_) => self.text_format_math(node.parent().unwrap()),
            NodeValue::SoftBreak => self.text_format(node.parent().unwrap()),
            NodeValue::SpoileredText => self.text_format_spoilered_text(node.parent().unwrap()),
            NodeValue::Strikethrough => self.text_format_strikethrough(node.parent().unwrap()),
            NodeValue::Strong => self.text_format_strong(node.parent().unwrap()),
            NodeValue::Superscript => self.text_format_superscript(node.parent().unwrap()),
            NodeValue::Text(_) => self.text_format(node.parent().unwrap()),
            NodeValue::Underline => self.text_format_underline(node.parent().unwrap()),
            NodeValue::WikiLink(_) => self.text_format_wiki_link(node.parent().unwrap()),

            // leaf_block
            NodeValue::CodeBlock(_) => self.text_format_code_block(node.parent().unwrap()),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => self.text_format(node.parent().unwrap()),
            NodeValue::HtmlBlock(_) => self.text_format_html_block(node.parent().unwrap()),
            NodeValue::Paragraph => self.text_format(node.parent().unwrap()),
            NodeValue::TableCell => self.text_format(node.parent().unwrap()),
            NodeValue::TaskItem(_) => self.text_format(node.parent().unwrap()),
            NodeValue::ThematicBreak => self.text_format(node.parent().unwrap()),
        }
    }
}

impl<'a, 't> Ast<'a, 't> {
    pub fn new(
        node: &'a AstNode<'a>, parent_text_format: TextFormat, theme: &'t Theme, ctx: &Context,
    ) -> Self {
        let text_format = match &node.data.borrow().value {
            NodeValue::BlockQuote => BlockQuote::text_format(theme, parent_text_format, ctx),
            NodeValue::Code(_) => Code::text_format(theme, parent_text_format, ctx),
            NodeValue::CodeBlock(_) => CodeBlock::text_format(theme, parent_text_format, ctx),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Document => Document::text_format(theme, parent_text_format, ctx),
            NodeValue::Emph => Emph::text_format(theme, parent_text_format, ctx),
            NodeValue::Escaped => Escaped::text_format(theme, parent_text_format, ctx),
            NodeValue::EscapedTag(_) => EscapedTag::text_format(theme, parent_text_format, ctx),
            NodeValue::FootnoteDefinition(_) => {
                FootnoteDefinition::text_format(theme, parent_text_format, ctx)
            }
            NodeValue::FootnoteReference(_) => {
                FootnoteReference::text_format(theme, parent_text_format, ctx)
            }
            NodeValue::FrontMatter(_) => parent_text_format,
            NodeValue::Heading(node_heading) => {
                Heading::text_format(node_heading, parent_text_format)
            }
            NodeValue::HtmlBlock(_) => HtmlBlock::text_format(theme, parent_text_format, ctx),
            NodeValue::HtmlInline(_) => HtmlInline::text_format(theme, parent_text_format, ctx),
            NodeValue::Image(_) => Image::text_format(theme, parent_text_format, ctx),
            NodeValue::Item(_) => parent_text_format,
            NodeValue::LineBreak => parent_text_format,
            NodeValue::Link(_) => Link::text_format(theme, parent_text_format, ctx),
            NodeValue::List(_) => parent_text_format,
            NodeValue::Math(_) => Math::text_format(theme, parent_text_format, ctx),
            NodeValue::MultilineBlockQuote(_) => {
                MultilineBlockQuote::text_format(theme, parent_text_format, ctx)
            }
            NodeValue::Paragraph => parent_text_format,
            NodeValue::SoftBreak => parent_text_format,
            NodeValue::SpoileredText => SpoileredText::text_format(theme, parent_text_format, ctx),
            NodeValue::Strikethrough => Strikethrough::text_format(theme, parent_text_format, ctx),
            NodeValue::Strong => Strong::text_format(theme, parent_text_format, ctx),
            NodeValue::Superscript => Superscript::text_format(theme, parent_text_format, ctx),
            NodeValue::Table(_) => parent_text_format,
            NodeValue::TableCell => parent_text_format,
            NodeValue::TableRow(is_header_row) => {
                TableRow::text_format(parent_text_format, *is_header_row)
            }
            NodeValue::TaskItem(_) => parent_text_format,
            NodeValue::Text(_) => parent_text_format,
            NodeValue::ThematicBreak => parent_text_format,
            NodeValue::Underline => Underline::text_format(theme, parent_text_format, ctx),
            NodeValue::WikiLink(_) => WikiLink::text_format(theme, parent_text_format, ctx),
        };

        let mut children = Vec::with_capacity(node.children().count());
        for child in node.children() {
            children.push(Ast::new(child, text_format.clone(), theme, ctx));
        }
        Self { node, text_format, children, theme }
    }

    fn row_height(&self, ctx: &Context) -> f32 {
        ctx.fonts(|fonts| fonts.row_height(&self.text_format.font_id))
    }

    // the spacing that should go *before* this block
    fn block_spacing(&self, ctx: &Context) -> f32 {
        let sourcepos = self.node.data.borrow().sourcepos;
        let value = &self.node.data.borrow().value;

        let mut spacing = 0.;

        if let NodeValue::TableRow(_) = value {
            return 0.;
        }

        if let Some(prev_sibling) = self.node.previous_sibling() {
            let prev_sibling_sourcepos = prev_sibling.data.borrow().sourcepos;
            let prev_sibling_value = &prev_sibling.data.borrow().value;

            let mut line_count = sourcepos.start.line - prev_sibling_sourcepos.end.line;

            line_count = line_count.saturating_sub(1); // determined empirically

            // special cases
            match prev_sibling_value {
                NodeValue::ThematicBreak => {
                    line_count +=
                        prev_sibling_sourcepos.end.line - prev_sibling_sourcepos.start.line
                }
                NodeValue::TableRow(_) => {
                    return 0.;
                }
                _ => {}
            }

            spacing = line_count as f32 * self.row_height(ctx);
            spacing = spacing.max(BLOCK_SPACING); // add at least the default spacing
        }

        spacing
    }
}

impl Block for Ast<'_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        match &self.node.data.borrow().value {
            NodeValue::BlockQuote => BlockQuote::new(self).show(width, top_left, ui),
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::CodeBlock(node) => CodeBlock::new(self, node).show(width, top_left, ui),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Document => Document::new(self).show(width, top_left, ui),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteDefinition(node) => {
                FootnoteDefinition::new(self, node).show(width, top_left, ui)
            }
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::FrontMatter(_) => {}
            NodeValue::Heading(node) => Heading::new(self, node).show(width, top_left, ui),
            NodeValue::HtmlBlock(node) => HtmlBlock::new(self, node).show(width, top_left, ui),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::Image(node) => Block::show(&Image::new(self, node), width, top_left, ui),
            NodeValue::Item(node_list) => Item::new(self, node_list).show(width, top_left, ui),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::List(node) => List::new(self, node).show(width, top_left, ui),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::MultilineBlockQuote(node) => {
                MultilineBlockQuote::new(self, node).show(width, top_left, ui)
            }
            NodeValue::Paragraph => Paragraph::new(self).show(width, top_left, ui),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Table(node) => Table::new(self, node).show(width, top_left, ui),
            NodeValue::TableCell => TableCell::new(self).show(width, top_left, ui),
            NodeValue::TableRow(is_header_row) => {
                TableRow::new(self, *is_header_row).show(width, top_left, ui)
            }
            NodeValue::TaskItem(maybe_check) => {
                TaskItem::new(self, maybe_check).show(width, top_left, ui)
            }
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::ThematicBreak => ThematicBreak::new(self).show(width, top_left, ui),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),
        }
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        match &self.node.data.borrow().value {
            NodeValue::BlockQuote => BlockQuote::new(self).height(width, ctx),
            NodeValue::Code(_) => unimplemented!("not a block"),
            NodeValue::CodeBlock(node) => CodeBlock::new(self, node).height(width, ctx),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Document => Document::new(self).height(width, ctx),
            NodeValue::Emph => unimplemented!("not a block"),
            NodeValue::Escaped => unimplemented!("not a block"),
            NodeValue::EscapedTag(_) => unimplemented!("not a block"),
            NodeValue::FootnoteDefinition(node) => {
                FootnoteDefinition::new(self, node).height(width, ctx)
            }
            NodeValue::FootnoteReference(_) => unimplemented!("not a block"),
            NodeValue::FrontMatter(_) => Default::default(),
            NodeValue::Heading(node) => Heading::new(self, node).height(width, ctx),
            NodeValue::HtmlBlock(node) => HtmlBlock::new(self, node).height(width, ctx),
            NodeValue::HtmlInline(_) => unimplemented!("not a block"),
            NodeValue::Image(node) => Block::height(&Image::new(self, node), width, ctx),
            NodeValue::Item(node_list) => Item::new(self, node_list).height(width, ctx),
            NodeValue::LineBreak => unimplemented!("not a block"),
            NodeValue::Link(_) => unimplemented!("not a block"),
            NodeValue::List(node) => List::new(self, node).height(width, ctx),
            NodeValue::Math(_) => unimplemented!("not a block"),
            NodeValue::MultilineBlockQuote(node) => {
                MultilineBlockQuote::new(self, node).height(width, ctx)
            }
            NodeValue::Paragraph => Paragraph::new(self).height(width, ctx),
            NodeValue::SoftBreak => unimplemented!("not a block"),
            NodeValue::SpoileredText => unimplemented!("not a block"),
            NodeValue::Strikethrough => unimplemented!("not a block"),
            NodeValue::Strong => unimplemented!("not a block"),
            NodeValue::Superscript => unimplemented!("not a block"),
            NodeValue::Table(node) => Table::new(self, node).height(width, ctx),
            NodeValue::TableCell => TableCell::new(self).height(width, ctx),
            NodeValue::TableRow(is_header_row) => {
                TableRow::new(self, *is_header_row).height(width, ctx)
            }
            NodeValue::TaskItem(maybe_check) => TaskItem::new(self, maybe_check).height(width, ctx),
            NodeValue::Text(_) => unimplemented!("not a block"),
            NodeValue::ThematicBreak => ThematicBreak::new(self).height(width, ctx),
            NodeValue::Underline => unimplemented!("not a block"),
            NodeValue::WikiLink(_) => unimplemented!("not a block"),
        }
    }
}

impl Ast<'_, '_> {
    // blocks are stacked vertically
    fn show_block_children(&self, width: f32, mut top_left: Pos2, ui: &mut Ui) {
        let rect = Rect::from_min_size(top_left, Vec2::new(width, 0.));
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.with_layout(Layout::top_down(Align::LEFT).with_main_wrap(false), |ui| {
                for child in &self.children {
                    // add spacing between blocks based on source lines
                    let spacing = child.block_spacing(ui.ctx());

                    // placement based on ui cursor supported for now...
                    let spacing_rect = Rect::from_min_size(top_left, Vec2::new(width, spacing));
                    ui.allocate_rect(spacing_rect, Sense::hover());

                    // debug
                    // ui.painter()
                    //     .rect_stroke(spacing_rect, 2., egui::Stroke::new(1., self.theme.bg().tertiary));

                    // ...soon all nodes will use the provided params
                    top_left.y += spacing;

                    // add block
                    Block::show(child, width, top_left, ui);
                    let child_height = child.height(width, ui.ctx());

                    // placement based on ui cursor supported for now...
                    let child_rect = Rect::from_min_size(top_left, Vec2::new(width, child_height));
                    ui.advance_cursor_after_rect(child_rect);

                    // debug
                    // ui.painter()
                    //     .rect_stroke(child_rect, 2., egui::Stroke::new(1., self.theme.bg().green));

                    // ...soon all nodes will use the provided params
                    top_left.y += child_height;
                }
            });
        });

        // debug
        // ui.painter()
        //     .rect_stroke(rect, 2., egui::Stroke::new(1., self.theme.bg().tertiary));
    }

    // the height of a block that contains blocks is the sum of the heights of the blocks
    fn block_children_height(&self, width: f32, ctx: &Context) -> f32 {
        let mut height_sum = 0.0;
        for child in &self.children {
            height_sum += child.block_spacing(ctx);
            height_sum += child.height(width, ctx)
        }
        height_sum
    }
}

impl Inline for Ast<'_, '_> {
    fn show(&self, wrap: &mut WrapContext, top_left: Pos2, ui: &mut Ui) {
        match &self.node.data.borrow().value {
            NodeValue::BlockQuote => unimplemented!("not an inline"),
            NodeValue::Code(node) => Code::new(self, node).show(wrap, top_left, ui),
            NodeValue::CodeBlock(_) => unimplemented!("not an inline"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Document => unimplemented!("not an inline"),
            NodeValue::Emph => Emph::new(self).show(wrap, top_left, ui),
            NodeValue::Escaped => Escaped::new(self).show(wrap, top_left, ui),
            NodeValue::EscapedTag(escaped) => {
                EscapedTag::new(self, escaped).show(wrap, top_left, ui)
            }
            NodeValue::FootnoteDefinition(_) => unimplemented!("not an inline"),
            NodeValue::FootnoteReference(node) => {
                FootnoteReference::new(self, node).show(wrap, top_left, ui)
            }
            NodeValue::FrontMatter(_) => {}
            NodeValue::Heading(_) => unimplemented!("not an inline"),
            NodeValue::HtmlBlock(_) => unimplemented!("not an inline"),
            NodeValue::HtmlInline(html) => HtmlInline::new(self, html).show(wrap, top_left, ui),
            NodeValue::Image(node) => Inline::show(&Image::new(self, node), wrap, top_left, ui),
            NodeValue::Item(_) => unimplemented!("not an inline"),
            NodeValue::LineBreak => LineBreak::new(self).show(wrap, top_left, ui),
            NodeValue::Link(node) => Link::new(self, node).show(wrap, top_left, ui),
            NodeValue::List(_) => unimplemented!("not an inline"),
            NodeValue::Math(node) => Math::new(self, node).show(wrap, top_left, ui),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("not an inline"),
            NodeValue::Paragraph => unimplemented!("not an inline"),
            NodeValue::SoftBreak => SoftBreak::new(self).show(wrap, top_left, ui),
            NodeValue::SpoileredText => SpoileredText::new(self).show(wrap, top_left, ui),
            NodeValue::Strikethrough => Strikethrough::new(self).show(wrap, top_left, ui),
            NodeValue::Strong => Strong::new(self).show(wrap, top_left, ui),
            NodeValue::Superscript => Superscript::new(self).show(wrap, top_left, ui),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableCell => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),
            NodeValue::TaskItem(_) => unimplemented!("not an inline"),
            NodeValue::Text(text) => Text::new(self, text).show(wrap, top_left, ui),
            NodeValue::ThematicBreak => unimplemented!("not an inline"),
            NodeValue::Underline => Underline::new(self).show(wrap, top_left, ui),
            NodeValue::WikiLink(node) => WikiLink::new(self, node).show(wrap, top_left, ui),
        }
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        match &self.node.data.borrow().value {
            NodeValue::BlockQuote => unimplemented!("not an inline"),
            NodeValue::Code(node) => Code::new(self, node).span(wrap, ctx),
            NodeValue::CodeBlock(_) => unimplemented!("not an inline"),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Document => unimplemented!("not an inline"),
            NodeValue::Emph => Emph::new(self).span(wrap, ctx),
            NodeValue::Escaped => Escaped::new(self).span(wrap, ctx),
            NodeValue::EscapedTag(escaped) => EscapedTag::new(self, escaped).span(wrap, ctx),
            NodeValue::FootnoteDefinition(_) => unimplemented!("not an inline"),
            NodeValue::FootnoteReference(node) => {
                FootnoteReference::new(self, node).span(wrap, ctx)
            }
            NodeValue::FrontMatter(_) => 0.0,
            NodeValue::Heading(_) => unimplemented!("not an inline"),
            NodeValue::HtmlBlock(_) => unimplemented!("not an inline"),
            NodeValue::HtmlInline(html) => HtmlInline::new(self, html).span(wrap, ctx),
            NodeValue::Image(node) => Image::new(self, node).span(wrap, ctx),
            NodeValue::Item(_) => unimplemented!("not an inline"),
            NodeValue::LineBreak => LineBreak::new(self).span(wrap, ctx),
            NodeValue::Link(node) => Link::new(self, node).span(wrap, ctx),
            NodeValue::List(_) => unimplemented!("not an inline"),
            NodeValue::Math(node) => Math::new(self, node).span(wrap, ctx),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("not an inline"),
            NodeValue::Paragraph => unimplemented!("not an inline"),
            NodeValue::SoftBreak => SoftBreak::new(self).span(wrap, ctx),
            NodeValue::SpoileredText => SpoileredText::new(self).span(wrap, ctx),
            NodeValue::Strikethrough => Strikethrough::new(self).span(wrap, ctx),
            NodeValue::Strong => Strong::new(self).span(wrap, ctx),
            NodeValue::Superscript => Superscript::new(self).span(wrap, ctx),
            NodeValue::Table(_) => unimplemented!("not an inline"),
            NodeValue::TableCell => unimplemented!("not an inline"),
            NodeValue::TableRow(_) => unimplemented!("not an inline"),
            NodeValue::TaskItem(_) => unimplemented!("not an inline"),
            NodeValue::Text(text) => Text::new(self, text).span(wrap, ctx),
            NodeValue::ThematicBreak => unimplemented!("not an inline"),
            NodeValue::Underline => Underline::new(self).span(wrap, ctx),
            NodeValue::WikiLink(node) => WikiLink::new(self, node).span(wrap, ctx),
        }
    }
}

impl Ast<'_, '_> {
    // inlines are stacked horizontally and wrapped
    fn show_inline_children(&self, wrap: &mut WrapContext, top_left: &mut Pos2, ui: &mut Ui) {
        ui.allocate_ui_at_rect(Rect::from_min_size(*top_left, Vec2::new(wrap.width, 0.)), |ui| {
            ui.with_layout(Layout::left_to_right(Align::TOP).with_main_wrap(true), |ui| {
                for child in &self.children {
                    Inline::show(child, wrap, *top_left, ui);
                }
            })
        });
        top_left.y += self.inline_children_height(wrap.width, ui.ctx())
    }

    // the span of an inline that contains inlines is the sum of the spans of the inlines
    fn inline_children_span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        let mut result = 0.;
        for child in &self.children {
            result += child.span(wrap, ctx);
        }
        result
    }

    // the size of a block that contains inlines is the span of the inlines divided
    // by the wrap width (rounded up), times the row height (plus spacing)
    fn inline_children_height(&self, width: f32, ctx: &Context) -> f32 {
        let children_span = self.inline_children_span(&WrapContext::new(width), ctx);
        let rows = (children_span / width).ceil();
        rows * self.row_height(ctx) + (rows - 1.) * ROW_SPACING
    }

    // the height of inline text; used for code blocks and other situations where text isn't in inlines
    fn inline_text_height(&self, wrap: &WrapContext, ctx: &Context, text: String) -> f32 {
        let span = self.text_span(wrap, ctx, text);
        let rows = (span / wrap.width).ceil();
        rows * self.row_height(ctx) + (rows - 1.) * ROW_SPACING
    }
}
