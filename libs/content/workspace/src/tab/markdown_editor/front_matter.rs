//! Front matter in the editor: [`crate::front_matter`]'s rows in graphemes,
//! kept in [`Bounds::front_matter`](super::bounds::Bounds::front_matter),
//! and where the caret may sit among them. The grid draws in
//! `widget::block::leaf::front_matter` and takes keys in
//! `input::front_matter`.

use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::buffer;
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::front_matter::{self, FrontMatter, RowKind, Value};
use crate::tab::markdown_editor::{MdEdit, MdRender, View};

impl<'ast> MdRender {
    /// Recompute [`Bounds::front_matter`](super::bounds::Bounds). Depends
    /// on text only, so source view has them for its front matter edits too.
    pub fn calc_front_matter_bounds(&mut self, root: &'ast AstNode<'ast>) {
        let node = root
            .first_child()
            .filter(|node| matches!(node.data.borrow().value, NodeValue::FrontMatter(_)));
        self.bounds.front_matter = node.and_then(|node| {
            let node = self.node_range(node);
            let base = self.offset_to_byte(node.start());
            let segs = &self.buffer.current.segs;
            // a byte offset can fall inside a grapheme
            Some(front_matter::parse(&self.buffer[node])?.map(|b| segs.byte_to_char_ceil(base + b)))
        });
    }

    /// The start of the line after the closing fence, or the block's end
    /// when the document ends there.
    pub fn front_matter_body(&self, fm: &FrontMatter<Grapheme>) -> Grapheme {
        (fm.node().end() + 1).min(self.last_cursor_position())
    }

    /// The hidden syntax: the fences and, per row, the bytes between the
    /// key and the value and after the value. Atoms for navigation, so
    /// the caret steps from a key's end to its value's start. The closing
    /// fence reaches from the last row's end to the next line, so no caret
    /// position hides between that row and the body.
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
        if !fm.tail.is_empty() {
            let start = fm.rows.last().map_or(fm.tail.start(), |r| r.exit());
            let tail = (start, self.front_matter_body(fm).max(fm.tail.end()));
            out.retain(|a| !(tail.start() <= a.start() && a.end() <= tail.end()));
            out.push(tail);
        }
        out.sort_unstable();
        out
    }

    /// Advances an offset strictly inside hidden front matter syntax to
    /// the syntax's edge in the travel direction. Nothing precedes the
    /// opening fence, so the document's start is its end either way.
    pub fn snap_offset_out_of_front_matter(&self, offset: Grapheme, backwards: bool) -> Grapheme {
        let Some(fm) = self
            .bounds
            .front_matter
            .as_ref()
            .filter(|_| self.view != View::Source)
        else {
            return offset;
        };
        if !fm.head.is_empty() && fm.head.start() <= offset && offset < fm.head.end() {
            return fm.head.end();
        }
        for atom in self.front_matter_syntax(fm) {
            if atom.start() < offset && offset < atom.end() {
                return if backwards { atom.start() } else { atom.end() };
            }
        }
        offset
    }

    /// A caret put down inside a key's hidden separator, by a click or the
    /// platform, lands at the key's value, where a keystroke edits the
    /// value rather than the syntax.
    pub fn snap_caret_out_of_front_matter_separator(
        &self, selection: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let Some(fm) = self
            .bounds
            .front_matter
            .as_ref()
            .filter(|_| self.view != View::Source && selection.is_empty())
        else {
            return selection;
        };
        let at = selection.start();
        fm.rows
            .iter()
            .find_map(|row| match &row.kind {
                RowKind::Property { key, value, .. } if key.end() < at && at < value.caret() => {
                    Some(value.caret().into_range())
                }
                _ => None,
            })
            .unwrap_or(selection)
    }

    /// Where ↑ or ↓ from outside the collapsed block lands in it: the end
    /// of its last row going up, of its first going down, since an x on
    /// the pills means nothing in the grid that opens.
    pub fn front_matter_landing(
        &self, from: Grapheme, to: Grapheme, backwards: bool,
    ) -> Option<Grapheme> {
        let fm = self
            .bounds
            .front_matter
            .as_ref()
            .filter(|_| self.view != View::Source)?;
        let node = fm.node();
        let inside = |o: Grapheme| node.start() <= o && o < node.end();
        if !self.front_matter_collapsed || inside(from) || !inside(to) {
            return None;
        }
        let row = if backwards { fm.rows.last() } else { fm.rows.first() };
        row.map(|row| row.exit())
    }
}

impl MdEdit {
    /// A caret before the opening fence, or a selection strictly inside
    /// hidden syntax, moves to a caret the eye can see. A note just opened
    /// puts the caret before the fence, and a click on a hidden span (the
    /// end of the last row, say) selects inside it; either way a keystroke
    /// would replace syntax. A selection from the keyboard keeps an end at
    /// the span's edge, so it stays to be extended. The gap after a key
    /// lands at its value, the opening fence at the first row, and anything
    /// else at the text before it.
    pub fn rehome_front_matter_caret(&mut self, undo_redo: bool) -> buffer::Response {
        let selection = self.renderer.buffer.current.selection;
        let (start, end) = (selection.start(), selection.end());
        let Some(fm) = &self.renderer.bounds.front_matter else {
            return buffer::Response::default();
        };
        let home = if !fm.head.is_empty() && start < fm.head.end() && end <= fm.head.end() {
            Some(fm.head.end())
        } else {
            let key_ends: Vec<Grapheme> = fm
                .rows
                .iter()
                .filter_map(|r| match &r.kind {
                    RowKind::Property { key, .. } => Some(key.end()),
                    _ => None,
                })
                .collect();
            self.renderer
                .front_matter_syntax(fm)
                .into_iter()
                .find(|a| start < end && a.start() < start && end < a.end())
                .map(|a| if key_ends.contains(&a.start()) { a.end() } else { a.start() })
        };
        match home {
            Some(home) if !undo_redo && selection != (home, home) => {
                self.renderer
                    .buffer
                    .queue(vec![Operation::Select(home.into_range())], self.renderer.now());
                self.renderer.buffer.update()
            }
            _ => buffer::Response::default(),
        }
    }
}
