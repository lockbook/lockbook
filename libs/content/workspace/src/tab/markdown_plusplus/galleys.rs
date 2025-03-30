use crate::tab::markdown_plusplus::bounds::Text;
use egui::epaint::text::cursor::Cursor;
use egui::text::CCursor;
use egui::{Galley, Rect};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt, RelCharOffset};
use std::ops::Index;
use std::sync::Arc;

#[derive(Default)]
pub struct Galleys {
    pub galleys: Vec<GalleyInfo>,
}

#[derive(Debug)]
pub struct GalleyInfo {
    pub range: (DocCharOffset, DocCharOffset),
    pub galley: Arc<Galley>,
    pub rect: Rect,
}

impl Index<usize> for Galleys {
    type Output = GalleyInfo;

    fn index(&self, index: usize) -> &Self::Output {
        &self.galleys[index]
    }
}

impl Galleys {
    pub fn is_empty(&self) -> bool {
        self.galleys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.galleys.len()
    }

    pub fn push(&mut self, galley: GalleyInfo) {
        self.galleys.push(galley);
    }

    pub fn galley_at_char(&self, offset: DocCharOffset) -> usize {
        for i in 0..self.galleys.len() {
            let galley = &self.galleys[i];
            if galley.range.contains_inclusive(offset) {
                return i;
            }
        }
        self.galleys.len() - 1
    }

    pub fn galley_and_cursor_by_char_offset(
        &self, char_offset: DocCharOffset, text: &Text,
    ) -> (usize, Cursor) {
        let galley_index = self.galley_at_char(char_offset);
        let galley = &self.galleys[galley_index];
        let galley_text_range = galley.range;
        let char_offset = char_offset.clamp(galley_text_range.start(), galley_text_range.end());

        // adjust for captured syntax chars
        let mut rendered_chars: RelCharOffset = 0.into();
        for text_range in text {
            if text_range.end() <= galley_text_range.start() {
                continue;
            }
            if text_range.start() >= char_offset {
                break;
            }

            let text_range = (
                text_range.start().max(galley_text_range.start()),
                text_range.end().min(char_offset),
            );
            rendered_chars += text_range.len();
        }

        let cursor = galley
            .galley
            .from_ccursor(CCursor { index: rendered_chars.0, prefer_next_row: true });
        (galley_index, cursor)
    }

    pub fn char_offset_by_galley_and_cursor(
        &self, galley_idx: usize, cursor: Cursor, text: &Text,
    ) -> DocCharOffset {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.range;
        let mut result = galley_text_range.start() + cursor.ccursor.index;

        // adjust for captured syntax chars
        let mut last_range: Option<(DocCharOffset, DocCharOffset)> = None;
        for text_range in text {
            if text_range.end() <= galley_text_range.start() {
                continue;
            }

            let text_range = (
                text_range.start().max(galley_text_range.start()),
                text_range.end().min(galley_text_range.end()),
            );
            if let Some(last_range) = last_range {
                result += text_range.start() - last_range.end();
            } else {
                result += text_range.start() - galley_text_range.start();
            }

            if text_range.end() >= result {
                break;
            }

            last_range = Some(text_range);
        }

        // correct for prefer_next_row behavior
        let read_cursor = galley.galley.from_ccursor(CCursor {
            index: (result - galley_text_range.start()).0,
            prefer_next_row: true,
        });
        if read_cursor.rcursor.row > cursor.rcursor.row {
            result -= 1;
        }

        result
    }
}
