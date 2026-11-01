//! Edits that reach a table. Each is a set of replacements on the rows'
//! lines that leaves every cell it does not name as it was: text typed or
//! deleted in a cell stays in the cell, a selection of whole cells clears
//! them, and a selection of whole rows removes their lines.

use comrak::nodes::TableAlignment;
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::{Operation, Replace};
use unicode_segmentation::UnicodeSegmentation as _;

use super::structure::{BLANK, row_text};
use super::{CellBounds, RowBounds, TableBounds, TableSelection, is_pipe_space};
use crate::tab::markdown_editor::View;
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::input::{Advance, Region};
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// Everything between a cell's pipes, made to parse as that one cell.
pub(super) struct CellText {
    pub text: String,
    /// Byte index into `text` of the caret.
    pub caret: usize,
    /// The row lacks this cell's opening pipe, and the cell needs it.
    pub opening_pipe: bool,
    /// The row lacks this cell's closing pipe, and the cell needs it.
    pub closing_pipe: bool,
}

/// Whether a line that starts with `text` is a row whatever follows on it:
/// a letter starts no other block, nor does a number that is not a list
/// item's.
fn starts_a_row(text: &str) -> bool {
    let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
    let mut after = rest.chars();
    let numbered = rest.len() < text.len()
        && matches!(after.next(), Some('.' | ')'))
        && after.next().is_none_or(char::is_whitespace);
    text.starts_with(char::is_alphanumeric) && !numbered
}

/// Makes `text` safe as a cell's whole span, keeping `caret` (a byte index
/// into it) in place. `opened` and `closed` tell whether the row has the
/// cell's pipes.
///
/// comrak's row scanner ends a cell at a pipe unless something before the
/// pipe consumes it: a backslash, or — with spoilers on — another pipe or an
/// apostrophe paired with it. So inside the cell a pipe is escaped unless it
/// is in an even run of pipes, which pair off, and a cell does not start or end on a
/// character that would pair with or escape its own pipe. A blank cell
/// keeps a space, or a row would read `||`; at a row's end without a pipe a
/// blank cell would not be a cell at all, and a first cell without one could
/// start another block.
pub(super) fn cell_text(text: &str, caret: usize, opened: bool, closed: bool) -> CellText {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = String::with_capacity(text.len() + 4);
    let mut new_caret = None;
    let mut at_caret = |index: usize, out: &String| {
        if index >= caret && new_caret.is_none() {
            new_caret = Some(out.len());
        }
    };

    let mut i = 0;
    while i < chars.len() {
        let (index, c) = chars[i];
        at_caret(index, &out);
        match c {
            '\\' => {
                out.push(c);
                i += 1;
                if let Some(&(index, escaped)) = chars.get(i) {
                    if escaped.is_ascii_punctuation() {
                        at_caret(index, &out);
                        out.push(escaped);
                        i += 1;
                    }
                }
            }
            '|' | '\'' => {
                let run = chars[i..]
                    .iter()
                    .take_while(|(_, c)| matches!(c, '|' | '\''))
                    .count();
                let pipes = chars[i..i + run].iter().all(|(_, c)| *c == '|');
                let pair = pipes && run.is_multiple_of(2);
                for &(index, c) in &chars[i..i + run] {
                    at_caret(index, &out);
                    if c == '|' && !pair {
                        out.push('\\');
                    }
                    out.push(c);
                }
                i += run;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    let mut caret = new_caret.unwrap_or(out.len());

    let blank = out.chars().all(is_pipe_space);
    if out.is_empty() {
        out.push_str("  ");
        caret = 1;
    }
    let first = out.chars().next();
    let last = out.chars().next_back();
    let opening_pipe = !opened && (blank || !starts_a_row(&out));
    let closing_pipe = !closed && blank;
    if (opened || opening_pipe) && matches!(first, Some('|' | '\'')) {
        out.insert(0, ' ');
        caret += 1;
    }
    if (closed || closing_pipe) && matches!(last, Some('|' | '\'' | '\\')) {
        out.push(' ');
    }
    CellText { text: out, caret, opening_pipe, closing_pipe }
}

/// Text typed or pasted into a cell, which is one line.
pub(super) fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// What an edit does to whole rows it takes.
#[derive(Clone, Copy, PartialEq)]
enum Rows {
    /// Their cells are emptied, as typing over them does.
    Emptied,
    /// Their lines are removed, as cutting them does.
    Removed,
    /// Emptied, and left selected, unless they are empty already; then
    /// removed. Delete, so that one press is never more than meant.
    EmptiedFirst,
}

/// A part of the document an edit takes, where the edit ends in a table.
enum Cut {
    /// Text taken as text is: outside the table, or all of it.
    Text((Grapheme, Grapheme)),
    /// Part of a cell's span. The cell stays.
    InCell { row: RowBounds, column: usize, range: (Grapheme, Grapheme) },
    /// A cell's content. The cell stays.
    Cell { row: RowBounds, column: usize },
    /// Source lines `.0..=.1`, by index, removed whole.
    Lines(usize, usize),
    /// Nothing; the selection is left over this range.
    Keep((Grapheme, Grapheme)),
}

impl MdRender {
    /// The replacement that makes `cell` read `text` between its pipes — the
    /// smallest that does, so that a keystroke stays a keystroke in the
    /// diff — with the caret left at byte `caret` of `text`. `edited` is
    /// the part of the cell the new text is in place of.
    fn rewrite_cell(
        &self, row: &RowBounds, column: usize, text: &str, caret: usize,
        edited: (Grapheme, Grapheme), operations: &mut Vec<Operation>,
    ) {
        let cell = row.cells[column];
        let opened = column > 0 || row.leading_pipe;
        let closed = column + 1 < row.cells.len() || row.trailing_pipe;
        let CellText { mut text, mut caret, opening_pipe, closing_pipe } =
            cell_text(text, caret, opened, closed);
        if opening_pipe {
            text.insert(0, '|');
            caret += 1;
        }
        if closing_pipe {
            text.push('|');
        }

        self.rewrite_around(cell.span, &text, Some(caret), Some(edited), operations);
    }

    /// The replacement that makes `range` read `text` — the smallest that
    /// does — with the caret, if any, left at byte `caret` of `text`.
    pub(super) fn rewrite(
        &self, range: (Grapheme, Grapheme), text: &str, caret: Option<usize>,
        operations: &mut Vec<Operation>,
    ) {
        self.rewrite_around(range, text, caret, None, operations);
    }

    /// [`Self::rewrite`], where the new text is known to be in place of
    /// `edited`: among same-sized replacements — a space typed beside a
    /// space — the one there is made, so that two keystrokes of one frame
    /// land in the order they were typed.
    fn rewrite_around(
        &self, range: (Grapheme, Grapheme), text: &str, caret: Option<usize>,
        edited: Option<(Grapheme, Grapheme)>, operations: &mut Vec<Operation>,
    ) {
        // trim what the old and new text share, a grapheme at a time
        let old: Vec<&str> = self.buffer[range].graphemes(true).collect();
        let new: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
        let shared = |pairs: &mut dyn Iterator<Item = (&str, &str)>| {
            pairs.take_while(|(old, new)| old == new).count()
        };
        // how much of the old text lies either side of what was edited
        let (before, after) = match edited {
            Some(edited) => (
                edited.start().0.saturating_sub(range.start().0),
                range.end().0.saturating_sub(edited.end().0),
            ),
            None => (old.len(), old.len()),
        };
        let prefix = shared(&mut old.iter().copied().zip(new.iter().map(|g| g.1)));
        let prefix = prefix.min(before.min(old.len()));
        let suffix = shared(
            &mut old[prefix..]
                .iter()
                .rev()
                .copied()
                .zip(new[prefix..].iter().rev().map(|g| g.1)),
        );
        let suffix = suffix.min(after.min(old.len() - prefix));
        let byte_at = |grapheme: usize| new.get(grapheme).map_or(text.len(), |g| g.0);
        let (changed_start, changed_end) = (byte_at(prefix), byte_at(new.len() - suffix));
        let changed = (range.start() + prefix, range.end() - suffix);

        let Some(caret) = caret else {
            if !changed.is_empty() || changed_start < changed_end {
                operations.push(replace(changed, &text[changed_start..changed_end]));
            }
            return;
        };
        if caret < changed_start {
            let before = text[..caret].graphemes(true).count();
            operations.push(Operation::Select((range.start() + before).into_range()));
            operations.push(replace(changed, &text[changed_start..changed_end]));
        } else if caret > changed_end {
            let after = text[caret..].graphemes(true).count();
            operations.push(replace(changed, &text[changed_start..changed_end]));
            operations.push(Operation::Select((range.end() - after).into_range()));
        } else if caret == changed_end {
            // typed text with the caret after it: one replacement, as a
            // keystroke is anywhere, which undo takes a run of together
            operations.push(replace(changed, &text[changed_start..changed_end]));
            operations.push(Operation::Select(changed.start().into_range()));
        } else {
            // the caret sits inside the replacement: write up to it, place
            // it, then write the rest behind it
            operations.push(replace(changed, &text[changed_start..caret]));
            operations.push(Operation::Select(changed.end().into_range()));
            operations.push(replace(changed.end().into_range(), &text[caret..changed_end]));
        }
    }

    /// `range` as an edit takes it: a selection snapped to cells and rows,
    /// except that an edit from the caret — a backspace, however a platform
    /// words it — stays in the caret's cell, and the caret itself may be in
    /// its cell's padding, where a typed space leaves it.
    fn table_edit_range(&self, range: (Grapheme, Grapheme)) -> (Grapheme, Grapheme) {
        let selection = self.buffer.current.selection;
        let caret_cell = self
            .bounds
            .table_at(selection.1)
            .filter(|_| selection.is_empty() && range.contains_inclusive(selection.1))
            .and_then(|table| {
                let (row, column) = table.cell_at(selection.1)?;
                Some(table.rows[row].cells[column])
            });
        let Some(cell) = caret_cell else {
            return self.snap_selection_into_table_cells(range);
        };
        let reach = self.table_cell_reach(&cell, selection.1);
        (range.start().max(reach.start()), range.end().min(reach.end()))
    }

    /// How far an edit from a caret in `cell` reaches: the content it is
    /// in, or from padding it is in, the padding too.
    fn table_cell_reach(&self, cell: &CellBounds, caret: Grapheme) -> (Grapheme, Grapheme) {
        (
            if caret < cell.content.start() { cell.span.start() } else { cell.content.start() },
            if caret > cell.content.end() { cell.span.end() } else { cell.content.end() },
        )
    }

    /// A blank line ends a table; text on the line after its last row would
    /// be another row. If deleting `range` would take the blank line from
    /// between a table and text below it, the end of that table's last
    /// cell, where the caret goes instead.
    fn table_joined_by_deleting(&self, range: (Grapheme, Grapheme)) -> Option<Grapheme> {
        let lines = &self.bounds.source_lines;
        let line_of = |offset| lines.find_containing(offset, true, true).0;
        let (first, last) = (line_of(range.start()), line_of(range.end()));
        let table = self
            .bounds
            .tables
            .iter()
            .find(|table| table.lines().1 == first)?;
        let joined_blank = self.buffer[(lines[first].start(), range.start())]
            .chars()
            .chain(self.buffer[(range.end(), lines[last].end())].chars())
            .all(char::is_whitespace);
        if first == last || joined_blank {
            return None;
        }
        table.contents().last().map(|content| content.end())
    }

    /// The parts of the document that an edit of `range` takes, or `None`
    /// if neither of its ends is in a table and it is ordinary. `taken` says
    /// what becomes of whole rows; a table taken whole is just text.
    fn table_cuts(&self, range: (Grapheme, Grapheme), taken: Rows) -> Option<Vec<Cut>> {
        let remove_rows = taken != Rows::Emptied;
        let (start, end) = (range.start(), range.end());
        let lines = &self.bounds.source_lines;
        let line_of = |offset| lines.find_containing(offset, true, true).0;
        let (start_table, end_table) = (self.bounds.table_at(start), self.bounds.table_at(end));
        // the delimiter row holds an end only while the table shows as source
        let on_delimiter_row = |table: Option<&TableBounds>, offset| {
            table.is_some_and(|table| line_of(offset) == table.first_line + 1)
        };
        if on_delimiter_row(start_table, start) || on_delimiter_row(end_table, end) {
            return None;
        }
        let cells = |table: &TableBounds, rows: (usize, usize), cuts: &mut Vec<Cut>| {
            for row in &table.rows[rows.0..=rows.1] {
                cuts.extend(
                    (0..row.cells.len()).map(|column| Cut::Cell { row: row.clone(), column }),
                );
            }
        };
        let lines_of_rows = |table: &TableBounds, rows: (usize, usize), cuts: &mut Vec<Cut>| {
            if rows.0 <= rows.1 {
                cuts.push(Cut::Lines(table.line_of_row(rows.0), table.line_of_row(rows.1)));
            }
        };

        let mut cuts = Vec::new();
        if start_table.is_some() && start_table == end_table {
            match self.table_selection(range)? {
                TableSelection::InCell { table, row, column } => {
                    cuts.push(Cut::InCell { row: table.rows[row].clone(), column, range });
                }
                TableSelection::Cells { table, row, columns } => {
                    let row = &table.rows[row];
                    cuts.extend(
                        (columns.0..=columns.1)
                            .map(|column| Cut::Cell { row: row.clone(), column }),
                    );
                }
                TableSelection::Rows { table, rows } => {
                    let last = table.rows.len() - 1;
                    // rows with something in them are emptied first, and
                    // stay selected: a second delete removes them
                    let empty = table.rows[rows.0..=rows.1]
                        .iter()
                        .all(|row| row.cells.iter().all(|cell| cell.content.is_empty()));
                    let emptied = taken == Rows::EmptiedFirst && !empty;
                    if rows == (0, last) && !emptied {
                        let end = lines[table.lines().1 - 1].end();
                        cuts.push(Cut::Text((table.rows[0].line.start(), end)));
                    } else if !remove_rows || emptied {
                        cells(table, rows, &mut cuts);
                        if emptied {
                            let kept =
                                (table.rows[rows.0].line.start(), table.rows[rows.1].line.end());
                            cuts.push(Cut::Keep(kept));
                        }
                    } else if rows.0 == 0 {
                        // without its header the rest would not be a table
                        cells(table, (0, 0), &mut cuts);
                        lines_of_rows(table, (1, rows.1), &mut cuts);
                    } else {
                        lines_of_rows(table, rows, &mut cuts);
                    }
                }
            }
            return Some(cuts);
        }

        let (mut text_start, mut text_end) = (start, end);
        if let Some(table) = start_table {
            let row = table.row_on_line(line_of(start));
            if row > 0 && !remove_rows {
                cells(table, (row, table.rows.len() - 1), &mut cuts);
                text_start = lines.get(table.lines().1)?.start();
            }
        }
        let text = cuts.len();
        if let Some(table) = end_table {
            let row = table.row_on_line(line_of(end));
            if row + 1 < table.rows.len() {
                text_end = lines[table.first_line.checked_sub(1)?].end();
                cells(table, (0, if remove_rows { 0 } else { row }), &mut cuts);
                if remove_rows {
                    lines_of_rows(table, (1, row), &mut cuts);
                }
            }
        }
        if text_start < text_end {
            cuts.insert(text, Cut::Text((text_start, text_end)));
        }
        (start_table.is_some() || end_table.is_some()).then_some(cuts)
    }

    /// Carries out `cuts`, with `text` put where the first of them was and,
    /// if `select`, the caret after it.
    fn apply_table_cuts(
        &self, cuts: Vec<Cut>, text: &str, select: bool, operations: &mut Vec<Operation>,
    ) {
        // rows kept selected take the caret's place
        let kept = cuts.iter().any(|cut| matches!(cut, Cut::Keep(_)));
        let select = select && !kept;
        for (i, cut) in cuts.into_iter().enumerate() {
            let text = if i == 0 { text } else { "" };
            let mut ops = Vec::new();
            match cut {
                Cut::Text(range) => {
                    ops.push(replace(range, text));
                    ops.push(Operation::Select(range.start().into_range()));
                }
                Cut::InCell { row, column, range } => {
                    let span = row.cells[column].span;
                    let before = &self.buffer[(span.start(), range.start())];
                    let after = &self.buffer[(range.end(), span.end())];
                    let typed = one_line(text);
                    let caret = before.len() + typed.len();
                    let text = format!("{before}{typed}{after}");
                    self.rewrite_cell(&row, column, &text, caret, range, &mut ops);
                }
                Cut::Cell { row, column } => {
                    let cell = row.cells[column];
                    let before = &self.buffer[(cell.span.start(), cell.content.start())];
                    let after = &self.buffer[(cell.content.end(), cell.span.end())];
                    let typed = one_line(text);
                    let caret = before.len() + typed.len();
                    let text = format!("{before}{typed}{after}");
                    self.rewrite_cell(&row, column, &text, caret, cell.content, &mut ops);
                }
                Cut::Keep(range) => {
                    operations.push(Operation::Select(range));
                    continue;
                }
                Cut::Lines(first, last) => {
                    let range = self.whole_lines(first, last);
                    ops.push(replace(range, ""));
                    // the caret goes to the row that takes their place, or
                    // to the one before
                    let lines = &self.bounds.source_lines;
                    let column = self.table_position().map_or(0, |(.., column)| column);
                    let cell = self
                        .bounds
                        .table_at(lines[first].start())
                        .and_then(|table| {
                            let before = table.row_on_line(first).checked_sub(1);
                            let row = table.rows.get(table.row_on_line(last) + 1);
                            let row = row.or(before.map(|before| &table.rows[before]))?;
                            row.cells.get(column).or(row.cells.last())
                        });
                    let caret = cell.map_or(range.start(), |cell| cell.content.start());
                    ops.push(Operation::Select(caret.into_range()));
                }
            }
            let caret = select && i == 0;
            operations.extend(
                ops.into_iter()
                    .filter(|op| caret || matches!(op, Operation::Replace(_))),
            );
        }
    }
}

pub(super) fn replace(range: (Grapheme, Grapheme), text: &str) -> Operation {
    Operation::Replace(Replace { range, text: text.into() })
}

impl MdEdit {
    /// Replaces `range` with `text` where that reaches a table: typing or
    /// pasting over a selection, or a platform's replacement of a range.
    /// Typed over cells and rows, text empties them and goes in the first.
    /// False if the edit is ordinary.
    pub fn replace_in_table(
        &self, range: (Grapheme, Grapheme), text: &str, select: bool,
        operations: &mut Vec<Operation>,
    ) -> bool {
        // source edits a table as text
        if self.renderer.view == View::Source {
            return false;
        }
        let r = &self.renderer;
        let range = r.table_edit_range(range);
        let Some(cuts) = r.table_cuts(range, Rows::Emptied) else { return false };
        r.apply_table_cuts(cuts, text, select, operations);
        true
    }

    /// Deletes `range` where that reaches a table: cells are emptied, and
    /// whole rows emptied and kept selected, or removed once they are
    /// empty — the header emptied instead if rows would remain without it.
    /// False if the edit is ordinary.
    pub fn delete_range_in_table(
        &self, range: (Grapheme, Grapheme), operations: &mut Vec<Operation>,
    ) -> bool {
        // source edits a table as text
        if self.renderer.view == View::Source {
            return false;
        }
        let r = &self.renderer;
        let range = r.table_edit_range(range);
        let Some(cuts) = r.table_cuts(range, Rows::EmptiedFirst) else { return false };
        r.apply_table_cuts(cuts, "", true, operations);
        true
    }

    /// [`Self::delete_range_in_table`] of a range that is the selection, or
    /// the caret's row: taken as it is.
    pub fn cut_range_in_table(
        &self, range: (Grapheme, Grapheme), operations: &mut Vec<Operation>,
    ) -> bool {
        // source edits a table as text
        if self.renderer.view == View::Source {
            return false;
        }
        let r = &self.renderer;
        let Some(cuts) = r.table_cuts(range, Rows::Removed) else { return false };
        r.apply_table_cuts(cuts, "", true, operations);
        true
    }

    /// Backspace and its kin from a caret, where that meets a table. In a
    /// cell it deletes within the cell and at the cell's edge does nothing:
    /// it never joins cells. Against a table from outside, it steps the
    /// caret into the table instead. False if the edit is ordinary.
    pub fn delete_from_caret_at_table(
        &mut self, region: Region, operations: &mut Vec<Operation>,
    ) -> bool {
        // source edits a table as text
        if self.renderer.view == View::Source {
            return false;
        }
        let Region::SelectionOrAdvance { advance, backwards } = region else { return false };
        let selection = self.renderer.buffer.current.selection;
        if !selection.is_empty() {
            return false;
        }
        let caret = selection.1;
        let cell = self.renderer.bounds.table_at(caret).map(|table| {
            let (row, column) = table.cell_at(caret)?;
            Some((table.rows[row].clone(), column))
        });
        match cell {
            // along the delimiter row, shown as source
            Some(None) => false,
            Some(Some((row, column))) => {
                let reach = self.renderer.table_cell_reach(&row.cells[column], caret);
                let target = self.advance_in_table_cell(caret, advance, backwards);
                let target = target.max(reach.start()).min(reach.end());
                let range = (caret.min(target), caret.max(target));
                if !range.is_empty() {
                    let cuts = vec![Cut::InCell { row, column, range }];
                    self.renderer.apply_table_cuts(cuts, "", true, operations);
                }
                true
            }
            None => {
                let target = self.advance(caret, advance, backwards);
                let r = &self.renderer;
                let into_table = if r.bounds.table_at(target).is_some() {
                    Some(target)
                } else {
                    r.table_joined_by_deleting((caret.min(target), caret.max(target)))
                };
                if let Some(into_table) = into_table {
                    operations.push(Operation::Select(into_table.into_range()));
                }
                into_table.is_some()
            }
        }
    }

    /// An advance from a caret in a cell's padding, which navigation does
    /// not move through, taken a character at a time as text.
    fn advance_in_table_cell(
        &mut self, caret: Grapheme, advance: Advance, backwards: bool,
    ) -> Grapheme {
        let table = self.renderer.bounds.table_at(caret);
        let in_content = table.is_some_and(|table| {
            table
                .contents()
                .any(|content| content.contains_inclusive(caret))
        });
        if in_content {
            self.advance(caret, advance, backwards)
        } else if backwards {
            caret - 1.min(caret.0)
        } else {
            caret + 1
        }
    }
}

// ─── keys ────────────────────────────────────────────────────────────

impl MdRender {
    /// The table, row, and column the selection's end is in. `None` off a
    /// table, and on a delimiter row, which shows as source.
    pub fn table_position(&self) -> Option<(&TableBounds, usize, usize)> {
        self.table_position_at(self.buffer.current.selection.1)
    }

    /// The table, row, and column `offset` is in, as [`Self::table_position`]
    /// has the selection's end.
    pub fn table_position_at(&self, offset: Grapheme) -> Option<(&TableBounds, usize, usize)> {
        let table = self.bounds.table_at(offset)?;
        let line = self
            .bounds
            .source_lines
            .find_containing(offset, true, true)
            .0;
        if line == table.first_line + 1 {
            return None;
        }
        let row = table.row_on_line(line);
        Some((table, row, table.rows[row].cell_near(offset)))
    }

    /// Moves to a cell and selects its content, writing the cell first if
    /// its row is short of it.
    fn enter_table_cell(
        &self, table: &TableBounds, row: usize, column: usize, operations: &mut Vec<Operation>,
    ) {
        let row = &table.rows[row];
        match row.cells.get(column) {
            Some(cell) => operations.push(Operation::Select(cell.content)),
            None => self.write_table_cells(row, column, operations),
        }
    }

    /// What a new line of `table` starts with: its container's prefix, as
    /// the delimiter row's line has it.
    pub(super) fn table_line_prefix(&self, table: &TableBounds) -> &str {
        let line = self.bounds.source_lines[table.first_line + 1];
        &self.buffer[(line.start(), table.delimiter.line.start())]
    }

    /// Adds a blank row at the table's end, with the caret in `column`.
    fn append_table_row(
        &self, table: &TableBounds, column: usize, operations: &mut Vec<Operation>,
    ) {
        let prefix = self.table_line_prefix(table);
        let at = self.bounds.source_lines[table.lines().1 - 1]
            .end()
            .into_range();
        let before = format!("\n{prefix}|{} ", "  |".repeat(column));
        let after = format!(" |{}", "  |".repeat(table.columns() - column - 1));
        operations.push(replace(at, &before));
        operations.push(Operation::Select(at));
        operations.push(replace(at, &after));
    }

    /// Takes a blank last row away and puts the caret below the table,
    /// past a blank line: text on the line after a table would be a row.
    fn leave_table(&self, table: &TableBounds, row: &RowBounds, operations: &mut Vec<Operation>) {
        let lines = &self.bounds.source_lines;
        let prefix = self.table_line_prefix(table);
        // a line is blank if it holds nothing but the container's prefix
        let below = lines
            .get(table.lines().1)
            .filter(|below| self.buffer[**below].trim() == prefix.trim());
        match below {
            Some(below) => {
                operations.push(replace(row.line, ""));
                operations.push(Operation::Select(below.end().into_range()));
            }
            None => {
                operations.push(replace(row.line, &format!("\n{prefix}")));
                operations.push(Operation::Select(row.line.start().into_range()));
            }
        }
    }

    /// What cut and copy take when they take whole rows, and its text: the
    /// rows' lines without their container's prefix, so that with the
    /// header among them they paste as a table. With no selection, the
    /// caret's row. `None` if the selection isn't whole rows of one table.
    pub fn table_rows_clipboard(&self) -> Option<((Grapheme, Grapheme), String)> {
        // source edits a table as text
        if self.view == View::Source {
            return None;
        }
        let selection = self.buffer.current.selection;
        let (table, rows) = match self.table_selection(selection) {
            Some(TableSelection::Rows { table, rows }) => (table, rows),
            _ if selection.is_empty() => {
                let (table, row, _) = self.table_position()?;
                (table, (row, row))
            }
            _ => return None,
        };
        let mut lines = Vec::new();
        for (i, row) in table.rows.iter().enumerate().take(rows.1 + 1).skip(rows.0) {
            lines.push(&self.buffer[row.line]);
            // the header takes the delimiter row with it when it goes
            // with the rows under it, or is all the table has
            if i == 0 && (rows.1 > 0 || table.rows.len() == 1) {
                lines.push(&self.buffer[table.delimiter.line]);
            }
        }
        let range = (table.rows[rows.0].line.start(), table.rows[rows.1].line.end());
        Some((range, lines.join("\n")))
    }

    /// The selected cells or rows, for the clipboard's rendered flavors: as a
    /// table of their own with its first row the header, which renders as
    /// one, and as each row's cells' content ranges — empty for a cell the
    /// row lacks. `None` unless the selection takes whole cells or rows.
    pub fn table_selection_grid(&self) -> Option<(String, TableGrid)> {
        // source edits a table as text
        if self.view == View::Source {
            return None;
        }
        let (table, rows, columns) = match self.table_selection(self.buffer.current.selection)? {
            TableSelection::Cells { table, row, columns } => (table, (row, row), columns),
            TableSelection::Rows { table, rows } => (table, rows, (0, table.columns() - 1)),
            TableSelection::InCell { .. } => return None,
        };
        let width = columns.1 - columns.0 + 1;
        let (mut lines, mut grid) = (Vec::new(), Vec::new());
        for row in &table.rows[rows.0..=rows.1] {
            let cells: Vec<_> = row.cells.iter().skip(columns.0).take(width).collect();
            let mut texts: Vec<String> = cells
                .iter()
                .map(|cell| self.buffer[cell.span].to_string())
                .collect();
            texts.resize(width, BLANK.to_string());
            lines.push(row_text(&texts, true, true));
            if lines.len() == 1 {
                // as the editor aligns them: an unaligned column is left, where
                // HTML would center a header cell
                let delimiter = table.delimiter.alignments[columns.0..=columns.1].iter();
                let delimiter: Vec<String> = delimiter
                    .map(|alignment| match alignment {
                        TableAlignment::Center => " :-: ",
                        TableAlignment::Right => " --: ",
                        _ => " :-- ",
                    })
                    .map(str::to_string)
                    .collect();
                lines.push(row_text(&delimiter, true, true));
            }
            let end = row.line.end();
            let mut contents: Vec<_> = cells.iter().map(|cell| cell.content).collect();
            contents.resize(width, (end, end));
            grid.push(contents);
        }
        Some((lines.join("\n"), grid))
    }
}

/// Each row's cells' content ranges.
pub type TableGrid = Vec<Vec<(Grapheme, Grapheme)>>;

impl MdEdit {
    /// Tab in a table moves to the next cell, or with shift the previous,
    /// wrapping across rows, and selects its content. From the last cell it
    /// adds a row. False off a table.
    pub fn indent_in_table(&self, deindent: bool, operations: &mut Vec<Operation>) -> bool {
        let r = &self.renderer;
        let Some((table, row, column)) = r.table_position() else { return false };
        let last_column = table.columns() - 1;
        if deindent {
            let (row, column) = match (row, column) {
                (row, 0) => (row.saturating_sub(1), if row == 0 { 0 } else { last_column }),
                (row, column) => (row, column - 1),
            };
            r.enter_table_cell(table, row, column, operations);
        } else if column < last_column {
            r.enter_table_cell(table, row, column + 1, operations);
        } else if row + 1 < table.rows.len() {
            r.enter_table_cell(table, row + 1, 0, operations);
        } else {
            r.append_table_row(table, 0, operations);
        }
        true
    }

    /// Return in a table moves to the cell below, or with shift the cell
    /// above, and selects its content. From the last row it adds a row —
    /// unless that row is blank, when it takes the row away and leaves the
    /// table, as Return on an empty list item ends a list. From the
    /// delimiter row of a table typed by hand it starts the first row.
    /// False off a table.
    pub fn newline_in_table(&self, shift: bool, operations: &mut Vec<Operation>) -> bool {
        let r = &self.renderer;
        let Some((table, row, column)) = r.table_position() else {
            let offset = r.buffer.current.selection.1;
            let Some(table) = r.bounds.table_at(offset).filter(|_| !shift) else { return false };
            if table.rows.len() > 1 {
                r.enter_table_cell(table, 1, 0, operations);
            } else {
                r.append_table_row(table, 0, operations);
            }
            return true;
        };
        let blank = |row: &RowBounds| row.cells.iter().all(|cell| cell.content.is_empty());
        if shift {
            r.enter_table_cell(table, row.saturating_sub(1), column, operations);
        } else if row + 1 < table.rows.len() {
            r.enter_table_cell(table, row + 1, column, operations);
        } else if row > 0 && blank(&table.rows[row]) {
            r.leave_table(table, &table.rows[row], operations);
        } else {
            r.append_table_row(table, column, operations);
        }
        true
    }
}
