use crate::appearance::Appearance;
use egui::{FontFamily, Stroke, TextFormat};
use pulldown_cmark::{HeadingLevel, LinkType};
use std::hash::Hash;
use std::sync::Arc;

/// Represents a type of markdown node e.g. link, not a particular node e.g. link to google.com (see MarkdownNode)
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub enum MarkdownNodeType {
    #[default]
    Document,
    Paragraph,

    Inline(InlineNodeType),
    Block(BlockNodeType),
}

impl MarkdownNodeType {
    pub fn head(&self) -> &'static str {
        match self {
            Self::Document => "",
            Self::Paragraph => "",
            Self::Inline(InlineNodeType::Code) => "`",
            Self::Inline(InlineNodeType::Bold) => "__",
            Self::Inline(InlineNodeType::Italic) => "_",
            Self::Inline(InlineNodeType::Strikethrough) => "~~",
            Self::Inline(InlineNodeType::Link) => "[",
            Self::Inline(InlineNodeType::Image) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Heading(..)) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Quote) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Code) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::ListItem(item_type)) => item_type.head(), // todo: support indentation
            Self::Block(BlockNodeType::Rule) => "***",
        }
    }

    pub fn tail(&self) -> &'static str {
        match self {
            Self::Document => "",
            Self::Paragraph => "",
            Self::Inline(InlineNodeType::Code) => "`",
            Self::Inline(InlineNodeType::Bold) => "__",
            Self::Inline(InlineNodeType::Italic) => "_",
            Self::Inline(InlineNodeType::Strikethrough) => "~~",
            Self::Inline(InlineNodeType::Link) => "]()",
            Self::Inline(InlineNodeType::Image) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Heading(..)) => "",
            Self::Block(BlockNodeType::Quote) => "",
            Self::Block(BlockNodeType::Code) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::ListItem(..)) => "",
            Self::Block(BlockNodeType::Rule) => "",
        }
    }

    pub fn needs_whitespace(&self) -> bool {
        matches!(self, Self::Inline(InlineNodeType::Bold | InlineNodeType::Italic))
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum InlineNodeType {
    Code,
    Bold,
    Italic,
    Strikethrough,
    Link,
    Image,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum BlockNodeType {
    Heading(HeadingLevel),
    Quote,
    Code,
    ListItem(ListItemType),
    Rule,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ListItemType {
    Bulleted,
    Numbered,
    Todo,
}

impl ListItemType {
    pub fn head(&self) -> &'static str {
        match self {
            Self::Bulleted => "* ",
            Self::Numbered => "1. ",
            Self::Todo => "* [ ] ",
        }
    }
}

/// Represents a style that can be applied to rendered text
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenderStyle {
    Selection,
    Syntax,
    Markdown(MarkdownNode),
}

/// Represents a particular markdown node e.g. link to google.com, not a type of node e.g. link (see MarkdownNodeType)
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum MarkdownNode {
    #[default]
    Document,
    Paragraph,

    Inline(InlineNode),
    Block(BlockNode),
}

#[derive(Clone, Debug)]
pub enum InlineNode {
    Code,
    Bold,
    Italic,
    Strikethrough,
    Link(LinkType, Url, Title), // todo: swap strings for text ranges and impl Copy
    Image(LinkType, Url, Title), // todo: swap strings for text ranges and impl Copy
}

impl InlineNode {
    fn node_type(&self) -> InlineNodeType {
        match self {
            Self::Code => InlineNodeType::Code,
            Self::Bold => InlineNodeType::Bold,
            Self::Italic => InlineNodeType::Italic,
            Self::Strikethrough => InlineNodeType::Strikethrough,
            Self::Link(..) => InlineNodeType::Link,
            Self::Image(..) => InlineNodeType::Image,
        }
    }
}

impl PartialEq for InlineNode {
    fn eq(&self, other: &Self) -> bool {
        self.node_type() == other.node_type()
    }
}

impl Eq for InlineNode {}

impl Hash for InlineNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_type().hash(state);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BlockNode {
    Heading(HeadingLevel),
    Quote,
    Code,
    ListItem(ListItem, IndentLevel),
    Rule,
}

impl BlockNode {
    fn node_type(&self) -> BlockNodeType {
        match self {
            Self::Heading(level) => BlockNodeType::Heading(*level),
            Self::Quote => BlockNodeType::Quote,
            Self::Code => BlockNodeType::Code,
            Self::ListItem(item, ..) => BlockNodeType::ListItem(item.item_type()),
            Self::Rule => BlockNodeType::Rule,
        }
    }
}

impl PartialEq for BlockNode {
    fn eq(&self, other: &Self) -> bool {
        self.node_type() == other.node_type()
    }
}

impl Eq for BlockNode {}

impl Hash for BlockNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_type().hash(state);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ListItem {
    Bulleted,
    Numbered(usize),
    Todo(bool),
}

impl ListItem {
    pub fn item_type(&self) -> ListItemType {
        match self {
            ListItem::Bulleted => ListItemType::Bulleted,
            ListItem::Numbered(_) => ListItemType::Numbered,
            ListItem::Todo(_) => ListItemType::Todo,
        }
    }
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.item_type() == other.item_type()
    }
}

impl Eq for ListItem {}

impl Hash for ListItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.item_type().hash(state);
    }
}

pub type Url = String;
pub type Title = String;
pub type IndentLevel = u8;

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
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Code)) => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.color = vis.code();
                text_format.font_id.size = 14.0;
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Bold)) => {
                text_format.color = vis.bold();
                text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Italic)) => {
                text_format.color = vis.italics();
                text_format.italics = true;
            }
            RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Strikethrough)) => {
                text_format.strikethrough = Stroke { width: 1.5, color: vis.strikethrough() };
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
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Quote)) => {
                text_format.italics = true;
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Code)) => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.font_id.size = 14.0;
                text_format.color = vis.code();
            }
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::ListItem(..))) => {}
            RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Rule)) => {}
        }
    }
}

impl MarkdownNode {
    pub fn node_type(&self) -> MarkdownNodeType {
        match self {
            Self::Document => MarkdownNodeType::Document,
            Self::Paragraph => MarkdownNodeType::Paragraph,
            Self::Inline(inline_node) => MarkdownNodeType::Inline(inline_node.node_type()),
            Self::Block(block_node) => MarkdownNodeType::Block(block_node.node_type()),
        }
    }
}

// todo: move to appearance
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
