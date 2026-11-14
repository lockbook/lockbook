//! Keys in the front matter grid: Return starts a row or a list item, Tab
//! moves between key and value, a colon ends a key, a comma ends an item,
//! and Backspace stops at the edges of a key, value, or item rather than
//! joining them. A value that comes to need quotes gets them as it is
//! typed. Every edit replaces one span.

use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::{Operation, Replace};
use unicode_segmentation::UnicodeSegmentation as _;

use super::{Advance, Bound, FrontMatterAction, Increment, Region};
use crate::front_matter::{FrontMatter, ListStyle, Quotes, RowKind as Kind, Value, needs_quotes};
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

    /// A new item after item `j` of row `i`; on an empty last item, a new
    /// row in its place.
    fn front_matter_new_item(
        &self, fm: &FrontMatter<Grapheme>, i: usize, j: usize, ops: &mut Vec<Operation>,
    ) {
        let row = &fm.rows[i];
        let Kind::Property { value: Value::List { style, items, .. }, .. } = &row.kind else {
            return;
        };
        let item = &items[j];
        if item.text.is_empty() {
            if j + 1 == items.len() {
                match style {
                    ListStyle::Block => {
                        Self::replace(ops, (item.lead.start() - 1, row.line.end()), "\n")
                    }
                    ListStyle::Flow => {
                        ops.push(Operation::Replace(Replace {
                            range: (item.lead.start(), item.written().end()),
                            text: "".into(),
                        }));
                        Self::replace(ops, row.line.end().into_range(), "\n");
                    }
                }
            }
            return;
        }
        match style {
            ListStyle::Block => {
                let line_end = items
                    .get(j + 1)
                    .map_or(row.line.end(), |next| next.lead.start() - 1);
                let lead = &self.renderer.buffer[item.lead];
                Self::replace(ops, line_end.into_range(), &format!("\n{lead}"));
            }
            ListStyle::Flow => Self::replace(ops, item.written().end().into_range(), ", "),
        }
    }

    /// Return: a new row after this one, or before it from its start,
    /// taking an empty last list item with it (a comma starts an item); on
    /// the last row, with nothing on it, out of the block into the body.
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
        if offset == row.line.start() && !row.is_blank() {
            Self::replace(ops, offset.into_range(), "\n");
            return true;
        }
        if let Kind::Property { value: value @ Value::List { items, .. }, .. } = &row.kind {
            if let Some(j) = value.item_at(offset) {
                if items[j].text.is_empty() && j + 1 == items.len() {
                    self.front_matter_new_item(fm, i, j, ops);
                    return true;
                }
            }
        }
        Self::front_matter_new_row(fm, i, ops);
        true
    }

    /// Backspace at a row's start, a value's start, an item's start, or a
    /// quote: an empty row or item goes, a caret at a value's start moves
    /// to the key, and nothing joins two rows or a key and its value.
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
            self.front_matter_action(FrontMatterAction::Remove(i), ops);
            return true;
        }
        let Kind::Property { key, sep, value } = &row.kind else { return false };
        match value {
            Value::Empty { at, .. } if offset == *at => {
                Self::replace(ops, (sep.start(), row.line.end()), "");
                true
            }
            Value::Text { text, quotes, .. } => {
                if offset == text.start() {
                    Self::select(ops, key.end());
                    return true;
                }
                Self::front_matter_delete_at_quote(*text, *quotes, offset, row.line.end(), ops)
            }
            Value::List { style, items, open, .. } => {
                let Some(j) = value.item_at(offset) else { return false };
                let item = &items[j];
                if offset != item.text.start() {
                    return Self::front_matter_delete_at_quote(
                        item.text,
                        item.quotes,
                        offset,
                        items
                            .get(j + 1)
                            .map_or(row.line.end(), |next| next.lead.start()),
                        ops,
                    );
                }
                if !item.text.is_empty() {
                    Self::select(ops, if j == 0 { key.end() } else { items[j - 1].text.end() });
                    return true;
                }
                // an empty item goes: its line, or its comma, or the comma
                // after it when it is first
                let range = match style {
                    ListStyle::Block => (item.lead.start() - 1, item.written().end()),
                    ListStyle::Flow => match items.get(j + 1) {
                        Some(next) if item.lead.start() == *open + 1 => {
                            (item.lead.start(), next.written().start())
                        }
                        _ => (item.lead.start(), item.written().end()),
                    },
                };
                Self::replace(ops, range, "");
                true
            }
            _ => false,
        }
    }

    /// Backspace right of a closing quote takes the last character inside
    /// it, or the quotes when nothing is left.
    fn front_matter_delete_at_quote(
        text: (Grapheme, Grapheme), quotes: Quotes<Grapheme>, offset: Grapheme,
        trail_end: Grapheme, ops: &mut Vec<Operation>,
    ) -> bool {
        let Some((open, close)) = quotes else { return false };
        if offset != close.end() && offset != trail_end {
            return false;
        }
        if text.is_empty() {
            Self::replace(ops, (open.start(), close.end()), "");
        } else {
            Self::replace(ops, (text.end() - 1, text.end()), "");
        }
        true
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

    /// Typing a colon at the end of a key being typed makes it `key: `; a
    /// value typed after `key:` gets its space; a comma at an item's end
    /// starts the next item; a plain
    /// value that comes to need quotes gets them, and a quote typed inside
    /// a quoted value is escaped.
    pub fn front_matter_typed(
        &mut self, region: Region, text: &str, ops: &mut Vec<Operation>,
    ) -> bool {
        if !matches!(region, Region::Selection) || text.contains('\n') {
            return false;
        }
        let Some((fm, offset)) = self.front_matter_caret() else { return false };
        let Some(i) = fm.row_at(offset) else { return false };
        let row = &fm.rows[i];
        let value = match &row.kind {
            Kind::Pending { key } if text == ":" && offset == key.end() => {
                Self::replace(ops, (key.end(), row.line.end()), ": ");
                return true;
            }
            Kind::Property { value, .. } => value,
            _ => return false,
        };
        match value {
            Value::Empty { at, needs_space: true } if offset == *at => {
                Self::replace(ops, offset.into_range(), &format!(" {text}"));
                true
            }
            Value::Empty { .. } => false,
            Value::Text { text: range, quotes, .. } => self.front_matter_type_into(
                *range,
                *quotes,
                row.line.end(),
                offset,
                text,
                false,
                ops,
            ),
            Value::List { style, items, close, .. } => {
                let Some(j) = value.item_at(offset).or_else(|| {
                    // a caret in the spaces after an item is still on it
                    items.iter().rposition(|it| it.text.end() <= offset)
                }) else {
                    return false;
                };
                let item = &items[j];
                if text == "," && offset == item.text.end() && item.quotes.is_none() {
                    self.front_matter_new_item(fm, i, j, ops);
                    return true;
                }
                let flow = *style == ListStyle::Flow;
                // spaces typed before the next comma or the line's end
                // still belong to this item
                let trail_end = match items.get(j + 1) {
                    Some(next) => {
                        let lead = &self.renderer.buffer[next.lead];
                        next.lead.start() + lead.find(',').unwrap_or(0)
                    }
                    None if flow => *close,
                    None => row.line.end(),
                };
                self.front_matter_type_into(
                    item.text,
                    item.quotes,
                    trail_end,
                    offset,
                    text,
                    flow,
                    ops,
                )
            }
        }
    }

    /// Typing into a scalar: quotes it when the result needs them, escapes
    /// a quote typed inside quotes, else types as usual.
    #[allow(clippy::too_many_arguments)]
    fn front_matter_type_into(
        &self, range: (Grapheme, Grapheme), quotes: Quotes<Grapheme>, trail_end: Grapheme,
        offset: Grapheme, typed: &str, flow: bool, ops: &mut Vec<Operation>,
    ) -> bool {
        let trail_end = trail_end.max(range.end());
        if offset < range.start()
            || offset > trail_end
            || (quotes.is_some() && offset > range.end())
        {
            return false;
        }
        // spaces typed past the end are part of the value
        let range = (range.start(), range.end().max(offset));
        let buffer = &self.renderer.buffer;
        if let Some((open, _)) = quotes {
            let escaped = match &buffer[open] {
                "\"" => typed.replace('\\', "\\\\").replace('"', "\\\""),
                _ => typed.replace('\'', "''"),
            };
            if escaped == typed {
                return false;
            }
            Self::replace(ops, offset.into_range(), &escaped);
            return true;
        }
        let current = &buffer[range];
        let at = buffer[(range.start(), offset)].graphemes(true).count();
        let byte = current
            .grapheme_indices(true)
            .nth(at)
            .map_or(current.len(), |(b, _)| b);
        let after = format!("{}{typed}{}", &current[..byte], &current[byte..]);
        if !needs_quotes(&after, flow) {
            return false;
        }
        // open quote, escapes before the caret, the typed text, the caret,
        // then escapes after it and the close quote, which the caret op so
        // stays before
        let escapes = |ops: &mut Vec<Operation>, after: bool| {
            for (k, g) in current.graphemes(true).enumerate() {
                let escaped = match g {
                    "\"" => "\\\"",
                    "\\" => "\\\\",
                    _ => continue,
                };
                let at = range.start() + k;
                if (at >= offset) == after {
                    ops.push(Operation::Replace(Replace {
                        range: (at, at + 1),
                        text: escaped.into(),
                    }));
                }
            }
        };
        ops.push(Operation::Replace(Replace {
            range: range.start().into_range(),
            text: "\"".into(),
        }));
        escapes(ops, false);
        let typed = typed.replace('\\', "\\\\").replace('"', "\\\"");
        ops.push(Operation::Replace(Replace { range: offset.into_range(), text: typed }));
        ops.push(Operation::Select(offset.into_range()));
        escapes(ops, true);
        ops.push(Operation::Replace(Replace {
            range: range.end().into_range(),
            text: "\"".into(),
        }));
        true
    }
}

impl MdEdit {
    /// The front matter checkbox whose value starts at `check`, and whether
    /// it is checked.
    pub fn front_matter_checkbox_at(
        &self, check: Grapheme,
    ) -> Option<((Grapheme, Grapheme), bool)> {
        let fm = self.renderer.bounds.front_matter.as_ref()?;
        fm.rows.iter().find_map(|row| match &row.kind {
            Kind::Property { value: Value::Text { text, checked: Some(checked), .. }, .. }
                if text.start() == check =>
            {
                Some((*text, *checked))
            }
            _ => None,
        })
    }

    /// Whether the caret is in the front matter block.
    pub fn in_front_matter(&self) -> bool {
        self.front_matter_caret().is_some()
    }

    /// The toolbar's and row menu's commands.
    pub fn front_matter_action(&self, action: FrontMatterAction, ops: &mut Vec<Operation>) {
        let fm = self.renderer.bounds.front_matter.as_ref();
        let buffer = &self.renderer.buffer;
        match (action, fm) {
            (FrontMatterAction::Add, None) => {
                // the caret op sits between the two halves so it lands on
                // the empty row between the fences
                let at = Grapheme(0).into_range();
                ops.push(Operation::Replace(Replace { range: at, text: "---\n".into() }));
                ops.push(Operation::Select(at));
                ops.push(Operation::Replace(Replace { range: at, text: "\n---\n".into() }));
            }
            (FrontMatterAction::Add, Some(fm)) => {
                let offset = self.renderer.selection_offset();
                let row = offset.and_then(|o| fm.row_at(o));
                match (row, fm.rows.last()) {
                    (Some(i), _) => Self::front_matter_new_row(fm, i, ops),
                    (None, Some(last)) if last.is_blank() => Self::select(ops, last.line.start()),
                    (None, Some(_)) => Self::front_matter_new_row(fm, fm.rows.len() - 1, ops),
                    (None, None) => Self::select(ops, fm.head.end()),
                }
            }
            (FrontMatterAction::Remove(i), Some(fm)) => {
                let Some(row) = fm.rows.get(i) else { return };
                if fm.rows.len() == 1 {
                    // the last row takes the block with it
                    let body = self.front_matter_body_start(fm);
                    Self::replace(ops, (fm.node().start(), body), "");
                } else if i == 0 {
                    Self::replace(ops, (row.line.start(), row.line.end() + 1), "");
                } else {
                    Self::replace(ops, (row.line.start() - 1, row.line.end()), "");
                }
            }
            // the other row moves around this one, so the caret stays put
            (FrontMatterAction::MoveUp(i), Some(fm)) => {
                let (Some(prev), Some(row)) =
                    (i.checked_sub(1).and_then(|p| fm.rows.get(p)), fm.rows.get(i))
                else {
                    return;
                };
                let text = buffer[prev.line].to_string();
                ops.push(Operation::Replace(Replace {
                    range: (prev.line.start(), row.line.start()),
                    text: "".into(),
                }));
                ops.push(Operation::Replace(Replace {
                    range: row.line.end().into_range(),
                    text: format!("\n{text}"),
                }));
            }
            (FrontMatterAction::MoveDown(i), Some(fm)) => {
                let (Some(row), Some(next)) = (fm.rows.get(i), fm.rows.get(i + 1)) else { return };
                let text = buffer[next.line].to_string();
                // at the end of the line above, so a caret at the row's start stays
                ops.push(Operation::Replace(Replace {
                    range: (row.line.start() - 1).into_range(),
                    text: format!("\n{text}"),
                }));
                ops.push(Operation::Replace(Replace {
                    range: (row.line.end(), next.line.end()),
                    text: "".into(),
                }));
            }
            (
                FrontMatterAction::MakeText(i)
                | FrontMatterAction::MakeList(i)
                | FrontMatterAction::MakeCheckbox(i),
                Some(fm),
            ) => {
                let Some(row) = fm.rows.get(i) else { return };
                let Kind::Property { sep, value, .. } = &row.kind else { return };
                let written: Vec<String> = match value {
                    Value::Text { text, quotes, .. } => {
                        let w = quotes.map_or(*text, |(o, c)| (o.start(), c.end()));
                        vec![buffer[w].to_string()]
                    }
                    Value::Empty { .. } => vec![],
                    Value::List { items, .. } => items
                        .iter()
                        .map(|it| buffer[it.written()].to_string())
                        .collect(),
                };
                let text = match action {
                    FrontMatterAction::MakeText(_) => format!(": {}", written.join(", ")),
                    FrontMatterAction::MakeCheckbox(_) => {
                        format!(": {}", written.first().is_some_and(|w| w == "true"))
                    }
                    _ if written.is_empty() => ":\n  - ".to_string(),
                    _ => format!(
                        ":{}",
                        written
                            .iter()
                            .map(|w| format!("\n  - {w}"))
                            .collect::<String>()
                    ),
                };
                ops.push(Operation::Replace(Replace {
                    range: (sep.start(), row.line.end()),
                    text: text.clone(),
                }));
                // the row's old end advances to the new value's end
                ops.push(Operation::Select(row.line.end().into_range()));
            }
            // handled where the renderer's state lives
            (FrontMatterAction::ToggleCollapsed, _) | (_, None) => {}
        }
    }
}
