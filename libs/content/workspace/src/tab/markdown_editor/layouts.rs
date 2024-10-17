use crate::tab::markdown_editor::style::{IndentLevel, ListItem, Title, Url};
use egui::text::LayoutJob;
use egui::TextFormat;
use lb_rs::text::offset_types::{DocCharOffset, RelCharOffset};
use pulldown_cmark::LinkType;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutJobInfo {
    pub range: (DocCharOffset, DocCharOffset),
    pub job: LayoutJob,
    pub annotation: Option<Annotation>,

    // is it better to store this information in Annotation?
    pub head_size: RelCharOffset,
    pub tail_size: RelCharOffset,

    pub annotation_text_format: TextFormat,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Item(ListItem, IndentLevel),
    Image(LinkType, Url, Title),
    BlockQuote,
    CodeBlock,
    HeadingRule,
    Rule,
}
