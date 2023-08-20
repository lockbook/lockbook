use crate::offset_types::{DocCharOffset, RelCharOffset};
use crate::style::{IndentLevel, ListItem, Title, Url};
use egui::text::LayoutJob;
use egui::TextFormat;
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
    HeadingRule,
    Rule,
}
