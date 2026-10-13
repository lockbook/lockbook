//! UIKit counts text positions in UTF-16 units, as `NSString` does; the
//! editor counts graphemes. Positions cross this boundary as UTF-16.

use lb_c::model::text::offset_types::{Byte, Grapheme};
use workspace_rs::tab::markdown_editor::MdEdit;

use super::response::{CTextPosition, CTextRange};

pub fn utf16_of(md: &MdEdit, g: Grapheme) -> usize {
    let snap = &md.renderer.buffer.current;
    let g = g.min(snap.segs.last_cursor_position());
    let byte = snap.segs.offset_to_byte(g).0;
    snap.text[..byte].encode_utf16().count()
}

/// An offset inside a grapheme snaps to its start, or its end with `ceil`.
pub fn grapheme_of(md: &MdEdit, utf16: usize, ceil: bool) -> Grapheme {
    let snap = &md.renderer.buffer.current;
    let mut units = 0;
    let mut byte = snap.text.len();
    for (i, c) in snap.text.char_indices() {
        if units >= utf16 {
            byte = i;
            break;
        }
        units += c.len_utf16();
        if units > utf16 {
            byte = if ceil { i + c.len_utf8() } else { i };
            break;
        }
    }
    if ceil {
        snap.segs.byte_to_char_ceil(Byte(byte))
    } else {
        snap.segs.byte_to_char_floor(Byte(byte))
    }
}

pub fn position(md: &MdEdit, g: Grapheme) -> CTextPosition {
    CTextPosition { none: false, pos: utf16_of(md, g) }
}

pub fn grapheme_at(md: &MdEdit, pos: &CTextPosition) -> Option<Grapheme> {
    (!pos.none).then(|| grapheme_of(md, pos.pos, false))
}

/// Ordered, widened to whole graphemes.
pub fn graphemes_in(md: &MdEdit, range: &CTextRange) -> Option<(Grapheme, Grapheme)> {
    if range.none {
        return None;
    }
    let (lo, hi) = (range.start.pos.min(range.end.pos), range.start.pos.max(range.end.pos));
    let start = grapheme_of(md, lo, false);
    Some((start, if lo == hi { start } else { grapheme_of(md, hi, true) }))
}
