use crate::appearance::Appearance;
use crate::buffer::SubBuffer;
use crate::element::{Element, IndentLevel, ItemType, Title, Url};
use crate::offset_types::{DocByteOffset, DocCharOffset, RelByteOffset, RelCharOffset};
use crate::styles::StyleInfo;
use crate::unicode_segs::UnicodeSegs;
use crate::Editor;
use egui::text::LayoutJob;
use egui::TextFormat;
use pulldown_cmark::{HeadingLevel, LinkType};
use std::cmp::max;
use std::ops::{Index, Range};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Layouts {
    pub layouts: Vec<LayoutJobInfo>,
}

#[derive(Clone, Default, PartialEq)]
pub struct LayoutJobInfo {
    pub range: Range<DocByteOffset>,
    pub job: LayoutJob,
    pub annotation: Option<Annotation>,

    // is it better to store this information in Annotation?
    pub head_size: RelByteOffset,
    pub tail_size: RelByteOffset,

    pub annotation_text_format: TextFormat,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Item(ItemType, IndentLevel),
    Image(LinkType, Url, Title),
    Rule,
}

pub fn calc(buffer: &SubBuffer, styles: &[StyleInfo], vis: &Appearance) -> Layouts {
    let mut layout = Layouts::default();
    let mut current: Option<LayoutJobInfo> = None;

    for (index, style) in styles.iter().enumerate() {
        let last_item = index == styles.len() - 1;
        if style.block_start {
            if let Some(block) = current.take() {
                layout.layouts.push(block);
            }
        }

        // If the next range starts a new block, absorb the terminal newline in this block
        let absorb_terminal_nl = last_item
            || if let Some(next) = styles.get(index + 1) { next.block_start } else { false };

        match &mut current {
            Some(block) => block.append(&buffer.text, vis, style, absorb_terminal_nl),
            None => {
                current = Some(LayoutJobInfo::new(&buffer.text, vis, style, absorb_terminal_nl))
            }
        };

        if last_item {
            if let Some(block) = current.take() {
                layout.layouts.push(block);
            }
        }
    }

    if buffer.text.ends_with('\n') {
        layout.layouts.push(LayoutJobInfo::new(
            &buffer.text,
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

impl Index<usize> for Layouts {
    type Output = LayoutJobInfo;

    fn index(&self, index: usize) -> &Self::Output {
        &self.layouts[index]
    }
}

impl Layouts {
    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.layouts.len()
    }

    pub fn layout_at_char(&self, char_index: DocCharOffset, segs: &UnicodeSegs) -> usize {
        let byte_offset = segs.char_offset_to_byte(char_index);
        for i in 0..self.layouts.len() {
            let galley = &self.layouts[i];
            if galley.range.start <= byte_offset && byte_offset < galley.range.end {
                return i;
            }
        }
        self.layouts.len() - 1
    }
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
            &src[style.range.start.0..(style.range.end - self.tail_size).0],
            0.0,
            style.text_format(vis),
        );
    }

    fn annotation_and_head_tail_size(
        style: &StyleInfo, src: &str, absorb_terminal_nl: bool,
    ) -> (Option<Annotation>, RelByteOffset, RelByteOffset) {
        let (mut annotation, mut head_size) = (None, RelByteOffset(0));
        let text = &src[style.range.start.0..style.range.end.0];

        for element in &style.elements {
            if let Element::Image(link_type, url, title) = element {
                // capture image annotation
                annotation =
                    Some(Annotation::Image(*link_type, url.to_string(), title.to_string()));
            }
        }
        if style.elements.contains(&Element::Item) {
            let indent_level = style
                .elements
                .iter()
                .filter(|&e| e == &Element::Item)
                .count() as IndentLevel;
            let text = {
                let trimmed_text = text.trim_start();
                head_size = RelByteOffset(text.len() - trimmed_text.len());
                trimmed_text
            };

            // capture unchecked task list annotation
            if text.starts_with("+ [ ] ")
                || text.starts_with("* [ ] ")
                || text.starts_with("- [ ] ")
            {
                annotation = Some(Annotation::Item(ItemType::Todo(false), indent_level));
                head_size += 6;
            }
            // capture checked task list annotation
            else if text.starts_with("+ [x] ")
                || text.starts_with("* [x] ")
                || text.starts_with("- [x] ")
            {
                annotation = Some(Annotation::Item(ItemType::Todo(true), indent_level));
                head_size += 6;
            }
            // capture bulleted list annotation
            else if text.starts_with("+ ") || text.starts_with("* ") || text.starts_with("- ") {
                annotation = Some(Annotation::Item(ItemType::Bulleted, indent_level));
                head_size += 2;
            }
            // capture numbered list annotation
            else if let Some(prefix) = text.split(". ").next() {
                if let Ok(num) = prefix.parse::<usize>() {
                    annotation = Some(Annotation::Item(ItemType::Numbered(num), indent_level));
                    head_size += prefix.len() + 2;
                }
            }
        } else if let Some(heading_element) = style
            .elements
            .iter()
            .filter_map(|e| {
                if let Element::Heading(heading_level) = e {
                    Some(heading_level)
                } else {
                    None
                }
            })
            .next()
        {
            match heading_element {
                HeadingLevel::H1 => {
                    annotation = Some(Annotation::Rule);
                    head_size += 1
                }
                HeadingLevel::H2 => head_size += 2,
                HeadingLevel::H3 => head_size += 3,
                HeadingLevel::H4 => head_size += 4,
                HeadingLevel::H5 => head_size += 5,
                HeadingLevel::H6 => head_size += 6,
            }
            if text.starts_with(&("#".repeat(head_size.0) + " ")) {
                head_size += 1;
            } else {
                head_size = 0.into();
            }
        }

        (annotation, head_size, Self::tail_size(style, src, absorb_terminal_nl))
    }

    fn tail_size(style: &StyleInfo, src: &str, absorb_terminal_nl: bool) -> RelByteOffset {
        usize::from(
            style.range.end > style.range.start
                && absorb_terminal_nl
                && src[style.range.start.0..style.range.end.0].ends_with('\n'),
        )
        .into()
    }

    pub fn size(&self) -> RelByteOffset {
        self.range.end - self.range.start
    }

    pub fn head<'b>(&self, buffer: &'b SubBuffer) -> &'b str {
        &buffer.text[(self.range.start).0..(self.range.start + self.head_size).0]
    }

    pub fn head_size_chars(&self, buffer: &SubBuffer) -> RelCharOffset {
        UnicodeSegmentation::grapheme_indices(self.head(buffer), true)
            .count()
            .into()
    }
}

impl Editor {
    pub fn print_layouts(&self) {
        println!("layouts:");
        for layout in &self.layouts.layouts {
            println!(
                "annotation: {:?},\t{:?}{:?}{:?}",
                layout.annotation,
                &self.buffer.current.text
                    [layout.range.start.0..layout.range.start.0 + layout.head_size.0],
                &self.buffer.current.text[layout.range.start.0 + layout.head_size.0
                    ..layout.range.end.0 - layout.tail_size.0],
                &self.buffer.current.text
                    [layout.range.end.0 - layout.tail_size.0..layout.range.end.0],
            );
        }
    }
}
