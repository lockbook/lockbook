//! Structure edits: rows and columns inserted, deleted, and moved, a
//! column's alignment set, a table inserted. Each is one batch of
//! replacements on the table's lines after their container's prefix — one
//! undo unit — that rewrites only what it moves, pads a ragged row to the
//! table's width, and leaves the caret in the cell it was in.

use comrak::nodes::{AstNode, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use super::edit::{cell_text, replace};
use super::{RowBounds, TableBounds, is_cell_space};
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// A structure edit, made where the selection ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableAction {
    /// A new table of three columns and one row.
    Insert,
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
const BLANK: &str = "  ";

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
                // a cell from the row's edge may lean on having no pipe there
                let cell = cell_text(&cells.remove(from), 0, true, true).text;
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
fn row_text(cells: &[String], leading_pipe: bool, trailing_pipe: bool) -> String {
    let (leading, trailing) =
        (if leading_pipe { "|" } else { "" }, if trailing_pipe { "|" } else { "" });
    format!("{leading}{}{trailing}", cells.join("|"))
}

fn blank(cell: &str) -> bool {
    cell.chars().all(is_cell_space)
}

impl MdRender {
    /// The part of `row`'s line its cells and their pipes take up: all of
    /// it but indentation, space after it, and a long row's surplus cells.
    fn table_row_structure(&self, row: &RowBounds) -> (Grapheme, Grapheme) {
        let (first, last) = (row.cells[0].span, row.cells[row.cells.len() - 1].span);
        (first.start() - row.leading_pipe as usize, last.end() + row.trailing_pipe as usize)
    }

    /// The text between the pipes of each of `row`'s cells, with blank
    /// cells for those it lacks of `columns`. A cell with nothing between
    /// its pipes — only a row's first can be — is written blank: anywhere
    /// else it would not be a cell.
    fn table_row_cells(&self, row: &RowBounds, columns: usize) -> Vec<String> {
        let mut cells: Vec<String> = row
            .cells
            .iter()
            .map(|cell| &self.buffer[cell.span])
            .map(|cell| if cell.is_empty() { BLANK } else { cell }.to_string())
            .collect();
        cells.resize(columns.max(cells.len()), BLANK.into());
        cells
    }

    /// The delimiter row's structure, as a row's: where its cells and
    /// their pipes are, and whether it has the outer two.
    fn table_delimiter_structure(&self, table: &TableBounds) -> ((Grapheme, Grapheme), bool, bool) {
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
        let caret = self.buffer.current.selection.1;
        for row in &table.rows {
            let mut cells = self.table_row_cells(row, table.columns());
            // the caret keeps its place in its cell, wherever the cell goes
            let caret = row.line.contains_inclusive(caret).then(|| {
                let column = row.cell_near(caret);
                let cell = row.cells[column].span;
                let into =
                    self.buffer[(cell.start(), caret.max(cell.start()).min(cell.end()))].len();
                (column, into)
            });
            edit.apply(&mut cells, BLANK);
            let leading = row.leading_pipe || edit.moves_first() || blank(&cells[0]);
            let trailing = row.trailing_pipe || blank(&cells[cells.len() - 1]);
            let caret = caret.and_then(|(column, into)| {
                let column = edit.moves(column)?;
                let before: usize = cells[..column].iter().map(|cell| cell.len() + 1).sum();
                Some(leading as usize + before + into.min(cells[column].len()))
            });
            let text = row_text(&cells, leading, trailing);
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
        let text = row_text(&cells, leading || edit.moves_first(), trailing);
        self.rewrite(structure, &text, None, operations);
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
        let part =
            |row: &RowBounds| if with_header { self.table_row_structure(row) } else { row.line };
        let text = |row: &RowBounds| {
            if with_header {
                row_text(&self.table_row_cells(row, table.columns()), true, true)
            } else {
                self.buffer[row.line].to_string()
            }
        };

        // each row between takes its neighbor's text; `to` takes `from`'s
        for row in low..=high {
            let source = match row {
                row if row == to => from,
                row if from < to => row + 1,
                row => row - 1,
            };
            let (row, source) = (&table.rows[row], &table.rows[source]);
            let caret = part(source).contains_inclusive(caret).then(|| {
                let kept = self.buffer[(part(source).start(), caret)].len();
                let gained_pipe = with_header && !source.leading_pipe;
                (kept + gained_pipe as usize).min(text(source).len())
            });
            self.rewrite(part(row), &text(source), caret, operations);
        }
    }

    /// A delimiter cell that says `alignment`, as wide as `cell` was where
    /// that is wide enough.
    fn aligned_delimiter_cell(cell: &str, alignment: TableAlignment) -> String {
        let padding = |text: &str| text.len() - text.trim_start_matches(is_cell_space).len();
        let (before, after) =
            (padding(cell), cell.len() - cell.trim_end_matches(is_cell_space).len());
        let (left, right) = match alignment {
            TableAlignment::None => ("", ""),
            TableAlignment::Left => (":", ""),
            TableAlignment::Center => (":", ":"),
            TableAlignment::Right => ("", ":"),
        };
        let width = cell.len().saturating_sub(before + after);
        let dashes = "-".repeat(width.saturating_sub(left.len() + right.len()).max(1));
        format!("{}{left}{dashes}{right}{}", &cell[..before], &cell[cell.len() - after..])
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
            if action == TableAction::Insert {
                self.insert_table(root, operations);
            }
            return;
        };
        let (rows, columns) = (table.rows.len(), table.columns());
        let lines = &r.bounds.source_lines;
        match action {
            TableAction::Insert => {}
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
                    let caret = r.buffer.current.selection.1;
                    let moved = row_text(&r.table_row_cells(header, columns), true, true);
                    let kept = r.buffer[(structure.start(), caret.max(structure.start()))].len()
                        + !header.leading_pipe as usize;
                    let kept = kept.min(moved.len());
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
                let aligned = MdRender::aligned_delimiter_cell(&r.buffer[cell], alignment);
                r.rewrite(cell, &aligned, None, operations);
            }
        }
    }

    /// Writes a header, a delimiter row, and one row of three columns on
    /// their own lines, with a blank line either side where there is text,
    /// and puts the caret in the first header cell.
    fn insert_table(&self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>) {
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
        let mut after = format!(" |  |  |\n{prefix}| --- | --- | --- |\n{prefix}|  |  |  |");
        if has_text(line_index + 1) {
            after += &format!("\n{prefix}");
        }
        let at = line.end().into_range();
        operations.push(replace(at, &before));
        operations.push(Operation::Select(at));
        operations.push(replace(at, &after));
    }
}
