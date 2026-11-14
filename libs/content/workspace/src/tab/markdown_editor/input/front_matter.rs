//! Keys in the front matter grid: Return starts a row, Tab moves between
//! key and value, a colon ends a key, and Backspace stops at the edges of
//! a key or value rather than joining them. Every edit replaces one span.

use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::{Operation, Replace};

use super::{Advance, Bound, Increment, Region};
use crate::front_matter::{FrontMatter, RowKind as Kind, Value};
use crate::tab::markdown_editor::MdEdit;

impl MdEdit {
    /// The front matter and the caret in it, when the selection is a caret
    /// in the block.
    fn front_matter_caret(&self) -> Option<(&FrontMatter<Grapheme>, Grapheme)> {
        let fm = self.renderer.bounds.front_matter.as_ref()?;
        let offset = self.renderer.selection_offset()?;
        (fm.node().start() <= offset && offset <= fm.node().end()).then_some((fm, offset))
    }

    /// The first line after the block with something on it, else the line
    /// after the block.
    fn front_matter_body_start(&self, fm: &FrontMatter<Grapheme>) -> Grapheme {
        let lines = &self.renderer.bounds.source_lines;
        let mut after = lines.iter().filter(|l| l.start() > fm.node().end());
        let first = after.next();
        after
            .chain(first)
            .find(|l| !l.is_empty())
            .or(first)
            .map(|l| l.start())
            .unwrap_or(fm.node().end())
    }

    fn replace(ops: &mut Vec<Operation>, range: (Grapheme, Grapheme), text: &str) {
        ops.push(Operation::Replace(Replace { range, text: text.into() }));
        ops.push(Operation::Select(range.start().into_range()));
    }

    fn select(ops: &mut Vec<Operation>, at: Grapheme) {
        ops.push(Operation::Select(at.into_range()));
    }

    /// A new blank row after `row`, caret on it.
    fn front_matter_new_row(fm: &FrontMatter<Grapheme>, row: usize, ops: &mut Vec<Operation>) {
        let end = fm.rows[row].line.end();
        Self::replace(ops, end.into_range(), "\n");
    }

    /// Return: a new row after this one; on the last row, with nothing on
    /// it, out of the block into the body.
    pub fn front_matter_newline(&self, ops: &mut Vec<Operation>) -> bool {
        let Some((fm, offset)) = self.front_matter_caret() else { return false };
        let Some(i) = fm.row_at(offset) else {
            // in a fence: the caret belongs on the nearest row or the body
            Self::select(
                ops,
                if offset < fm.tail.start() { fm.head.end() } else { fm.node().end() },
            );
            return true;
        };
        let row = &fm.rows[i];
        if row.is_blank() && i + 1 == fm.rows.len() {
            let body = self.front_matter_body_start(fm);
            if fm.rows.len() == 1 {
                Self::select(ops, body);
            } else {
                ops.push(Operation::Replace(Replace {
                    range: (row.line.start() - 1, row.line.start()),
                    text: "".into(),
                }));
                Self::select(ops, body);
            }
            return true;
        }
        Self::front_matter_new_row(fm, i, ops);
        true
    }

    /// Backspace at a row's start, a value's start, or a quote: the row
    /// goes when empty, a caret at a value's start moves to the key, and
    /// nothing joins two rows or a key and its value.
    pub fn front_matter_delete(&self, region: Region, ops: &mut Vec<Operation>) -> bool {
        if !matches!(
            region,
            Region::SelectionOrAdvance {
                advance: Advance::Next(Bound::Word) | Advance::By(Increment::Char),
                backwards: true,
            }
        ) {
            return false;
        }
        let Some(fm) = self.renderer.bounds.front_matter.as_ref() else { return false };
        let Some(offset) = self.renderer.selection_offset() else { return false };
        if offset == fm.node().end() + 1 {
            // the line after the fence: back onto the last row
            let at = fm.rows.last().map_or(fm.head.end(), |r| r.exit());
            Self::select(ops, at);
            return true;
        }
        if offset < fm.node().start() || offset > fm.node().end() {
            return false;
        }
        // inside a fence: nothing to take
        let Some(i) = fm.row_at(offset) else { return true };
        let row = &fm.rows[i];
        if offset == row.line.start() {
            if !row.is_blank() {
                match i.checked_sub(1).map(|above| &fm.rows[above]) {
                    Some(above) if above.is_blank() => {
                        Self::replace(ops, (above.line.start(), row.line.start()), "")
                    }
                    Some(above) => Self::select(ops, above.exit()),
                    None => {}
                }
                return true;
            }
            if fm.rows.len() == 1 {
                // the last empty row takes the block with it
                let body = self.front_matter_body_start(fm);
                Self::replace(ops, (fm.node().start(), body), "");
            } else if i == 0 {
                Self::replace(ops, (row.line.start(), row.line.end() + 1), "");
            } else {
                Self::replace(ops, (row.line.start() - 1, row.line.start()), "");
            }
            return true;
        }
        let Kind::Property { key, sep, value } = &row.kind else { return false };
        match value {
            Value::Empty { at, .. } if offset == *at => {
                Self::replace(ops, (sep.start(), row.line.end()), "");
                let _ = key;
                true
            }
            Value::Text { text, quotes } => {
                if offset == text.start() {
                    Self::select(ops, key.end());
                    return true;
                }
                let Some((open, close)) = quotes else { return false };
                if offset != close.end() && offset != row.line.end() {
                    return false;
                }
                if text.is_empty() {
                    Self::replace(ops, (open.start(), row.line.end()), "");
                } else {
                    Self::replace(ops, (text.end() - 1, text.end()), "");
                }
                true
            }
            _ => false,
        }
    }

    /// Tab: key to value to the next row's key, a new row past the last;
    /// Shift+Tab back. The caret lands at the cell's end, as it does
    /// tabbing into a table cell. On a key being typed, Tab ends it with a
    /// colon.
    pub fn front_matter_indent(&self, deindent: bool, ops: &mut Vec<Operation>) -> bool {
        let Some((fm, offset)) = self.front_matter_caret() else { return false };
        let Some(i) = fm.row_at(offset) else { return true };
        let row = &fm.rows[i];
        if deindent {
            let in_value =
                matches!(&row.kind, Kind::Property { value, .. } if value.caret() <= offset);
            if in_value {
                Self::select(ops, row.cells().first().map_or(row.entry(), |key| key.end()));
            } else if i > 0 {
                Self::select(ops, fm.rows[i - 1].exit());
            }
            return true;
        }
        match &row.kind {
            Kind::Pending { key } => {
                Self::replace(ops, (key.end(), row.line.end()), ": ");
                return true;
            }
            Kind::Property { key, value, .. } if offset <= key.end() => {
                Self::select(ops, value.end());
                return true;
            }
            _ => {}
        }
        match fm.rows.get(i + 1) {
            Some(next) => {
                Self::select(ops, next.cells().first().map_or(next.entry(), |key| key.end()))
            }
            None => Self::front_matter_new_row(fm, i, ops),
        }
        true
    }

    /// Typing a colon at the end of a key being typed makes it `key: `;
    /// typing a value after `key:` puts the space in.
    pub fn front_matter_typed(&self, region: Region, text: &str, ops: &mut Vec<Operation>) -> bool {
        if !matches!(region, Region::Selection) || text.contains('\n') {
            return false;
        }
        let Some((fm, offset)) = self.front_matter_caret() else { return false };
        let Some(i) = fm.row_at(offset) else { return false };
        let row = &fm.rows[i];
        match &row.kind {
            Kind::Pending { key } if text == ":" && offset == key.end() => {
                Self::replace(ops, (key.end(), row.line.end()), ": ");
                true
            }
            Kind::Property { value: Value::Empty { at, needs_space: true }, .. }
                if offset == *at =>
            {
                Self::replace(ops, offset.into_range(), &format!(" {text}"));
                true
            }
            _ => false,
        }
    }
}
