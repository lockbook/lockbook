use egui::epaint::text::cursor::Cursor;
use egui::text::CCursor;
use egui::{Galley, Rect};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};
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
    pub padded: bool,
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

    pub fn galley_at_offset(&self, offset: DocCharOffset) -> Option<usize> {
        for i in (0..self.galleys.len()).rev() {
            let galley = &self.galleys[i];
            if galley.range.contains_inclusive(offset) {
                return Some(i);
            }
        }
        None
    }

    pub fn galley_and_cursor_by_offset(&self, offset: DocCharOffset) -> Option<(usize, Cursor)> {
        let galley_index = self.galley_at_offset(offset)?;
        let galley = &self.galleys[galley_index];

        let cursor = galley.galley.from_ccursor(CCursor {
            index: (offset - galley.range.start()).0,
            prefer_next_row: true,
        });
        Some((galley_index, cursor))
    }

    pub fn offset_by_galley_and_cursor(
        &self, galley: &GalleyInfo, cursor: Cursor,
    ) -> DocCharOffset {
        let galley_text_range = galley.range;
        let mut result = galley_text_range.start() + cursor.ccursor.index;

        // correct for prefer_next_row behavior
        let read_cursor = galley.galley.from_ccursor(CCursor {
            index: (result - galley_text_range.start()).0,
            prefer_next_row: true,
        });
        if read_cursor.rcursor.row > cursor.rcursor.row {
            result -= 1;
        }

        result.max(galley.range.start()).min(galley.range.end())
    }
}
