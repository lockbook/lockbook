use crate::appearance::Appearance;
use egui::{FontFamily, Stroke, TextFormat};
use pulldown_cmark::{HeadingLevel, LinkType};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderStyle {
    Selection,
    Syntax,
    Markdown(MarkdownNode),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MarkdownNode {
    #[default]
    Document,
    Paragraph,

    Inline(InlineNode),
    Block(BlockNode),
}

#[derive(Clone, Debug)]
pub enum InlineNode {
    InlineCode, // todo: name stutters
    Strong,     // todo: make name reflect applied style
    Emphasis,   // todo: make name reflect applied style
    Strikethrough,
    Link(LinkType, Url, Title), // todo: swap strings for text ranges and impl Copy
    Image(LinkType, Url, Title), // todo: swap strings for text ranges and impl Copy
}

// if you add a variant to InlineNode, you have to also add it here
// two nodes should be considered equal if toggling the style for one should remove the other
// todo: better pattern where you don't have to just remember to update this
impl PartialEq for InlineNode {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::InlineCode, Self::InlineCode)
                | (Self::Strong, Self::Strong)
                | (Self::Emphasis, Self::Emphasis)
                | (Self::Strikethrough, Self::Strikethrough)
                | (Self::Link(..), Self::Link(..))
                | (Self::Image(..), Self::Image(..))
        )
    }
}

impl Eq for InlineNode {}

#[derive(Clone, Copy, Debug, Eq)]
pub enum BlockNode {
    Heading(HeadingLevel),
    QuoteBlock, // todo: name stutters
    CodeBlock,  // todo: name stutters
    ListItem(ItemType, IndentLevel),
}

// if you add a variant to BlockNode, you have to also add it here
// two nodes should be considered equal if toggling the style for one should remove the other
// todo: better pattern where you don't have to just remember to update this
impl PartialEq for BlockNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::QuoteBlock, Self::QuoteBlock)
            | (Self::CodeBlock, Self::CodeBlock)
            | (Self::Heading(..), Self::Heading(..)) => true,
            (Self::ListItem(item_type_a, ..), Self::ListItem(item_type_b, ..)) => {
                item_type_a == item_type_b
            }
            _ => false,
        }
    }
}

impl RenderStyle {
    pub fn apply_style(&self, text_format: &mut TextFormat, vis: &Appearance) {
        match self {
            RenderStyle::Selection => {
                text_format.background = vis.selection_bg();
            }
            RenderStyle::Syntax => {
                text_format.color = vis.syntax();
            }
            RenderStyle::Markdown(MarkdownNode::Document) => {
                text_format.font_id.size = 16.0;
                text_format.color = vis.text();
            }
            RenderStyle::Markdown(MarkdownNode::Paragraph) => {}
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::InlineCode)) => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.color = vis.code();
                text_format.font_id.size = 14.0;
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Strong)) => {
                text_format.color = vis.bold();
                text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Emphasis)) => {
                text_format.color = vis.italics();
                text_format.italics = true;
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Strikethrough)) => {
                text_format.strikethrough = Stroke { width: 0.5, color: vis.strikethrough() };
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Link(..))) => {
                text_format.color = vis.link();
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Image(..))) => {
                text_format.italics = true;
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Heading(level))) => {
                if level == &HeadingLevel::H1 {
                    text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
                }
                text_format.font_id.size = heading_size(level);
                text_format.color = vis.heading();
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::QuoteBlock)) => {
                text_format.italics = true;
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::CodeBlock)) => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.font_id.size = 14.0;
                text_format.color = vis.code();
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::ListItem(..))) => {}
        }
    }
}

impl MarkdownNode {
    pub fn head(&self) -> &'static str {
        match self {
            MarkdownNode::Document => "",
            MarkdownNode::Paragraph => "",
            MarkdownNode::Inline(InlineNode::InlineCode) => "`",
            MarkdownNode::Inline(InlineNode::Strong) => "__",
            MarkdownNode::Inline(InlineNode::Emphasis) => "_",
            MarkdownNode::Inline(InlineNode::Strikethrough) => "~~",
            MarkdownNode::Inline(InlineNode::Link(..)) => {
                unimplemented!()
            }
            MarkdownNode::Inline(InlineNode::Image(..)) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::Heading(..)) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::QuoteBlock) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::CodeBlock) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::ListItem(..)) => {
                unimplemented!()
            }
        }
    }

    pub fn tail(&self) -> &'static str {
        match self {
            MarkdownNode::Document => "",
            MarkdownNode::Paragraph => "",
            MarkdownNode::Inline(InlineNode::InlineCode) => "`",
            MarkdownNode::Inline(InlineNode::Strong) => "__",
            MarkdownNode::Inline(InlineNode::Emphasis) => "_",
            MarkdownNode::Inline(InlineNode::Strikethrough) => "~~",
            MarkdownNode::Inline(InlineNode::Link(..)) => {
                unimplemented!()
            }
            MarkdownNode::Inline(InlineNode::Image(..)) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::Heading(..)) => "",
            MarkdownNode::Block(BlockNode::QuoteBlock) => "",
            MarkdownNode::Block(BlockNode::CodeBlock) => {
                unimplemented!()
            }
            MarkdownNode::Block(BlockNode::ListItem(..)) => "",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq)]
pub enum ItemType {
    Bulleted,
    Numbered(usize),
    Todo(bool),
}

pub fn item_type(text: &str) -> ItemType {
    let text = text.trim_start();
    if text.starts_with("+ [ ]") || text.starts_with("* [ ]") || text.starts_with("- [ ]") {
        ItemType::Todo(false)
    } else if text.starts_with("+ [x]") || text.starts_with("* [x]") || text.starts_with("- [x]") {
        ItemType::Todo(true)
    } else if let Some(prefix) = text.split('.').next() {
        if let Ok(num) = prefix.parse::<usize>() {
            ItemType::Numbered(num)
        } else {
            ItemType::Bulleted // default to bullet
        }
    } else {
        ItemType::Bulleted // default to bullet
    }
}

// Ignore inner values in enum variant comparison
// Note: you need to remember to incorporate new variants here!
impl PartialEq for ItemType {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (ItemType::Bulleted, ItemType::Bulleted)
                | (ItemType::Numbered(_), ItemType::Numbered(_))
                | (ItemType::Todo(_), ItemType::Todo(_))
        )
    }
}

pub type Url = String;
pub type Title = String;

pub type IndentLevel = u8;

fn heading_size(level: &HeadingLevel) -> f32 {
    match level {
        HeadingLevel::H1 => 32.0,
        HeadingLevel::H2 => 28.0,
        HeadingLevel::H3 => 25.0,
        HeadingLevel::H4 => 22.0,
        HeadingLevel::H5 => 19.0,
        HeadingLevel::H6 => 17.0,
    }
}
