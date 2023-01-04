use crate::cursor_types::DocByteOffset;
use crate::editor::Editor;
use crate::element::ItemType::Bulleted;
use crate::element::{Element, IndentLevel, ItemType, Title, Url};
use crate::layout_job::Annotation::Item;
use crate::styled_chunk::StyledChunk;
use crate::theme::VisualAppearance;
use egui::text::LayoutJob;
use pulldown_cmark::LinkType;
use std::cmp::{max, min};
use std::ops::Range;

#[derive(Clone, Default, PartialEq)]
pub struct LayoutJobInfo {
    pub range: Range<DocByteOffset>,
    pub job: LayoutJob,
    pub annotation: Option<Annotation>,
    // is it better to store this information in Annotation?
    pub head_modification: usize,
    pub tail_modification: usize,
}

#[derive(Clone, PartialEq)]
pub enum Annotation {
    Item(ItemType, IndentLevel),
    Image(LinkType, Url, Title),
    Rule,
}

impl LayoutJobInfo {
    pub fn new(
        src: &str, vis: &VisualAppearance, region: &StyledChunk, absorb_terminal_nl: bool,
    ) -> Self {
        let mut ret = Self {
            range: region.range.clone(),
            job: Default::default(),
            annotation: None,
            head_modification: 0,
            tail_modification: 0,
        };

        if region.block_start {
            let item_count = region.item_count();
            if item_count > 0 {
                ret.annotation = Some(Item(Bulleted, item_count as IndentLevel))
            }

            if region
                .elements
                .iter()
                .any(|el| matches!(el, Element::Heading(_)))
            {
                ret.annotation = Some(Annotation::Rule);
            }
        }

        ret.append(src, vis, region, absorb_terminal_nl);
        ret
    }

    pub fn append(
        &mut self, src: &str, vis: &VisualAppearance, data: &StyledChunk, absorb_terminal_nl: bool,
    ) {
        let text_format = data.text_format(vis);

        self.range.start = min(self.range.start, data.range.start);
        self.range.end = max(self.range.end, data.range.end);

        let mut string = &src[data.range.start.0..data.range.end.0];
        let mut range = data.range.clone();

        if let Some(Item(Bulleted, _)) = self.annotation {
            if data.block_start {
                let t = string.trim_start();
                self.head_modification = string.len() - t.len();
                if t.starts_with("+ ") || t.starts_with("* ") || t.starts_with("- ") {
                    self.head_modification += 2;
                } else if t.starts_with('+') || t.starts_with('*') || t.starts_with('-') {
                    self.head_modification += 1;
                }

                range.start += self.head_modification;
            }
        }

        if absorb_terminal_nl && string.ends_with('\n') {
            self.tail_modification = 1;
        }
        range.end -= self.tail_modification;

        string = &src[range.start.0..range.end.0];
        self.job.append(string, 0.0, text_format);
    }
}

impl Editor {
    pub fn populate_layouts(&mut self) {
        self.layout.clear();
        let mut current: Option<LayoutJobInfo> = None;

        for (index, data) in self.styled.iter().enumerate() {
            let last_item = index == self.styled.len() - 1;
            if data.block_start {
                if let Some(block) = current.take() {
                    self.layout.push(block);
                }
            }

            // If the next chunk starts a new block, absorb the terminal newline in this block
            let absorb_ternimal_newline = last_item
                || if let Some(next) = self.styled.get(index + 1) {
                    next.block_start
                } else {
                    false
                };

            match &mut current {
                Some(block) => {
                    block.append(&self.raw, &self.visual_appearance, data, absorb_ternimal_newline)
                }
                None => {
                    current = Some(LayoutJobInfo::new(
                        &self.raw,
                        &self.visual_appearance,
                        data,
                        absorb_ternimal_newline,
                    ))
                }
            };

            if last_item {
                if let Some(block) = current.take() {
                    self.layout.push(block);
                }
            }
        }

        if self.raw.ends_with('\n') {
            self.layout.push(LayoutJobInfo::new(
                &self.raw,
                &self.visual_appearance,
                &StyledChunk {
                    block_start: true,
                    range: Range {
                        start: DocByteOffset(self.raw.len()),
                        end: DocByteOffset(self.raw.len()),
                    },
                    elements: vec![Element::Document, Element::Paragraph],
                },
                true,
            ))
        }
    }
}
