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
                    Value::List { items, .. } => {
                        for pair in items.windows(2) {
                            out.push((pair[0].text.end(), pair[1].text.start()));
                        }
                        ((key.end(), value.caret()), (value.end(), row.line.end()))
                    }
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
        out.sort_unstable();
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

    /// Where ↑ or ↓ from outside the collapsed block lands in it: the end
    /// of its last row going up, of its first going down, since an x on
    /// the pills means nothing in the grid that opens.
    pub fn front_matter_landing(
        &self, from: Grapheme, to: Grapheme, backwards: bool,
    ) -> Option<Grapheme> {
        let fm = self.bounds.front_matter.as_ref()?;
        let node = fm.node();
        let inside = |o: Grapheme| node.start() <= o && o < node.end();
        if !self.front_matter_collapsed || inside(from) || !inside(to) {
            return None;
        }
        let row = if backwards { fm.rows.last() } else { fm.rows.first() };
        row.map(|row| row.exit())
    }
}
