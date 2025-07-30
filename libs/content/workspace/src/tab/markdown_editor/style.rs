use comrak::nodes::NodeValue;
use pulldown_cmark::{HeadingLevel, LinkType};
use std::fmt::{Display, Formatter};
use std::hash::Hash;

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
    /// Returns true if the markdown syntax for the node contains text which should be split into words for word bounds calculation
    pub fn syntax_includes_text(&self) -> bool {
        matches!(self, Self::Inline(InlineNodeType::Link) | Self::Inline(InlineNodeType::Image))
    }

    pub fn conflicts_with(&self, other: &MarkdownNodeType) -> bool {
        matches!((self, other), (Self::Block(..), Self::Block(..)))
    }

    pub fn matching(value: &NodeValue) -> Option<Self> {
        match value {
            NodeValue::Code(_) => Some(Self::Inline(InlineNodeType::Code)),
            NodeValue::Emph => Some(Self::Inline(InlineNodeType::Italic)),
            NodeValue::Strong => Some(Self::Inline(InlineNodeType::Bold)),
            NodeValue::Strikethrough => Some(Self::Inline(InlineNodeType::Strikethrough)),
            _ => None,
        }
    }

    pub fn matches(&self, value: &NodeValue) -> bool {
        match self {
            Self::Document => matches!(value, NodeValue::Document),
            Self::Paragraph => matches!(value, NodeValue::Paragraph),
            Self::Inline(inline) => inline.matches(value),
            Self::Block(block) => block.matches(value),
        }
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

impl InlineNodeType {
    pub fn head(&self) -> &'static str {
        match self {
            InlineNodeType::Code => "`",
            InlineNodeType::Bold => "**",
            InlineNodeType::Italic => "*",
            InlineNodeType::Strikethrough => "~~",
            InlineNodeType::Link => "[",
            InlineNodeType::Image => {
                unimplemented!()
            }
        }
    }

    pub fn tail(&self) -> &'static str {
        match self {
            InlineNodeType::Code => "`",
            InlineNodeType::Bold => "**",
            InlineNodeType::Italic => "*",
            InlineNodeType::Strikethrough => "~~",
            InlineNodeType::Link => "]()",
            InlineNodeType::Image => {
                unimplemented!()
            }
        }
    }

    pub fn matches(&self, value: &NodeValue) -> bool {
        matches!(
            (value, self),
            (NodeValue::Code(_), InlineNodeType::Code)
                | (NodeValue::Emph, InlineNodeType::Italic)
                | (NodeValue::Strikethrough, InlineNodeType::Strikethrough)
                | (NodeValue::Strong, InlineNodeType::Bold)
        )
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum BlockNodeType {
    Heading(HeadingLevel),
    Quote,
    Code,
    ListItem(ListItemType),
    Rule,
}

impl BlockNodeType {
    pub fn matches(&self, value: &NodeValue) -> bool {
        match (value, self) {
            // container_block
            (NodeValue::Alert(_), BlockNodeType::Quote) => true,
            (NodeValue::BlockQuote, BlockNodeType::Quote) => true,
            (
                NodeValue::Item(
                    comrak::nodes::NodeList { list_type: comrak::nodes::ListType::Bullet, .. },
                    ..,
                ),
                BlockNodeType::ListItem(ListItemType::Bulleted),
            ) => true,
            (
                NodeValue::Item(
                    comrak::nodes::NodeList { list_type: comrak::nodes::ListType::Ordered, .. },
                    ..,
                ),
                BlockNodeType::ListItem(ListItemType::Numbered),
            ) => true,
            (NodeValue::MultilineBlockQuote(_), BlockNodeType::Quote) => true,
            (NodeValue::TaskItem(_), BlockNodeType::ListItem(ListItemType::Todo)) => true,

            // leaf_block
            (NodeValue::CodeBlock(_), BlockNodeType::Code) => true,
            (NodeValue::Heading(_), BlockNodeType::Heading(_)) => true,
            (NodeValue::ThematicBreak, BlockNodeType::Rule) => true,
            _ => false,
        }
    }
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
    pub fn node_type(&self) -> InlineNodeType {
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
    pub fn node_type(&self) -> BlockNodeType {
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

impl Display for MarkdownNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document => write!(f, "Document"),
            Self::Paragraph => write!(f, "Paragraph"),
            Self::Inline(inline_node) => write!(f, "{inline_node}"),
            Self::Block(block_node) => write!(f, "{block_node}"),
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
