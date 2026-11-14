//! Front matter in the editor: [`crate::front_matter`]'s rows in graphemes,
//! kept in [`Bounds::front_matter`](super::bounds::Bounds::front_matter),
//! and where the caret may sit among them. The grid draws in
//! `widget::block::leaf::front_matter` and takes keys in
//! `input::front_matter`.

use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::front_matter::{self, FrontMatter, RowKind, Value};
use crate::tab::markdown_editor::MdRender;

impl<'ast> MdRender {
    /// Recompute [`Bounds::front_matter`](super::bounds::Bounds) from the
    /// node's text.
    pub fn compute_bounds_front_matter(&mut self, node: &'ast AstNode<'ast>) {
        let node = self.node_range(node);
        let base = self.offset_to_byte(node.start());
        let segs = &self.buffer.current.segs;
        // a byte offset can fall inside a grapheme
        self.bounds.front_matter = front_matter::parse(&self.buffer[node])
            .map(|fm| fm.map(|b| segs.byte_to_char_ceil(base + b)));
    }

    /// The hidden syntax: the fences and, per row, the bytes between the
    /// key and the value and after the value. Atoms for navigation, so
    /// the caret steps from a key's end to its value's start.
    pub fn front_matter_syntax(&self, fm: &FrontMatter<Grapheme>) -> Vec<(Grapheme, Grapheme)> {
        let mut out = vec![];
        if !fm.head.is_empty() {
            out.push(fm.head);
        }
        for (i, row) in fm.rows.iter().enumerate() {
            let (lead, trail) = match &row.kind {
                RowKind::Property { key, value, .. } => match value {
                    Value::Text { text, .. } => {
                        ((key.end(), text.start()), (text.end(), row.line.end()))
                    }
                    Value::Empty { at, .. } => ((key.end(), *at), (*at, *at)),
                },
                RowKind::Pending { key } => ((key.end(), key.end()), (key.end(), row.line.end())),
                // a blank row's spaces draw nothing
                RowKind::Blank => ((row.line.start(), row.line.start()), row.line),
                _ => continue,
            };
            // past a hidden quote or spaces the line's end draws where the
            // value's does, so the caret steps on to the next row
            let next = fm.rows.get(i + 1).map(|r| r.line.start());
            let trail = match next {
                Some(next) if !trail.is_empty() => (trail.start(), next),
                _ => trail,
            };
            for atom in [lead, trail] {
                if !atom.is_empty() {
                    out.push(atom);
                }
            }
        }
        if !fm.tail.is_empty() {
            out.push(fm.tail);
        }
        out
    }

    /// Advances an offset strictly inside hidden front matter syntax to
    /// the syntax's edge in the travel direction.
    pub fn snap_offset_out_of_front_matter(&self, offset: Grapheme, backwards: bool) -> Grapheme {
        let Some(fm) = &self.bounds.front_matter else { return offset };
        for atom in self.front_matter_syntax(fm) {
            if atom.start() < offset && offset < atom.end() {
                return if backwards { atom.start() } else { atom.end() };
            }
        }
        offset
    }
}
