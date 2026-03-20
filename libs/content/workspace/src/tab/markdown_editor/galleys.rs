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
            if next_galley.range.start() == galley.range.end() {
                // when galleys touch, the boundary belongs to the later galley
                glyphs.len().saturating_sub(1)
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

            // It seems inoccuous, but this 'if' statement turns out to involve
            // more understanding about the structure of text than anywhere else
            // in the codebase to date.

            // Hopefully by this point you already know that a character is not
            // a byte. Instead, we have unicode. In Unicode, there is a table
            // that assigns a number called a **codepoint** to a symbol it
            // represents. The codepoints are represented in a variable-width
            // format so the most common ones take up less space. So, multiple
            // bytes form a codepoint, and generally you defer to a library to
            // tell you which. We interface with codepoints whenever we
            // interpret the document in any way; all of the document's
            // structure is built on top of codepoints.

            // Sometimes we want to combine codepoints because otherwise the
            // number of required codepoints would be too high. For example,
            // Vietnamese has two types of modifiers/accents that simultaneously
            // apply to any vowel, which would make the number of required
            // codepoints a product of three numbers. Other languages have more
            // dramatic examples. Fortunately we have the **grapheme** which
            // represents a collection of codepoints that form one functional
            // character, the kind you'd like to advance by when you use the
            // arrow keys. Emojis use multi-codepoint graphemes to represent
            // skin tones (which apply to many emojis each) and country flag
            // variations (which would be politically tense to get through the
            // unicode committee). We interface with graphemes when moving the
            // cursor or reading substrings from the text buffer because not all
            // codepoints are valid text boundaries (and when not, they crash
            // the program).

            // Graphemes are not the unit the font system works in. Instead, it
            // works in glyphs. A **glyph** is a unit that's output by the font.
            // It can represent a grapheme (as in "a"), part of a grapheme (as
            // in "´" for an accented character when the font lacks a
            // precomposed form), or multiple graphemes (as in "->" in some code
            // fonts or "fi" in some display fonts). The number and nature of
            // the glyphs for the text will depend on the font used and the
            // support of the font rendering system. We interface with glyphs
            // when making inquiries about the geometry of text, like when
            // drawing the cursor or clicking to place it.

            // In the case of an accented character without a precomposed form,
            // two glyphs are stacked. Together, these form a **glyph cluster**.
            // Fortunately for us, a glyph cluster can represent a grapheme or
            // multiple graphemes, but never only part of a grapheme. You heard
            // that right. Glyph clusters are formed of glyphs which are formed
            // by codepoints, and graphemes are formed of codepoints, so it's
            // kind of special that glyph clusters are not just groups of glyphs
            // but additionally have this relationship with graphemes that makes
            // them essentially a stack of glyphs that represents one grapheme
            // (as in an accented character) or multiple graphemes (as in "->"
            // in some code fonts or "fi" in some display fonts). A boundary
            // between glyphs may not be a boundary between graphemes, but a
            // boundary between glyph clusters always is.

            // In a glyph cluster, cosmic text reports the first glyph as having
            // zero width and attributes the full width to the final glyph -
            // this is best thought of as the width by which to advance the
            // layout cursor after drawing the glyph. We only update
            // `rel_offset` when the width is non-zero because that glyph is the
            // last glyph in the glyph cluster, so its end is also the end of a
            // grapheme. If it was not, the call to `offset_to_char()` below
            // would immediately crash the program.
            if glyph.w > 0. {
                x += glyph.w / self.ctx.pixels_per_point();
                rel_offset = glyph.end;
            }
        }

        self.offset_to_char(start + rel_offset)
    }
}
