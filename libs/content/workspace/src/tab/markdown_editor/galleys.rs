use egui::{Pos2, Rect};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};
use std::ops::Index;
use std::sync::{Arc, RwLock};

use crate::tab::markdown_editor::Editor;

#[derive(Default)]
pub struct Galleys {
    pub galleys: Vec<GalleyInfo>,
}

#[derive(Debug)]
pub struct GalleyInfo {
    pub is_override: bool,
    pub range: (DocCharOffset, DocCharOffset),
    pub buffer: Arc<RwLock<glyphon::Buffer>>,
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
}

impl Editor {
    /// Returns the x position of the offset, assuming the offset lies in this
    /// galley. For the y position, use self.rect.y_range().
    pub fn galley_x(&self, galley: &GalleyInfo, offset: DocCharOffset) -> f32 {
        let buffer = galley.buffer.read().unwrap();
        let glyphs = buffer.layout_runs().next().unwrap().glyphs;

        let rel_offset = self.range_to_byte((galley.range.start(), offset)).len();
        let mut rel_x = 0.;

        for glyph in glyphs {
            if glyph.end > rel_offset {
                break;
            }
            rel_x += glyph.w / self.ctx.pixels_per_point();
        }

        galley.rect.min.x + rel_x
    }

    /// Returns the offset closest to pos in this galley, excluding the offset
    /// after the last glyph.
    pub fn galley_offset(&self, galley_idx: usize, pos: Pos2) -> DocCharOffset {
        let galley = &self.galleys.galleys[galley_idx];
        let buffer = galley.buffer.read().unwrap();
        let layout_run = buffer.layout_runs().next().unwrap();
        let glyphs = layout_run.glyphs;

        let rel_x = pos.x - galley.rect.min.x;
        let start = self.offset_to_byte(galley.range.start());

        let mut rel_offset = 0;
        let mut x = 0.;

        // prefer next row
        let owned_glyphs = if self.galleys.len() > galley_idx + 1 {
            let next_galley = &self.galleys.galleys[galley_idx + 1];
            if next_galley.range.start() == galley.range.end() && glyphs.len() > 1 {
                // when galleys touch, the boundary belongs to the later galley;
                // only reduce if there are at least 2 glyphs so we never get 0
                // owned glyphs (which would snap the cursor to the galley start)
                glyphs.len() - 1
            } else {
                // doesn't touch next galley
                glyphs.len()
            }
        } else {
            // no further galleys
            glyphs.len()
        };

        for glyph in glyphs.iter().take(owned_glyphs) {
            if x + glyph.w / self.ctx.pixels_per_point() / 2. > rel_x {
                break;
            }
            x += glyph.w / self.ctx.pixels_per_point();
            rel_offset = glyph.end;
        }

        // Convert the byte offset back to a char offset. For ZWJ emoji sequences
        // the font may emit one glyph per codepoint, so glyph.end can land on a
        // codepoint boundary that is not a grapheme boundary. Round down to the
        // nearest grapheme boundary rather than panicking.
        let byte_offset = start + rel_offset;
        let segs = &self.buffer.current.segs;
        match segs.grapheme_indexes.binary_search(&byte_offset) {
            Ok(idx) => DocCharOffset(idx),
            Err(idx) => DocCharOffset(idx.saturating_sub(1)),
        }
    }
}
