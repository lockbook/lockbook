use crate::appearance::Appearance;
use crate::buffer::SubBuffer;
use crate::element::{Element, IndentLevel, ItemType, Title, Url};
use crate::offset_types::{DocCharOffset, IntoRangeExt, RangeExt, RelCharOffset};
use crate::styles::StyleInfo;
use crate::Editor;
use egui::text::LayoutJob;
use egui::TextFormat;
use pulldown_cmark::{HeadingLevel, LinkType};
use std::cmp::max;
use std::ops::Index;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Layouts {
    pub layouts: Vec<LayoutJobInfo>,
}

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
            Some(block) => block.append(buffer, vis, style, absorb_terminal_nl),
            None => current = Some(LayoutJobInfo::new(buffer, vis, style, absorb_terminal_nl)),
        };

        if last_item {
            if let Some(block) = current.take() {
                layout.layouts.push(block);
            }
        }
    }

    if buffer.text.ends_with('\n') {
        layout.layouts.push(LayoutJobInfo::new(
            buffer,
            vis,
            &StyleInfo {
                block_start: true,
                range: buffer.segs.last_cursor_position().into_range(),
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

    pub fn layout_at_char(&self, offset: DocCharOffset) -> usize {
        for i in 0..self.layouts.len() {
            if self.layouts[i].range.contains(offset) {
                return i;
            }
        }
        self.layouts.len() - 1
    }
}

impl LayoutJobInfo {
    pub fn new(
        buffer: &SubBuffer, vis: &Appearance, style: &StyleInfo, absorb_terminal_nl: bool,
    ) -> Self {
        let (annotation, head_size, tail_size) =
            Self::annotation_and_head_tail_size(style, buffer, absorb_terminal_nl);
        let text_format = style.text_format(vis);
        let mut result = Self {
            range: style.range,
            job: Default::default(),
            annotation,
            head_size,
            tail_size,
            annotation_text_format: text_format.clone(),
        };
        let range = (style.range.start() + head_size, style.range.end() - tail_size);
        result.job.append(&buffer[range], 0.0, text_format);
        result
    }

    fn append(
        &mut self, buffer: &SubBuffer, vis: &Appearance, style: &StyleInfo,
        absorb_terminal_nl: bool,
    ) {
        self.range = (self.range.0, max(self.range.end(), style.range.end()));
        self.tail_size = Self::tail_size(style, buffer, absorb_terminal_nl);
        self.job.append(
            &buffer[(style.range.start(), style.range.end() - self.tail_size)],
            0.0,
            style.text_format(vis),
        );
    }

    fn annotation_and_head_tail_size(
        style: &StyleInfo, buffer: &SubBuffer, absorb_terminal_nl: bool,
    ) -> (Option<Annotation>, RelCharOffset, RelCharOffset) {
        let (mut annotation, mut head_size) = (None, RelCharOffset(0));
        let text = &buffer[style.range];

        for element in &style.elements {
            if let Element::Image(link_type, url, title) = element {
                // capture image annotation
                annotation =
                    Some(Annotation::Image(*link_type, url.to_string(), title.to_string()));
            }
        }

        if style
            .elements
            .iter()
            .any(|e| matches!(e, &Element::Item(..)))
        {
            let indent_level = style
                .elements
                .iter()
                .filter(|&e| matches!(e, &Element::Item(..)))
                .count() as IndentLevel;
            let text = {
                let trimmed_text = text.trim_start();
                head_size = RelCharOffset(text.len() - trimmed_text.len());
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

        (annotation, head_size, Self::tail_size(style, buffer, absorb_terminal_nl))
    }

    fn tail_size(style: &StyleInfo, buffer: &SubBuffer, absorb_terminal_nl: bool) -> RelCharOffset {
        usize::from(
            style.range.end() > style.range.start()
                && absorb_terminal_nl
                && buffer[style.range].ends_with('\n'),
        )
        .into()
    }

    pub fn size(&self) -> RelCharOffset {
        self.range.end() - self.range.start()
    }

    pub fn head<'b>(&self, buffer: &'b SubBuffer) -> &'b str {
        &buffer[(self.range.start(), self.range.start() + self.head_size)]
    }

    pub fn head_size_chars(&self, buffer: &SubBuffer) -> RelCharOffset {
        self.head(buffer).grapheme_indices(true).count().into()
    }
}

impl Editor {
    pub fn print_layouts(&self) {
        println!("layouts:");
        for layout in &self.layouts.layouts {
            println!(
                "annotation: {:?},\t{:?}{:?}{:?}",
                layout.annotation,
                &self.buffer.current
                    [(layout.range.start(), layout.range.start() + layout.head_size)],
                &self.buffer.current[(
                    layout.range.start() + layout.head_size,
                    layout.range.end() - layout.tail_size
                )],
                &self.buffer.current[(layout.range.end() - layout.tail_size, layout.range.end())],
            );
        }
    }
}
