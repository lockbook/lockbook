//! Pasting a grid of cells. Tab-separated text of two or more lines pastes
//! as a table, and so does comma-separated text when a table is asked for.
//! Pasted into a table, a grid fills cells rightward and downward from the
//! cell the selection starts in, and the table grows to hold it.

use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::RangeExt as _;
use lb_rs::model::text::operation_types::Operation;

use super::cell_content;
use super::edit::{cell_text, one_line};
use super::structure::{BLANK, RowText, delimiter_wants_pipe, row_text};
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// Rows of cells, as the text of each, read from a paste.
pub type Grid = Vec<Vec<String>>;

/// Reads rows of fields separated by `delimiter`. A field in double quotes
/// holds delimiters, line breaks, and — doubled — quotes as text, the way
/// spreadsheets write them.
fn delimited(text: &str, delimiter: char) -> Grid {
    let (mut rows, mut row, mut field) = (Vec::new(), Vec::new(), String::new());
    // whether in quotes, and whether at the start of a field, where they open
    let (mut quoted, mut fresh) = (false, true);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' if quoted => quoted = false,
            '"' if fresh => quoted = true,
            c if quoted => field.push(c),
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
                fresh = true;
                continue;
            }
            '\r' if chars.peek() == Some(&'\n') => continue,
            '\n' | '\r' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                fresh = true;
                continue;
            }
            c => field.push(c),
        }
        fresh = false;
    }
    // a line break ends the last row; it does not start another
    if !fresh || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// What a cell holds between its pipes once it is filled with `value`:
/// the padding it had around it, or a space either side if it was blank.
fn filled_cell(old: &str, value: &str) -> String {
    let value = one_line(value);
    let (start, end) = cell_content(old, true);
    let (before, after) = if start < end { (&old[..start], &old[end..]) } else { (" ", " ") };
    cell_text(&format!("{before}{}{after}", value.trim()), 0, true, true).text
}

/// Where a cell's content ends, as a byte index into the text between its
/// pipes: for a blank cell, one space in.
fn content_end(cell: &str) -> usize {
    cell_content(cell, true).1
}

impl MdRender {
    /// The grid a paste of `text` holds, if it is to paste as one. Tabs
    /// separate cells; with `as_table`, a table having been asked for,
    /// commas do where there are no tabs. A plain paste outside a table is a
    /// table only if it could hardly be anything else: two or more lines
    /// of as many cells each.
    pub fn pasted_grid<'ast>(
        &self, root: &'ast AstNode<'ast>, text: &str, as_table: bool,
    ) -> Option<Grid> {
        let selection = self.buffer.current.selection;
        let in_table = self.table_position_at(selection.start()).is_some();
        // a selection that only reaches into a table is replaced as text
        let at_table = [selection.0, selection.1]
            .iter()
            .any(|end| self.bounds.table_at(*end).is_some());
        // code and markup hold their text as it is
        let literal = matches!(
            self.leaf_block_at_offset(root, selection.start())
                .data
                .borrow()
                .value,
            NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_)
        );
        // source pastes text as it is unless asked for a table
        let as_text = !self.is_markdown() || self.plaintext && !as_table;
        if as_text || self.readonly || literal || (at_table && !in_table) {
            return None;
        }

        let tabs = text.contains('\t');
        let grid = delimited(text, if tabs { '\t' } else { ',' });
        let width = grid.iter().map(Vec::len).max()?;
        // lines that each begin with a tab are indented, not cells
        if tabs && !as_table && grid.iter().all(|row| row[0].is_empty()) {
            return None;
        }
        let pastes = if as_table {
            // one cell pasted into a table is text
            !in_table || grid.len() > 1 || width > 1
        } else {
            tabs && (in_table || (grid.len() > 1 && grid.iter().all(|row| row.len() == width)))
        };
        pastes.then_some(grid)
    }

    /// Fills the table's cells with `grid`, rightward and downward from
    /// the cell the selection starts in. The table grows to hold it:
    /// columns at its right, rows at its end. The caret goes to the end of
    /// the last cell filled.
    fn fill_table(&self, grid: &Grid, operations: &mut Vec<Operation>) {
        let selection = self.buffer.current.selection;
        let Some((table, top, left)) = self.table_position_at(selection.start()) else { return };
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);
        let columns = table.columns().max(left + width);
        let grows = columns > table.columns();
        let lines = &self.bounds.source_lines;

        // the table's text from its header on, rebuilt a line at a time
        let (mut text, mut caret) = (String::new(), 0);
        // a row with the cells `grid` fills, as source
        let mut fill = |text: &mut String, index: usize, mut row: RowText| {
            row.caret = None;
            if let Some(values) = index.checked_sub(top).and_then(|index| grid.get(index)) {
                for (cell, value) in row.cells[left..].iter_mut().zip(values) {
                    *cell = filled_cell(cell, value);
                }
                row.leading |= left == 0;
                if index + 1 == top + grid.len() {
                    let last = left + values.len().saturating_sub(1);
                    row.caret = Some((last, content_end(&row.cells[last])));
                }
            }
            let (row, last_cell) = row.write();
            caret = last_cell.map_or(caret, |last_cell| text.len() + last_cell);
            *text += &row;
        };

        for (index, row) in table.rows.iter().enumerate() {
            let line = lines[table.line_of_row(index)];
            if index > 0 {
                text += "\n";
                text += &self.buffer[(line.start(), row.line.start())];
            }
            let filled = index >= top && index < top + grid.len();
            if !grows && !filled {
                text += &self.buffer[row.line];
            } else {
                let structure = self.table_row_structure(row);
                text += &self.buffer[(row.line.start(), structure.start())];
                fill(&mut text, index, self.table_row_text(row, columns));
                text += &self.buffer[(structure.end(), row.line.end())];
            }

            if index == 0 {
                let delimiter = table.delimiter.line;
                text += "\n";
                text += &self.buffer[(lines[table.first_line + 1].start(), delimiter.start())];
                if grows {
                    let (structure, leading, trailing) = self.table_delimiter_structure(table);
                    let cells = table.delimiter.cells.iter();
                    let mut cells: Vec<String> =
                        cells.map(|cell| self.buffer[*cell].to_string()).collect();
                    cells.resize(columns, " --- ".into());
                    let leading = leading || delimiter_wants_pipe(&cells, trailing);
                    text += &self.buffer[(delimiter.start(), structure.start())];
                    text += &row_text(&cells, leading, trailing);
                    text += &self.buffer[(structure.end(), delimiter.end())];
                } else {
                    text += &self.buffer[delimiter];
                }
            }
        }
        for index in table.rows.len()..top + grid.len() {
            text += "\n";
            text += self.table_line_prefix(table);
            let cells = vec![BLANK.to_string(); columns];
            fill(&mut text, index, RowText { cells, leading: true, trailing: true, caret: None });
        }

        let range = (table.rows[0].line.start(), lines[table.lines().1 - 1].end());
        self.rewrite(range, &text, Some(caret), operations);
    }

    /// Writes `grid` as a table in place of the selection — on its own
    /// lines, with a blank line either side where there is text — its first
    /// row the header. The caret goes to the end of its last cell.
    fn paste_table<'ast>(
        &self, root: &'ast AstNode<'ast>, grid: &Grid, operations: &mut Vec<Operation>,
    ) {
        let selection = self.buffer.current.selection;
        let lines = &self.bounds.source_lines;
        let (first, last) = lines.find_intersecting(selection, true);
        let last = last.saturating_sub(1).max(first);
        let container = self.deepest_container_block_at_offset(root, selection.end());
        let prefix = self.extension_prefix(container).unwrap_or_default();
        // a line with nothing but its container's prefix is blank
        let has_text = |text: &str| text.trim() != prefix.trim() && !text.trim().is_empty();
        let line_has_text = |index: Option<usize>| {
            index
                .and_then(|index| lines.get(index))
                .is_some_and(|line| has_text(&self.buffer[*line]))
        };
        let new_line = format!("\n{prefix}");

        let mut text = String::new();
        if has_text(&self.buffer[(lines[first].start(), selection.start())]) {
            text += &new_line;
            text += &new_line;
        } else if line_has_text(first.checked_sub(1)) {
            text += &new_line;
        }
        let width = grid.iter().map(Vec::len).max().unwrap_or(1);
        let mut caret = 0;
        for (index, row) in grid.iter().enumerate() {
            let mut cells: Vec<String> =
                row.iter().map(|value| filled_cell(BLANK, value)).collect();
            if index + 1 == grid.len() {
                let before: usize = cells[..cells.len() - 1]
                    .iter()
                    .map(|cell| cell.len() + 1)
                    .sum();
                caret = text.len() + 1 + before + content_end(&cells[cells.len() - 1]);
            }
            cells.resize(width, BLANK.into());
            text += &row_text(&cells, true, true);
            if index == 0 {
                text += &new_line;
                text += &row_text(&vec![" --- ".to_string(); width], true, true);
            }
            if index + 1 < grid.len() {
                text += &new_line;
            }
        }
        if has_text(&self.buffer[(selection.end(), lines[last].end())]) {
            text += &new_line;
            text += &new_line;
        } else if line_has_text(Some(last + 1)) {
            text += &new_line;
        }

        self.rewrite(selection, &text, Some(caret), operations);
    }
}

impl<'ast> MdEdit {
    /// Pastes a grid: into the table the selection starts in, or as a new
    /// table in the selection's place.
    pub fn paste_grid(
        &self, root: &'ast AstNode<'ast>, grid: &Grid, operations: &mut Vec<Operation>,
    ) {
        let r = &self.renderer;
        if r.table_position_at(r.buffer.current.selection.start())
            .is_some()
        {
            r.fill_table(grid, operations);
        } else {
            r.paste_table(root, grid, operations);
        }
    }
}
