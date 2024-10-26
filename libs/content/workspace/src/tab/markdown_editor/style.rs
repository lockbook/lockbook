use crate::tab::markdown_editor::appearance::Appearance;
use egui::{FontFamily, Stroke, TextFormat, Visuals};
use pulldown_cmark::{HeadingLevel, LinkType};
use std::fmt::{Display, Formatter};
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
            Self::Inline(InlineNodeType::Bold) => "**",
            Self::Inline(InlineNodeType::Italic) => "*",
            Self::Inline(InlineNodeType::Strikethrough) => "~~",
            Self::Inline(InlineNodeType::Link) => "[",
            Self::Inline(InlineNodeType::Image) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Heading(HeadingLevel::H1)) => "# ",
            Self::Block(BlockNodeType::Heading(HeadingLevel::H2)) => "## ",
            Self::Block(BlockNodeType::Heading(HeadingLevel::H3)) => "### ",
            Self::Block(BlockNodeType::Heading(HeadingLevel::H4)) => "#### ",
            Self::Block(BlockNodeType::Heading(HeadingLevel::H5)) => "##### ",
            Self::Block(BlockNodeType::Heading(HeadingLevel::H6)) => "###### ",
            Self::Block(BlockNodeType::Quote) => "> ",
            Self::Block(BlockNodeType::Code) => "```\n",
            Self::Block(BlockNodeType::ListItem(item_type)) => item_type.head(),
            Self::Block(BlockNodeType::Rule) => "***",
        }
    }

    pub fn tail(&self) -> &'static str {
        match self {
            Self::Document => "",
            Self::Paragraph => "",
            Self::Inline(InlineNodeType::Code) => "`",
            Self::Inline(InlineNodeType::Bold) => "**",
            Self::Inline(InlineNodeType::Italic) => "*",
            Self::Inline(InlineNodeType::Strikethrough) => "~~",
            Self::Inline(InlineNodeType::Link) => "]()",
            Self::Inline(InlineNodeType::Image) => {
                unimplemented!()
            }
            Self::Block(BlockNodeType::Heading(..)) => "",
            Self::Block(BlockNodeType::Quote) => "",
            Self::Block(BlockNodeType::Code) => "\n```",
            Self::Block(BlockNodeType::ListItem(..)) => "",
            Self::Block(BlockNodeType::Rule) => "",
        }
    }

    /// Returns true if the markdown syntax for the node contains text which should be split into words for word bounds calculation
    pub fn syntax_includes_text(&self) -> bool {
        matches!(self, Self::Inline(InlineNodeType::Link) | Self::Inline(InlineNodeType::Image))
    }

    pub fn conflicts_with(&self, other: &MarkdownNodeType) -> bool {
        matches!((self, other), (Self::Block(..), Self::Block(..)))
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
    PlaintextLink,
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
        match (self, other) {
            (Self::Code, Self::Code) => true,
            (Self::Bold, Self::Bold) => true,
            (Self::Italic, Self::Italic) => true,
            (Self::Strikethrough, Self::Strikethrough) => true,
            (Self::Link(_, url, title), Self::Link(_, other_url, other_title)) => {
                url == other_url && title == other_title
            }
            (Self::Image(_, url, title), Self::Image(_, other_url, other_title)) => {
                url == other_url && title == other_title
            }
            _ => false,
        }
    }
}

impl Eq for InlineNode {}

impl Hash for InlineNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Code => InlineNodeType::Code.hash(state),
            Self::Bold => InlineNodeType::Bold.hash(state),
            Self::Italic => InlineNodeType::Italic.hash(state),
            Self::Strikethrough => InlineNodeType::Strikethrough.hash(state),
            Self::Link(_, url, title) => {
                InlineNodeType::Link.hash(state);
                url.hash(state);
                title.hash(state);
            }
            Self::Image(_, url, title) => {
                InlineNodeType::Image.hash(state);
                url.hash(state);
                title.hash(state);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum BlockNode {
    Heading(HeadingLevel),
    Quote,
    Code(String), // language
    ListItem(ListItem, IndentLevel),
    Rule,
}

impl BlockNode {
    fn node_type(&self) -> BlockNodeType {
        match self {
            Self::Heading(level) => BlockNodeType::Heading(*level),
            Self::Quote => BlockNodeType::Quote,
            Self::Code(..) => BlockNodeType::Code,
            Self::ListItem(item, ..) => BlockNodeType::ListItem(item.item_type()),
            Self::Rule => BlockNodeType::Rule,
        }
    }
}

impl PartialEq for BlockNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Heading(level), Self::Heading(other_level)) => level == other_level,
            (Self::Quote, Self::Quote) => true,
            (Self::Code(..), Self::Code(..)) => true,
            (
                Self::ListItem(item, indent_level),
                Self::ListItem(other_item, other_indent_level),
            ) => item == other_item && indent_level == other_indent_level,
            (Self::Rule, Self::Rule) => true,
            _ => false,
        }
    }
}

impl Eq for BlockNode {}

impl Hash for BlockNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Heading(level) => {
                BlockNodeType::Heading(*level).hash(state);
            }
            Self::Quote => {
                BlockNodeType::Quote.hash(state);
            }
            Self::Code(..) => {
                BlockNodeType::Code.hash(state);
            }
            Self::ListItem(item, indent_level) => {
                BlockNodeType::ListItem(item.item_type()).hash(state);
                indent_level.hash(state);
            }
            Self::Rule => {
                BlockNodeType::Rule.hash(state);
            }
        }
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
        match (self, other) {
            (Self::Bulleted, Self::Bulleted) => true,
            (Self::Numbered(num), Self::Numbered(other_num)) => num == other_num,
            (Self::Todo(checked), Self::Todo(other_checked)) => checked == other_checked,
            _ => false,
        }
    }
}

impl Eq for ListItem {}

impl Hash for ListItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Bulleted => {
                ListItemType::Bulleted.hash(state);
            }
            Self::Numbered(num) => {
                ListItemType::Numbered.hash(state);
                num.hash(state);
            }
            Self::Todo(checked) => {
                ListItemType::Todo.hash(state);
                checked.hash(state);
            }
        }
    }
}

pub type Url = String;
pub type Title = String;
pub type IndentLevel = u8;

impl RenderStyle {
    pub fn apply_style(&self, text_format: &mut TextFormat, vis: &Appearance, visuals: &Visuals) {
        if vis.plaintext_mode {
            match self {
                RenderStyle::Selection => {
                    text_format.background = vis.selection_bg();
                }
                RenderStyle::PlaintextLink => {
                    text_format.color = vis.link();
                    text_format.underline = Stroke { width: 1.5, color: vis.link() };
                }
                RenderStyle::Syntax => {}
                RenderStyle::Markdown(MarkdownNode::Document) => {
                    text_format.font_id.family = FontFamily::Monospace;
                    text_format.font_id.size = vis.font_size();
                    text_format.color = vis.text();
                }
                RenderStyle::Markdown(_) => {}
            }
        } else {
            match self {
                RenderStyle::Selection => {
                    text_format.background = vis.selection_bg();
                }
                RenderStyle::PlaintextLink => {
                    text_format.color = vis.link();
                    text_format.underline = Stroke { width: 1.5, color: vis.link() };
                }
                RenderStyle::Syntax => {
                    text_format.color = vis.syntax();
                }
                RenderStyle::Markdown(MarkdownNode::Document) => {
                    text_format.font_id.size = vis.font_size();
                    text_format.color = vis.text();
                }
                RenderStyle::Markdown(MarkdownNode::Paragraph) => {
                    text_format.font_id.size = vis.font_size();
                    text_format.color = vis.text();
                }
                RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Code)) => {
                    text_format.background = visuals.code_bg_color;
                    text_format.font_id.family = FontFamily::Monospace;
                    text_format.font_id.size *= 14.0 / 16.0;
                    text_format.color = vis.code();
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
                    text_format.underline = Stroke { width: 1.5, color: vis.link() };
                }
                RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Image(..))) => {
                    text_format.italics = true;
                }
                RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Heading(level))) => {
                    if level == &HeadingLevel::H1 {
                        text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
                    }
                    text_format.color = vis.heading();
                    text_format.font_id.size = vis.heading_size(level);
                }
                RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Quote)) => {}
                RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Code(..))) => {
                    text_format.color = vis.code();
                    text_format.font_id.family = FontFamily::Monospace;
                    text_format.font_id.size *= 14.0 / 16.0;
                }
                RenderStyle::Markdown(MarkdownNode::Block(BlockNode::ListItem(..))) => {}
                RenderStyle::Markdown(MarkdownNode::Block(BlockNode::Rule)) => {}
            }
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

    pub fn head(&self) -> String {
        let type_head = self.node_type().head();
        if let MarkdownNode::Block(BlockNode::ListItem(_, indent)) = self {
            // todo: more intelligent indentation character selection
            "\t".repeat(*indent as usize) + type_head
        } else {
            type_head.to_string()
        }
    }
}

impl Display for MarkdownNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document => write!(f, "Document"),
            Self::Paragraph => write!(f, "Paragraph"),
            Self::Inline(inline_node) => write!(f, "{}", inline_node),
            Self::Block(block_node) => write!(f, "{}", block_node),
        }
    }
}

impl Display for InlineNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "Code"),
            Self::Bold => write!(f, "Bold"),
            Self::Italic => write!(f, "Italic"),
            Self::Strikethrough => write!(f, "Strikethrough"),
            Self::Link(..) => write!(f, "Link"),
            Self::Image(..) => write!(f, "Image"),
        }
    }
}

impl Display for BlockNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Heading(..) => write!(f, "Heading"),
            Self::Quote => write!(f, "Block Quote"),
            Self::Code(..) => write!(f, "Code Block"),
            Self::ListItem(ListItem::Bulleted, ..) => write!(f, "Bulleted List"),
            Self::ListItem(ListItem::Numbered(..), ..) => write!(f, "Numbered List"),
            Self::ListItem(ListItem::Todo(..), ..) => write!(f, "Todo List"),
            Self::Rule => write!(f, "Rule"),
        }
    }
}
