use egui::Rect;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};
use std::ops::Index;
use std::sync::{Arc, RwLock};

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

impl GalleyInfo {
    /// Returns the x position of the offset, assuming the offset lies in this galley.
    pub fn x(&self, offset: DocCharOffset) -> f32 {
        // todo: assumes one glyph per unicode segment
        let rel_offset = offset - self.range.start();
        let mut rel_x = 0.;
        if rel_offset > 0 {
            let buffer = self.buffer.read().unwrap();
            let glyphs = buffer.layout_runs().next().unwrap().glyphs;
            for glyph in glyphs.iter().take(rel_offset.0) {
                rel_x += glyph.w;
            }
        }

        self.rect.min.x + rel_x
    }
}
