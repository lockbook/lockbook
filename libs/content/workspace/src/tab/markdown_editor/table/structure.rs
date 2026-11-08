//! Structure edits: rows and columns inserted, deleted, and moved, a
//! column's alignment set, a table inserted. Each is one batch of
//! replacements on the table's lines after their container's prefix — one
//! undo unit — that rewrites only what it moves, pads a ragged row to the
//! table's width, and leaves the caret in the cell it was in.

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use super::edit::{cell_text, replace};
use super::{RowBounds, TableBounds, cell_content, is_cell_space};
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// A structure edit, made where the selection ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableAction {
    /// A new table of `columns` columns, with a header and `rows` rows.
    Insert {
        columns: usize,
        rows: usize,
    },
    InsertRow {
        below: bool,
    },
    DeleteRow,
    MoveRow {
        down: bool,
    },
    InsertColumn {
        right: bool,
    },
    DeleteColumn,
    MoveColumn {
        right: bool,
    },
    Align(TableAlignment),
}

/// A blank cell as the editor writes one: a space of padding either side.
pub(super) const BLANK: &str = "  ";

#[derive(Clone, Copy)]
pub(super) enum ColumnEdit {
    Insert(usize),
    Delete(usize),
    Move { from: usize, to: usize },
}

impl ColumnEdit {
    /// Applies the edit to one row's cells, each the text between its
    /// pipes, given a `blank` to insert.
    fn apply(self, cells: &mut Vec<String>, blank: &str) {
        match self {
            ColumnEdit::Insert(at) => cells.insert(at, blank.into()),
            ColumnEdit::Delete(at) => {
                cells.remove(at);
            }
            ColumnEdit::Move { from, to } => {
                let cell = cells.remove(from);
                cells.insert(to, cell);
            }
        }
    }

    /// Where the cell at `column` is after the edit, if it still is.
    fn moves(self, column: usize) -> Option<usize> {
        Some(match self {
            ColumnEdit::Insert(at) => column + (column >= at) as usize,
            ColumnEdit::Delete(at) if column == at => return None,
            ColumnEdit::Delete(at) => column - (column > at) as usize,
            ColumnEdit::Move { from, to } if column == from => to,
            ColumnEdit::Move { from, to } if from < column && column <= to => column - 1,
            ColumnEdit::Move { from, to } if to <= column && column < from => column + 1,
            ColumnEdit::Move { .. } => column,
        })
    }

    /// Whether the edit changes which cell is a row's first.
    fn moves_first(self) -> bool {
        match self {
            ColumnEdit::Insert(at) | ColumnEdit::Delete(at) => at == 0,
            ColumnEdit::Move { from, to } => from == 0 || to == 0,
        }
    }
}

/// A row of cells, each the text between its pipes, as source.
pub(super) fn row_text(cells: &[String], leading_pipe: bool, trailing_pipe: bool) -> String {
    let (leading, trailing) =
        (if leading_pipe { "|" } else { "" }, if trailing_pipe { "|" } else { "" });
    format!("{leading}{}{trailing}", cells.join("|"))
}

/// Whether a delimiter row is another block without a leading pipe: dashes
/// alone are a thematic break, and a dash then a space begin a list item.
pub(super) fn delimiter_wants_pipe(cells: &[String], trailing_pipe: bool) -> bool {
    let first = cells[0].trim_start_matches(is_cell_space);
    let alone = cells.len() == 1 && !trailing_pipe && !first.contains(':');
    alone || first.starts_with("- ") || first.starts_with("-\t")
}

/// A row as a structure edit works on it and writes it back: each cell's
/// text between its pipes, whether it has the outer pipes, and the caret as
/// a cell and a byte into it.
pub(super) struct RowText {
    pub cells: Vec<String>,
    pub leading: bool,
    pub trailing: bool,
    pub caret: Option<(usize, usize)>,
}

impl RowText {
    /// The row as source, and the caret as a byte into it. Each cell is
    /// made safe beside the pipes it now has — one from the row's edge may
    /// have leaned on there being none — and the row gets the outer pipe an
    /// edge cell needs.
    pub fn write(mut self) -> (String, Option<usize>) {
        let last = self.cells.len() - 1;
        for index in 0..=last {
            let (opened, closed) = (index > 0 || self.leading, index < last || self.trailing);
            let caret = self.caret.filter(|(column, _)| *column == index);
            let cell =
                cell_text(&self.cells[index], caret.map_or(0, |caret| caret.1), opened, closed);
            self.leading |= index == 0 && cell.opening_pipe;
            self.trailing |= index == last && cell.closing_pipe;
            self.caret = self
                .caret
                .map(|kept| caret.map_or(kept, |_| (index, cell.caret)));
            self.cells[index] = cell.text;
        }
        let caret = self.caret.map(|(column, into)| {
            let before: usize = self.cells[..column].iter().map(|cell| cell.len() + 1).sum();
            self.leading as usize + before + into
        });
        (row_text(&self.cells, self.leading, self.trailing), caret)
    }
}

impl MdRender {
    /// The part of `row`'s line its cells and their pipes take up: all of
    /// it but indentation, space after it, and a long row's surplus cells.
    pub(super) fn table_row_structure(&self, row: &RowBounds) -> (Grapheme, Grapheme) {
        let (first, last) = (row.cells[0].span, row.cells[row.cells.len() - 1].span);
        (first.start() - row.leading_pipe as usize, last.end() + row.trailing_pipe as usize)
    }

    /// `row` to edit and write back: the text between the pipes of each of
    /// its cells, with blank cells for those it lacks of `columns`, and the
    /// selection's end if it is in the row.
    pub(super) fn table_row_text(&self, row: &RowBounds, columns: usize) -> RowText {
        let mut cells: Vec<String> = row
            .cells
            .iter()
            .map(|cell| self.buffer[cell.span].to_string())
            .collect();
        cells.resize(columns.max(cells.len()), BLANK.into());
        let caret = self.buffer.current.selection.1;
        let caret = row.line.contains_inclusive(caret).then(|| {
            let column = row.cell_near(caret);
            let cell = row.cells[column].span;
            (column, self.buffer[(cell.start(), caret.max(cell.start()).min(cell.end()))].len())
        });
        RowText { cells, leading: row.leading_pipe, trailing: row.trailing_pipe, caret }
    }

    /// The delimiter row's structure, as a row's: where its cells and
    /// their pipes are, and whether it has the outer two.
    pub(super) fn table_delimiter_structure(
        &self, table: &TableBounds,
    ) -> ((Grapheme, Grapheme), bool, bool) {
        let delimiter = &table.delimiter;
        let (first, last) = (delimiter.cells[0], delimiter.cells[delimiter.cells.len() - 1]);
        let pipe_at = |offset: Grapheme| {
            delimiter.line.contains(offset, true, false)
                && &self.buffer[(offset, offset + 1)] == "|"
        };
        let leading = first.start() > delimiter.line.start() && pipe_at(first.start() - 1);
        let trailing = pipe_at(last.end());
        ((first.start() - leading as usize, last.end() + trailing as usize), leading, trailing)
    }

    /// Edits a column in every row and the delimiter row. A row is padded
    /// to the table's width first, so the table is left rectangular.
    pub(super) fn edit_table_columns(
        &self, table: &TableBounds, edit: ColumnEdit, operations: &mut Vec<Operation>,
    ) {
        for row in &table.rows {
            let mut text = self.table_row_text(row, table.columns());
            edit.apply(&mut text.cells, BLANK);
            // the caret keeps its place in its cell, wherever the cell goes
            text.caret = text
                .caret
                .and_then(|(column, into)| Some((edit.moves(column)?, into)));
            text.leading |= edit.moves_first();
            let (text, caret) = text.write();
            self.rewrite(self.table_row_structure(row), &text, caret, operations);
        }

        let (structure, leading, trailing) = self.table_delimiter_structure(table);
        let mut cells: Vec<String> = table
            .delimiter
            .cells
            .iter()
            .map(|cell| self.buffer[*cell].to_string())
            .collect();
        edit.apply(&mut cells, " --- ");
        let leading = leading || edit.moves_first() || delimiter_wants_pipe(&cells, trailing);
        self.rewrite(structure, &row_text(&cells, leading, trailing), None, operations);
    }

    /// The lines `first..=last`, by index, with a line break: the one after
    /// them, or at the document's end the one before.
    pub(super) fn whole_lines(&self, first: usize, last: usize) -> (Grapheme, Grapheme) {
        let lines = &self.bounds.source_lines;
        match lines.get(last + 1) {
            Some(next) => (lines[first].start(), next.start()),
            None => (lines[first.saturating_sub(1)].end(), lines[last].end()),
        }
    }

    /// Moves row `from` to where row `to` is. Body rows trade lines; the
    /// header stays where it is and trades its cells instead, padded to the
    /// table's width, since the table is as wide as its header.
    pub(super) fn move_table_row(
        &self, table: &TableBounds, from: usize, to: usize, operations: &mut Vec<Operation>,
    ) {
        let caret = self.buffer.current.selection.1;
        let (low, high) = (from.min(to), from.max(to));
        let with_header = low == 0;

        // each row between takes its neighbor's text; `to` takes `from`'s
        for row in low..=high {
            let source = match row {
                row if row == to => from,
                row if from < to => row + 1,
                row => row - 1,
            };
            let (row, source) = (&table.rows[row], &table.rows[source]);
            if with_header {
                let source = self.table_row_text(source, table.columns());
                let (text, caret) = RowText { leading: true, trailing: true, ..source }.write();
                self.rewrite(self.table_row_structure(row), &text, caret, operations);
            } else {
                let kept = source.line.contains_inclusive(caret);
                let kept = kept.then(|| self.buffer[(source.line.start(), caret)].len());
                self.rewrite(row.line, &self.buffer[source.line], kept, operations);
            }
        }
    }

    /// A delimiter cell that says `alignment`, as wide as `cell` was where
    /// that is wide enough. `opened`: whether a pipe precedes it.
    fn aligned_delimiter_cell(cell: &str, opened: bool, alignment: TableAlignment) -> String {
        let (start, end) = cell_content(cell, opened);
        let (left, right) = match alignment {
            TableAlignment::None => ("", ""),
            TableAlignment::Left => (":", ""),
            TableAlignment::Center => (":", ":"),
            TableAlignment::Right => ("", ":"),
        };
        let dashes = (end - start).saturating_sub(left.len() + right.len());
        let dashes = "-".repeat(dashes.max(1));
        format!("{}{left}{dashes}{right}{}", &cell[..start], &cell[end..])
    }
}

impl<'ast> MdEdit {
    /// Carries out a structure edit on the table the selection ends in, or
    /// for [`TableAction::Insert`], where it ends outside one.
    pub fn table_action(
        &self, root: &'ast AstNode<'ast>, action: TableAction, operations: &mut Vec<Operation>,
    ) {
        let r = &self.renderer;
        let Some((table, row, column)) = r.table_position() else {
            if let TableAction::Insert { columns, rows } = action {
                self.insert_table(root, columns.max(1), rows, operations);
            }
            return;
        };
        let (rows, columns) = (table.rows.len(), table.columns());
        let lines = &r.bounds.source_lines;
        match action {
            TableAction::Insert { .. } => {}
            TableAction::InsertRow { below } => {
                let blank_row = row_text(&vec![BLANK.to_string(); columns], true, true);
                let prefix = r.table_line_prefix(table);
                // after this body row, counting the delimiter row as the first
                let after = if below { row } else { row.saturating_sub(1) };
                let at = lines[table.first_line + 1 + after].end().into_range();
                if row == 0 && !below {
                    // a row above the header is a new header: the old one's
                    // cells move down a row, and the caret with them
                    let header = &table.rows[0];
                    let structure = r.table_row_structure(header);
                    let moved = r.table_row_text(header, columns);
                    let (moved, kept) = RowText { leading: true, trailing: true, ..moved }.write();
                    let kept = kept.unwrap_or(0);
                    r.rewrite(structure, &blank_row, None, operations);
                    operations.push(replace(at, &format!("\n{prefix}{}", &moved[..kept])));
                    operations.push(Operation::Select(at));
                    operations.push(replace(at, &moved[kept..]));
                } else {
                    operations.push(replace(at, &format!("\n{prefix}{blank_row}")));
                }
            }
            // the header is the table: it goes whole, as with its last column
            TableAction::DeleteRow if row == 0 => {
                let range = (table.rows[0].line.start(), lines[table.lines().1 - 1].end());
                operations.push(replace(range, ""));
            }
            TableAction::DeleteRow => {
                let line = table.line_of_row(row);
                let neighbor = &table.rows[if row + 1 < rows { row + 1 } else { row - 1 }];
                let cell = neighbor.cells.get(column).or(neighbor.cells.last());
                operations.push(replace(r.whole_lines(line, line), ""));
                operations
                    .extend(cell.map(|cell| Operation::Select(cell.content.start().into_range())));
            }
            TableAction::MoveRow { down } => {
                // the header does not move below a row; a row moved above it
                // trades cells with it
                let to = if down { row + 1 } else { row.wrapping_sub(1) };
                if to < rows && !(row == 0 && down) {
                    r.move_table_row(table, row, to, operations);
                }
            }
            TableAction::InsertColumn { right } => {
                r.edit_table_columns(
                    table,
                    ColumnEdit::Insert(column + right as usize),
                    operations,
                );
            }
            TableAction::DeleteColumn if columns == 1 => {
                // the last column: the table goes with it
                let range = (table.rows[0].line.start(), lines[table.lines().1 - 1].end());
                operations.push(replace(range, ""));
            }
            TableAction::DeleteColumn => {
                r.edit_table_columns(table, ColumnEdit::Delete(column), operations)
            }
            TableAction::MoveColumn { right } => {
                let to = if right { column + 1 } else { column.wrapping_sub(1) };
                if to < columns {
                    r.edit_table_columns(table, ColumnEdit::Move { from: column, to }, operations);
                }
            }
            TableAction::Align(alignment) => {
                let cell = table.delimiter.cells[column];
                let (_, leading, trailing) = r.table_delimiter_structure(table);
                let opened = column > 0 || leading;
                let mut aligned =
                    MdRender::aligned_delimiter_cell(&r.buffer[cell], opened, alignment);
                if columns == 1 && !leading && delimiter_wants_pipe(&[aligned.clone()], trailing) {
                    aligned.insert(0, '|');
                }
                r.rewrite(cell, &aligned, None, operations);
            }
        }
    }

    /// Return at the end of a paragraph's first line made only of cells,
    /// `| a | b |`, makes that line a table's header, as Typora does: the
    /// delimiter row and a blank row follow, with the caret in its first cell.
    pub fn newline_after_header_row(
        &self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let r = &self.renderer;
        let selection = r.buffer.current.selection;
        if r.readonly || r.kind != crate::tab::markdown_editor::Syntax::Markdown {
            return false;
        }
        let offset = selection.1;
        let line = r.line_at_offset(offset);
        if selection.0 != offset || offset != line.end() || r.bounds.table_at(offset).is_some() {
            return false;
        }
        let paragraph = root.descendants().find(|node| {
            matches!(node.data.borrow().value, NodeValue::Paragraph)
                && r.node_range(node).contains_inclusive(offset)
        });
        let Some(paragraph) = paragraph else { return false };
        if r.bounds.source_lines[r.node_first_line_idx(paragraph)] != line {
            return false;
        }
        let container = r.deepest_container_block_at_offset(root, offset);
        let text = r.buffer[r.line_content(container, line)].trim_end();
        let Some(inner) = text.strip_prefix('|').and_then(|t| t.strip_suffix('|')) else {
            return false;
        };
        if inner.is_empty() || inner.ends_with('\\') {
            return false;
        }
        let mut columns = 1;
        let mut escaped = false;
        for c in inner.chars() {
            columns += usize::from(c == '|' && !escaped);
            escaped = c == '\\' && !escaped;
        }

        let prefix = r.extension_prefix(container).unwrap_or_default();
        let blank_row = row_text(&vec![BLANK.to_string(); columns], true, true);
        let delimiter = row_text(&vec![" --- ".to_string(); columns], true, true);
        let before = format!("\n{prefix}{delimiter}\n{prefix}| ");
        let mut after = format!(" {}", &blank_row[3..]);
        // the paragraph's next line would become a row, so a blank line ends the table
        if r.node_last_line(paragraph) != line {
            after += &format!("\n{prefix}");
        }
        let at = line.end().into_range();
        operations.push(replace(at, &before));
        operations.push(Operation::Select(at));
        operations.push(replace(at, &after));
        true
    }

    /// Writes a header, a delimiter row, and one row of three columns on
    /// their own lines, with a blank line either side where there is text,
    /// and puts the caret in the first header cell.
    fn insert_table(
        &self, root: &'ast AstNode<'ast>, columns: usize, rows: usize,
        operations: &mut Vec<Operation>,
    ) {
        let r = &self.renderer;
        let offset = r.buffer.current.selection.end();
        let lines = &r.bounds.source_lines;
        let line_index = lines
            .iter()
            .position(|line| line.contains_inclusive(offset))
            .unwrap_or(0);
        let line = lines[line_index];
        let container = r.deepest_container_block_at_offset(root, offset);
        let prefix = r.extension_prefix(container).unwrap_or_default();
        let has_text = |index: usize| {
            lines
                .get(index)
                .is_some_and(|line| r.buffer[*line].trim() != prefix.trim())
        };

        // on this line if it is blank and so is the one above; else below it
        let own_line = !has_text(line_index);
        let mut before = String::new();
        if !own_line || line_index.checked_sub(1).is_some_and(has_text) {
            before += &format!("\n{prefix}");
        }
        if !own_line {
            before += &format!("\n{prefix}");
        }
        before += "| ";
        let blank_row = row_text(&vec![BLANK.to_string(); columns], true, true);
        let mut after = format!(" {}", &blank_row[3..]);
        after +=
            &format!("\n{prefix}{}", row_text(&vec![" --- ".to_string(); columns], true, true));
        for _ in 0..rows {
            after += &format!("\n{prefix}{blank_row}");
        }
        if has_text(line_index + 1) {
            after += &format!("\n{prefix}");
        }
        let at = line.end().into_range();
        operations.push(replace(at, &before));
        operations.push(Operation::Select(at));
        operations.push(replace(at, &after));
    }
}
