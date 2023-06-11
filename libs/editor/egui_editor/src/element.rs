use crate::appearance::Appearance;
use egui::{FontFamily, Stroke, TextFormat};
use pulldown_cmark::{HeadingLevel, LinkType, Tag};
use std::sync::Arc;

#[derive(Clone, PartialEq, Debug, Default)]
pub enum Element {
    #[default]
    Document,

    // Blocks
    Heading(HeadingLevel),
    Paragraph,
    QuoteBlock,
    CodeBlock,
    Item,

    // Non-blocks
    InlineCode,
    Strong,
    Emphasis,
    Strikethrough,
    Link(LinkType, Url, Title),
    Image(LinkType, Url, Title),

    // Cursor-based
    Selection,
}

impl Element {
    /// note: Not all tags supported, also see `Event`
    pub fn from_tag(tag: Tag) -> Option<Self> {
        match tag {
            Tag::Paragraph => Some(Element::Paragraph),
            Tag::Heading(level, _, _) => Some(Element::Heading(level)),
            Tag::BlockQuote => Some(Element::QuoteBlock),
            Tag::CodeBlock(_) => Some(Element::CodeBlock),
            Tag::Item => Some(Element::Item),
            Tag::Emphasis => Some(Element::Emphasis),
            Tag::Strong => Some(Element::Strong),
            Tag::Strikethrough => Some(Element::Strikethrough),
            Tag::Link(l, u, t) => Some(Element::Link(l, u.to_string(), t.to_string())),
            Tag::Image(l, u, t) => Some(Element::Image(l, u.to_string(), t.to_string())),
            Tag::List(_) => None,
            Tag::FootnoteDefinition(_) => None,
            Tag::Table(_) => None,
            Tag::TableHead => None,
            Tag::TableRow => None,
            Tag::TableCell => None,
        }
    }
}

impl Element {
    pub fn is_list(&self) -> bool {
        matches!(self, Element::Item)
    }

    pub fn apply_style(&self, text_format: &mut TextFormat, vis: &Appearance) {
        match &self {
            Element::Document => {
                text_format.font_id.size = 16.0;
                text_format.color = vis.text();
            }
            Element::Heading(level) => {
                if level == &HeadingLevel::H1 {
                    text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
                }
                text_format.font_id.size = heading_size(level);
                text_format.color = vis.heading();
            }
            Element::QuoteBlock => {
                text_format.italics = true;
            }
            Element::InlineCode => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.color = vis.code();
                text_format.font_id.size = 14.0;
            }
            Element::Strong => {
                text_format.color = vis.bold();
                text_format.font_id.family = FontFamily::Name(Arc::from("Bold"));
            }
            Element::Emphasis => {
                text_format.color = vis.italics();
                text_format.italics = true;
            }
            Element::Strikethrough => {
                text_format.strikethrough = Stroke { width: 0.5, color: vis.strikethrough() };
            }
            Element::Link(_, _, _) => {
                text_format.color = vis.link();
            }
            Element::CodeBlock => {
                text_format.font_id.family = FontFamily::Monospace;
                text_format.font_id.size = 14.0;
                text_format.color = vis.code();
            }
            Element::Paragraph | Element::Item => {}
            Element::Image(_, _, _) => {
                text_format.italics = true;
            }
            Element::Selection => {
                text_format.background = vis.selection_bg();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq)]
pub enum ItemType {
    Bulleted,
    Numbered(usize),
    Todo(bool),
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
