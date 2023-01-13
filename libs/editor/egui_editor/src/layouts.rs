use crate::appearance::Appearance;
use crate::buffer::Buffer;
use crate::element::{Element, IndentLevel, ItemType, Title, Url};
use crate::offset_types::DocByteOffset;
use crate::styles::StyleInfo;
use egui::text::LayoutJob;
use egui::TextFormat;
use pulldown_cmark::{HeadingLevel, LinkType};
use std::cmp::max;
use std::ops::Range;

#[derive(Clone, Default, PartialEq)]
pub struct LayoutJobInfo {
    pub range: Range<DocByteOffset>,
    pub job: LayoutJob,
    pub annotation: Option<Annotation>,

    // is it better to store this information in Annotation?
    pub head_size: usize,
    pub tail_size: usize,

    pub annotation_text_format: TextFormat,
}

#[derive(Clone, PartialEq)]
pub enum Annotation {
    Item(ItemType, IndentLevel),
    Image(LinkType, Url, Title),
    Rule,
}

impl LayoutJobInfo {
    pub fn new(src: &str, vis: &Appearance, style: &StyleInfo, absorb_terminal_nl: bool) -> Self {
        let (annotation, head_size, tail_size) =
            Self::annotation_and_head_tail_size(style, src, absorb_terminal_nl);
        let text_format = style.text_format(vis);
        let mut result = Self {
            range: style.range.clone(),
            job: Default::default(),
            annotation,
            head_size,
            tail_size,
            annotation_text_format: text_format.clone(),
        };
        let range = Range {
            start: (style.range.start + head_size).0,
            end: (style.range.end - tail_size).0,
        };
        result.job.append(&src[range], 0.0, text_format);
        result
    }

    fn append(&mut self, src: &str, vis: &Appearance, style: &StyleInfo, absorb_terminal_nl: bool) {
        self.range.end = max(self.range.end, style.range.end);
        self.tail_size = Self::tail_size(style, src, absorb_terminal_nl);
        self.job.append(
            &src[style.range.start.0..style.range.end.0 - self.tail_size],
            0.0,
            style.text_format(vis),
        );
    }

    fn annotation_and_head_tail_size(
        style: &StyleInfo, src: &str, absorb_terminal_nl: bool,
    ) -> (Option<Annotation>, usize, usize) {
        let (mut annotation, mut head_size) = (None, 0);
        let text = &src[style.range.start.0..style.range.end.0];
        if style.elements.contains(&Element::Item) {
            let indent_level = style
                .elements
                .iter()
                .filter(|&e| e == &Element::Item)
                .count() as IndentLevel;
            let text = {
                let trimmed_text = text.trim_start();
                head_size = text.len() - trimmed_text.len();
                trimmed_text
            };
            if text.starts_with("+ ") || text.starts_with("* ") || text.starts_with("- ") {
                annotation = Some(Annotation::Item(ItemType::Bulleted, indent_level));
                head_size += 2;
            } else if let Some(prefix) = text.split(". ").next() {
                if let Ok(num) = prefix.parse::<usize>() {
                    annotation = Some(Annotation::Item(ItemType::Numbered(num), indent_level));
                    head_size += prefix.len() + 2;
                }
            }
        } else if style.elements.contains(&Element::Heading(HeadingLevel::H1)) {
            annotation = Some(Annotation::Rule);
        }

        (annotation, head_size, Self::tail_size(style, src, absorb_terminal_nl))
    }

    fn tail_size(style: &StyleInfo, src: &str, absorb_terminal_nl: bool) -> usize {
        usize::from(
            style.range.end > style.range.start
                && absorb_terminal_nl
                && &src[style.range.end.0 - 1..style.range.end.0] == "\n",
        )
    }
}

pub fn calc(buffer: &Buffer, styles: &[StyleInfo], vis: &Appearance) -> Vec<LayoutJobInfo> {
    let mut layout = Vec::new();
    let mut current: Option<LayoutJobInfo> = None;

    for (index, style) in styles.iter().enumerate() {
        let last_item = index == styles.len() - 1;
        if style.block_start {
            if let Some(block) = current.take() {
                layout.push(block);
            }
        }

        // If the next range starts a new block, absorb the terminal newline in this block
        let absorb_terminal_newline = last_item
            || if let Some(next) = styles.get(index + 1) { next.block_start } else { false };

        match &mut current {
            Some(block) => block.append(&buffer.raw, vis, style, absorb_terminal_newline),
            None => {
                current = Some(LayoutJobInfo::new(&buffer.raw, vis, style, absorb_terminal_newline))
            }
        };

        if last_item {
            if let Some(block) = current.take() {
                layout.push(block);
            }
        }
    }

    if buffer.raw.ends_with('\n') {
        layout.push(LayoutJobInfo::new(
            &buffer.raw,
            vis,
            &StyleInfo {
                block_start: true,
                range: Range {
                    start: DocByteOffset(buffer.len()),
                    end: DocByteOffset(buffer.len()),
                },
                elements: vec![Element::Document, Element::Paragraph],
            },
            true,
        ))
    }

    layout
}
