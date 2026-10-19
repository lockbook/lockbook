//! Tables. A table is a GFM pipe table: a header row, a delimiter row, and
//! body rows, one source line each. [`TableBounds`] records where each
//! row's cells and pipes are; everything the editor does to a table is
//! arithmetic on it.
//!
//! A table is a grid: the caret lives in cell content and steps from cell
//! to cell, never onto a pipe, padding, the delimiter row, or the
//! container prefix of a table line. The exception is a caret an edit left
//! on the delimiter row — someone is typing a table by hand — which shows
//! the table as source ([`MdRender::table_revealed`]) for as long as it
//! stays.

use comrak::nodes::{AstNode, NodeValue, Sourcepos, TableAlignment};
use egui::Rect;
use lb_rs::model::text::buffer;
use lb_rs::model::text::offset_types::{Byte, Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::{Operation, Replace};

mod edit;

use super::bounds::{Bounds, RangesExt as _};
use super::input::{Advance, Increment, Location, Region};
use super::widget::utils::NodeValueExt as _;
use super::{MdEdit, MdRender};

/// One table's rows, cells, and pipes in the source.
#[derive(Clone, Debug, PartialEq)]
pub struct TableBounds {
    /// Index of the header's source line. The delimiter row's is the next,
    /// then one for each body row.
    pub first_line: usize,
    /// Header first. The delimiter row is not among them.
    pub rows: Vec<RowBounds>,
    pub delimiter: DelimiterBounds,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowBounds {
    /// The row's source line after its container's prefix.
    pub line: (Grapheme, Grapheme),
    /// The cells written in the row: at least one, at most the table's
    /// column count. A pipe separates each from the next.
    pub cells: Vec<CellBounds>,
    /// Whether a pipe precedes the first cell.
    pub leading_pipe: bool,
    /// Whether a pipe follows the last cell.
    pub trailing_pipe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellBounds {
    /// Everything between the cell's pipes.
    pub span: (Grapheme, Grapheme),
    /// `span` less its padding. Empty, one space in, for a blank cell.
    pub content: (Grapheme, Grapheme),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DelimiterBounds {
    /// The delimiter row's source line after its container's prefix.
    pub line: (Grapheme, Grapheme),
    /// Everything between each column's pipes.
    pub cells: Vec<(Grapheme, Grapheme)>,
    pub alignments: Vec<TableAlignment>,
}

/// A table cell painted this frame, for resolving a pointer to its cell.
#[derive(Clone, Copy, Debug)]
pub struct TableCellBox {
    pub rect: Rect,
    /// Start of the row's line, which identifies the row in
    /// [`Bounds::tables`].
    pub row: Grapheme,
    pub column: usize,
}

impl TableBounds {
    pub fn columns(&self) -> usize {
        self.delimiter.cells.len()
    }

    /// The table's source lines, as indexes with the end excluded.
    pub fn lines(&self) -> (usize, usize) {
        (self.first_line, self.first_line + self.rows.len() + 1)
    }

    /// The content of every written cell, in document order.
    pub fn contents(&self) -> impl DoubleEndedIterator<Item = (Grapheme, Grapheme)> + '_ {
        self.rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.content))
    }

    /// The row and column of the written cell whose span holds `offset`.
    pub fn cell_at(&self, offset: Grapheme) -> Option<(usize, usize)> {
        self.rows.iter().enumerate().find_map(|(r, row)| {
            let c = row
                .cells
                .iter()
                .position(|cell| cell.span.contains_inclusive(offset))?;
            Some((r, c))
        })
    }

    /// The row whose line holds `offset`.
    pub fn row_at(&self, offset: Grapheme) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.line.contains_inclusive(offset))
    }

    /// The row on source line `line`, one of the table's. The delimiter
    /// row's line counts as the header's.
    fn row_on_line(&self, line: usize) -> usize {
        line.saturating_sub(self.first_line + 1)
            .min(self.rows.len().saturating_sub(1))
    }

    /// Index of the source line row `row` is on.
    pub fn line_of_row(&self, row: usize) -> usize {
        self.first_line + if row == 0 { 0 } else { row + 1 }
    }
}

/// What a selection takes of a table that both its ends are in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableSelection<'t> {
    /// Part of one cell's content: an ordinary selection.
    InCell { table: &'t TableBounds, row: usize, column: usize },
    /// Cells `columns.0..=columns.1` of a row, whole.
    Cells { table: &'t TableBounds, row: usize, columns: (usize, usize) },
    /// Rows `rows.0..=rows.1`, whole.
    Rows { table: &'t TableBounds, rows: (usize, usize) },
}

impl RowBounds {
    /// The cell `offset` is in, or else the one before it in the row.
    fn cell_near(&self, offset: Grapheme) -> usize {
        self.cells
            .iter()
            .rposition(|cell| cell.span.start() <= offset)
            .unwrap_or(0)
    }

    /// The edge of a cell's content nearest `offset`.
    fn nearest_content_edge(&self, offset: Grapheme) -> Option<Grapheme> {
        self.cells
            .iter()
            .flat_map(|cell| [cell.content.start(), cell.content.end()])
            .min_by_key(|edge| edge.0.abs_diff(offset.0))
    }
}

impl Bounds {
    /// The table with a source line that holds `offset`.
    pub fn table_at(&self, offset: Grapheme) -> Option<&TableBounds> {
        let line = self.source_lines.find_containing(offset, true, true).0;
        self.tables.iter().find(|table| {
            let (start, end) = table.lines();
            (start..end).contains(&line)
        })
    }
}

/// The space characters the parser's row scanner pads cells with.
fn is_cell_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0b' | '\x0c')
}

// ─── bounds ──────────────────────────────────────────────────────────

impl<'ast> MdRender {
    /// Recompute [`Bounds::tables`]. Depends on text only.
    pub fn calc_table_bounds(&mut self, root: &'ast AstNode<'ast>) {
        let mut tables = Vec::new();
        for node in root.descendants().filter(|_| !self.plaintext) {
            if let NodeValue::Table(table) = &node.data.borrow().value {
                tables.extend(self.table_bounds(node, table.alignments.clone()));
            }
        }
        self.bounds.tables = tables;
    }

    fn table_bounds(
        &self, table: &'ast AstNode<'ast>, alignments: Vec<TableAlignment>,
    ) -> Option<TableBounds> {
        // the delimiter row has no node; it is the line after the header's
        let first_line = self.node_first_line_idx(table);
        let delimiter_line = self.bounds.source_lines.get(first_line + 1)?;
        let line = self.node_line(table, *delimiter_line);
        Some(TableBounds {
            first_line,
            rows: table.children().map(|row| self.row_bounds(row)).collect(),
            delimiter: DelimiterBounds { line, cells: self.delimiter_cells(line), alignments },
        })
    }

    fn row_bounds(&self, row: &'ast AstNode<'ast>) -> RowBounds {
        let line = self.node_line(row, self.node_first_line(row));
        let cells: Vec<CellBounds> = row
            .children()
            .take_while(|cell| !self.table_cell_autocompleted(cell))
            .map(|cell| {
                let span = self.node_range(cell);
                CellBounds { span, content: self.table_cell_content(span) }
            })
            .collect();
        let pipe_at = |offset: Grapheme| {
            line.contains(offset, true, false) && &self.buffer[(offset, offset + 1)] == "|"
        };
        let leading_pipe = cells
            .first()
            .is_some_and(|cell| cell.span.start() > line.start() && pipe_at(cell.span.start() - 1));
        let trailing_pipe = cells.last().is_some_and(|cell| pipe_at(cell.span.end()));
        RowBounds { line, cells, leading_pipe, trailing_pipe }
    }

    /// `span` less the padding on either side. A blank cell's content is the
    /// empty range after its first space, so that text typed there keeps a
    /// space on either side when the cell has two.
    pub fn table_cell_content(&self, span: (Grapheme, Grapheme)) -> (Grapheme, Grapheme) {
        let text = &self.buffer[span];
        let trimmed = text.trim_matches(is_cell_space);
        if trimmed.is_empty() {
            let content = span.start() + text.chars().count().min(1);
            return (content, content);
        }
        let padding = text.len() - text.trim_start_matches(is_cell_space).len();
        let start = self.offset_to_byte(span.start()) + padding;
        self.range_to_char_ceil((start, start + trimmed.len()))
    }

    /// Lexes the delimiter row, which the parser reports no positions for.
    /// It holds only pipes, colons, dashes, and spaces, so each of its
    /// characters is one grapheme and none is escaped.
    fn delimiter_cells(&self, line: (Grapheme, Grapheme)) -> Vec<(Grapheme, Grapheme)> {
        let text = &self.buffer[line];
        let bytes = text.as_bytes();
        let mut start = text.len() - text.trim_start_matches(is_cell_space).len();
        let mut end = text.trim_end_matches(is_cell_space).len();
        if bytes.get(start) == Some(&b'|') {
            start += 1;
        }
        if end > start && bytes[end - 1] == b'|' {
            end -= 1;
        }

        let mut cells = Vec::new();
        let mut cell_start = start;
        for (i, &byte) in bytes.iter().enumerate().take(end).skip(start) {
            if byte == b'|' {
                cells.push((line.start() + cell_start, line.start() + i));
                cell_start = i + 1;
            }
        }
        cells.push((line.start() + cell_start, line.start() + end));
        cells
    }

    /// Whether the parser added this cell to pad a short row. Such a cell
    /// has no source: comrak positions it one column past the cell before
    /// it, where a written cell starts at least two past (beyond a pipe).
    pub fn table_cell_autocompleted(&self, cell: &'ast AstNode<'ast>) -> bool {
        let Some(prev) = cell.previous_sibling() else { return false };
        cell.data.borrow().sourcepos.start.column <= prev.data.borrow().sourcepos.end.column + 1
    }

    /// The range of an inline node in a table cell whose text escapes a
    /// pipe, or `None` if `node` isn't one. comrak parses a cell's inlines
    /// from its text with each `\|` already replaced by `|`, so it reports
    /// every position after an escaped pipe short by the backslashes before
    /// it.
    pub fn table_cell_inline_range(
        &self, node: &'ast AstNode<'ast>, sourcepos: Sourcepos,
    ) -> Option<(Grapheme, Grapheme)> {
        let cell = node
            .ancestors()
            .skip(1)
            .take_while(|ancestor| !ancestor.is_container_block())
            .find(|ancestor| matches!(ancestor.data.borrow().value, NodeValue::TableCell))?;
        let span = self.range_to_byte(self.node_range(cell));
        let text = &self.buffer.current.text[span.start().0..span.end().0];
        if !text.contains("\\|") {
            return None;
        }

        // comrak counts a cell's inline columns from its first non-space
        let padding = text.len() - text.trim_start_matches(is_cell_space).len();
        let content = &text.as_bytes()[padding..];
        let content_start = span.start().0 + padding;
        let line = self.bounds.source_lines[sourcepos.start.line - 1];
        let line_start = self.offset_to_byte(line.start()).0;

        // offsets into the cell's text as comrak saw it, pipes unescaped
        let unescaped_start =
            (line_start + sourcepos.start.column - 1).checked_sub(content_start)?;
        let unescaped_end = (line_start + sourcepos.end.column).checked_sub(content_start)?;

        let (mut start, mut end) = (content.len(), content.len());
        let (mut source, mut unescaped) = (0, 0);
        let mut escaped = false; // a backslash takes the byte after it as it is
        while source < content.len() {
            if unescaped == unescaped_start {
                start = source;
            }
            if unescaped == unescaped_end {
                end = source;
                break;
            }
            let escape = !escaped && content[source] == b'\\';
            let escaped_pipe = escape && content.get(source + 1) == Some(&b'|');
            if escaped_pipe {
                source += 1; // the backslash comrak dropped
            }
            escaped = escape && !escaped_pipe;
            source += 1;
            unescaped += 1;
        }

        Some(
            self.range_to_char_ceil((
                Byte(content_start + start.min(end)),
                Byte(content_start + end),
            )),
        )
    }
}

// ─── the caret ───────────────────────────────────────────────────────

impl MdRender {
    /// Tables are grids for navigation: an advance from `origin` that lands
    /// on a table line outside cell content continues in the travel
    /// direction to the nearest content, or out of the table past its last
    /// cell. At a document edge with nowhere to leave to, it turns back.
    ///
    /// A caret on the delimiter row moves along it as through text, which
    /// is how the table shows while it is there.
    pub fn snap_offset_into_table_cell(
        &self, origin: Grapheme, offset: Grapheme, backwards: bool,
    ) -> Grapheme {
        let Some(table) = self.bounds.table_at(offset) else { return offset };
        if table
            .contents()
            .any(|content| content.contains_inclusive(offset))
        {
            return offset;
        }
        let delimiter_line = self.bounds.source_lines[table.first_line + 1];
        if delimiter_line.contains_inclusive(origin) && delimiter_line.contains_inclusive(offset) {
            return offset;
        }

        let lines = &self.bounds.source_lines;
        let (first, end) = table.lines();
        let before = table
            .contents()
            .map(|content| content.end())
            .rfind(|&content_end| content_end <= offset);
        let after = table
            .contents()
            .map(|content| content.start())
            .find(|&content_start| content_start >= offset);
        let above = first.checked_sub(1).map(|line| lines[line].end());
        let below = lines.get(end).map(|line| line.start());
        if backwards { before.or(above).or(after) } else { after.or(below).or(before) }
            .unwrap_or(offset)
    }

    /// Tables are grids for selection. A caret placed on a table line
    /// outside cell content goes to the nearest content in its row; on the
    /// delimiter row, to the header cell of its column. A selection within
    /// a cell stays within its content; across cells of a row it takes
    /// those cells whole; across rows, or across the table's edge, it takes
    /// rows whole. So no selection ends on a pipe, and none can be deleted
    /// into merged cells.
    ///
    /// A table whose delimiter row the selection is already on shows as
    /// source, and a selection may be placed anywhere along that row.
    pub fn snap_selection_into_table_cells(
        &self, range: (Grapheme, Grapheme),
    ) -> (Grapheme, Grapheme) {
        let (start, end) = (range.start(), range.end());
        let (start_table, end_table) = (self.bounds.table_at(start), self.bounds.table_at(end));
        let line_of = |offset| {
            self.bounds
                .source_lines
                .find_containing(offset, true, true)
                .0
        };
        let on_delimiter_row =
            |table: &TableBounds, offset| line_of(offset) == table.first_line + 1;

        let snapped = match (start_table, end_table) {
            (None, None) => return range,
            (Some(table), Some(end_table)) if table == end_table => {
                if on_delimiter_row(table, start)
                    && on_delimiter_row(table, end)
                    && on_delimiter_row(table, self.buffer.current.selection.1)
                {
                    return range;
                }
                if range.is_empty() {
                    let caret = self.table_caret_home(table, start);
                    return (caret, caret);
                }
                let rows = (table.row_on_line(line_of(start)), table.row_on_line(line_of(end)));
                let row = &table.rows[rows.0];
                if rows.0 != rows.1 {
                    (row.line.start(), table.rows[rows.1].line.end())
                } else if (start, end) == row.line {
                    (start, end)
                } else {
                    let (first, last) =
                        (row.cells[row.cell_near(start)], row.cells[row.cell_near(end)]);
                    if first == last {
                        let clamp = |offset: Grapheme| {
                            offset.max(first.content.start()).min(first.content.end())
                        };
                        (clamp(start), clamp(end))
                    } else {
                        (first.content.start(), last.content.end())
                    }
                }
            }
            _ => (
                start_table.map_or(start, |table| {
                    table.rows[table.row_on_line(line_of(start))].line.start()
                }),
                end_table
                    .map_or(end, |table| table.rows[table.row_on_line(line_of(end))].line.end()),
            ),
        };
        if range.0 <= range.1 { snapped } else { (snapped.1, snapped.0) }
    }

    /// Where a caret placed at `offset`, on one of `table`'s lines, goes.
    fn table_caret_home(&self, table: &TableBounds, offset: Grapheme) -> Grapheme {
        if table
            .contents()
            .any(|content| content.contains_inclusive(offset))
        {
            return offset;
        }
        let home = if let Some(row) = table.row_at(self.line_at_offset(offset).end()) {
            table.rows[row].nearest_content_edge(offset)
        } else {
            let column = table
                .delimiter
                .cells
                .iter()
                .position(|cell| offset <= cell.end())
                .unwrap_or(table.columns().saturating_sub(1));
            let header = table.rows.first();
            header.and_then(|row| Some(row.cells.get(column)?.content.start()))
        };
        home.unwrap_or(offset)
    }

    /// What `range`, a selection as [`Self::snap_selection_into_table_cells`]
    /// leaves it, takes of the table both its ends are in.
    pub fn table_selection(&self, range: (Grapheme, Grapheme)) -> Option<TableSelection<'_>> {
        let (start, end) = (range.start(), range.end());
        let table = self.bounds.table_at(start)?;
        if self.bounds.table_at(end) != Some(table) {
            return None;
        }
        let line_of = |offset| {
            self.bounds
                .source_lines
                .find_containing(offset, true, true)
                .0
        };
        if line_of(start) == table.first_line + 1 && line_of(end) == table.first_line + 1 {
            return None; // along the delimiter row, shown as source
        }
        let rows = (table.row_on_line(line_of(start)), table.row_on_line(line_of(end)));
        let row = &table.rows[rows.0];
        if rows.0 != rows.1 || (start, end) == row.line {
            return Some(TableSelection::Rows { table, rows });
        }
        let columns = (row.cell_near(start), row.cell_near(end));
        Some(if columns.0 == columns.1 {
            TableSelection::InCell { table, row: rows.0, column: columns.0 }
        } else {
            TableSelection::Cells { table, row: rows.0, columns }
        })
    }

    /// Where a caret at `offset`, on a table row outside every cell,
    /// belongs: the nearest content in that row. `None` if it may stay — off
    /// any table, inside a cell (padding included, where typing a space
    /// puts it), or on the delimiter row.
    pub fn table_caret_stranded(&self, offset: Grapheme) -> Option<Grapheme> {
        let table = self.bounds.table_at(offset)?;
        if table.cell_at(offset).is_some() {
            return None;
        }
        let row = table.row_at(self.line_at_offset(offset).end())?;
        table.rows[row].nearest_content_edge(offset)
    }

    /// The painted cell under `pos`.
    pub fn table_cell_box_at(&self, pos: egui::Pos2) -> Option<TableCellBox> {
        self.table_cell_boxes
            .iter()
            .copied()
            .find(|cell_box| cell_box.rect.contains(pos))
    }

    /// The bounds of the row a painted cell is in, with its table's.
    pub fn table_row_of(&self, cell_box: TableCellBox) -> Option<(&TableBounds, &RowBounds)> {
        let table = self.bounds.table_at(cell_box.row)?;
        let row = table
            .rows
            .iter()
            .find(|row| row.line.start() == cell_box.row)?;
        Some((table, row))
    }

    /// The rects of the cells `selection` takes whole, for a highlight that
    /// shows it has them: every cell of a row it spans, and each cell whose
    /// content it reaches beyond.
    pub fn table_cell_rects_taken_whole(&self, selection: (Grapheme, Grapheme)) -> Vec<Rect> {
        let (start, end) = (selection.start(), selection.end());
        let taken = |cell_box: &&TableCellBox| {
            let Some((_, row)) = self.table_row_of(**cell_box) else { return false };
            if start <= row.line.start() && row.line.end() <= end {
                return true;
            }
            row.cells.get(cell_box.column).is_some_and(|cell| {
                let content = cell.content;
                start <= content.start()
                    && content.end() <= end
                    && (start < content.start() || content.end() < end)
            })
        };
        self.table_cell_boxes
            .iter()
            .filter(taken)
            .map(|cell_box| cell_box.rect)
            .collect()
    }

    /// The span of the cell painted under `pos`; under a cell the row does
    /// not have, the span of the row's last.
    pub fn table_cell_span_at_pos(&self, pos: egui::Pos2) -> Option<(Grapheme, Grapheme)> {
        let cell_box = self.table_cell_box_at(pos)?;
        let (_, row) = self.table_row_of(cell_box)?;
        let cell = row.cells.get(cell_box.column).or(row.cells.last())?;
        Some(cell.span)
    }
}

impl<'ast> MdRender {
    /// Whether the table shows as its source lines instead of a grid: while
    /// the selection has an end on the delimiter row, which has no place
    /// in the grid, or a find match covers some of the table's syntax.
    pub fn table_revealed(&self, table: &'ast AstNode<'ast>) -> bool {
        let Some(&delimiter_line) = self
            .bounds
            .source_lines
            .get(self.node_first_line_idx(table) + 1)
        else {
            return false;
        };
        let on_delimiter_row = self.reveal_selection.is_some_and(|selection| {
            delimiter_line.contains_inclusive(selection.0)
                || delimiter_line.contains_inclusive(selection.1)
        });
        on_delimiter_row || {
            let range = self.node_range(table);
            let mut found = self
                .find_current_match
                .into_iter()
                .chain(self.preview_match);
            found.any(|found| {
                range.intersects(&found, false)
                    && !table.descendants().any(|cell| {
                        matches!(cell.data.borrow().value, NodeValue::TableCell)
                            && self
                                .table_cell_content(self.node_range(cell))
                                .contains_range(&found, true, true)
                    })
            })
        }
    }
}

// ─── event hooks ─────────────────────────────────────────────────────

impl MdEdit {
    /// A short row has no source for the cells it lacks. The first time the
    /// caret moves into one — by a click, the right arrow from the row's
    /// last cell, or a vertical arrow from the cell above or below — the row
    /// gets that cell's pipes. True if handled (ops + caret pushed).
    pub fn select_unwritten_table_cell(
        &self, region: Region, operations: &mut Vec<Operation>,
    ) -> bool {
        let r = &self.renderer;
        if r.readonly {
            return false;
        }
        let selection = r.buffer.current.selection;
        let caret_cell = || {
            let table = r
                .bounds
                .table_at(selection.1)
                .filter(|_| selection.is_empty())?;
            let (row, column) = table.cell_at(selection.1)?;
            Some((table, row, column))
        };
        let target = match region {
            Region::Location(Location::Pos(pos)) => r.table_cell_box_at(pos).and_then(|cell| {
                let (table, row) = r.table_row_of(cell)?;
                Some((table, row, cell.column))
            }),
            Region::ToAdvance {
                advance: Advance::By(Increment::Char),
                backwards: false,
                extend_selection: false,
            } => caret_cell().and_then(|(table, row, column)| {
                let row = &table.rows[row];
                (selection.1 >= row.cells[column].content.end()).then_some((table, row, column + 1))
            }),
            Region::ToAdvance {
                advance: Advance::By(Increment::Lines(1)),
                backwards,
                extend_selection: false,
            } => caret_cell().and_then(|(table, _, column)| {
                let neighbor = self.table_row_past(selection.1, backwards)?;
                self.table_cell_edge_row(selection.1, backwards).then_some((
                    table,
                    &table.rows[neighbor],
                    column,
                ))
            }),
            _ => None,
        };
        let Some((table, row, column)) = target else { return false };
        let Some(last) = row.cells.last() else { return false };
        if column < row.cells.len() || column >= table.columns() {
            return false;
        }

        // the caret goes between the last new cell's two spaces
        let mut before = String::new();
        let at = if row.trailing_pipe {
            last.span.end() + 1
        } else {
            let padded = r.buffer[last.span].ends_with([' ', '\t']);
            before += if padded { "|" } else { " |" };
            last.span.end()
        };
        before += &"  |".repeat(column - row.cells.len());
        before += " ";
        let at = at.into_range();
        operations.push(Operation::Replace(Replace { range: at, text: before }));
        operations.push(Operation::Select(at));
        operations.push(Operation::Replace(Replace { range: at, text: " |".into() }));
        true
    }

    /// Where shift with an arrow key moves the end of a selection that
    /// takes whole cells or rows, which a step of one character or line
    /// from that end would leave as it is: toward the anchor it gives up a
    /// cell, and up or down it takes or gives up a row. `None` otherwise.
    pub fn extend_table_selection(&self, region: Region) -> Option<(Grapheme, Grapheme)> {
        let Region::ToAdvance { advance, backwards, extend_selection: true } = region else {
            return None;
        };
        let r = &self.renderer;
        let selection = r.buffer.current.selection;
        let end_first = selection.1 < selection.0;
        let lines = &r.bounds.source_lines;
        let end = match (r.table_selection(selection)?, advance) {
            (TableSelection::Cells { table, row, columns }, Advance::By(Increment::Char))
                if backwards != end_first =>
            {
                let cells = &table.rows[row].cells;
                if end_first {
                    cells[columns.0 + 1].content.start()
                } else {
                    cells[columns.1 - 1].content.end()
                }
            }
            (TableSelection::Rows { table, rows }, Advance::By(Increment::Lines(1))) => {
                let row = if end_first { rows.0 } else { rows.1 };
                let past = if backwards { row.checked_sub(1) } else { Some(row + 1) };
                match past.and_then(|row| table.rows.get(row)) {
                    Some(row) if end_first => row.line.start(),
                    Some(row) => row.line.end(),
                    None if backwards => lines[table.first_line.checked_sub(1)?].end(),
                    None => lines.get(table.lines().1)?.start(),
                }
            }
            _ => return None,
        };
        Some((selection.0, end))
    }

    /// Navigation keeps the caret in cells, but it can still come to rest
    /// on a table row's pipes: where a document opens, after a paste that
    /// ends a row, when a merge moves it. With no place for it there, move
    /// it into the row. Undo and redo restore a selection that had a place.
    pub fn rehome_table_caret(&mut self, undo_redo: bool) -> buffer::Response {
        let selection = self.renderer.buffer.current.selection;
        if undo_redo || !selection.is_empty() {
            return buffer::Response::default();
        }
        let Some(home) = self.renderer.table_caret_stranded(selection.1) else {
            return buffer::Response::default();
        };
        self.renderer
            .buffer
            .queue(vec![Operation::Select(home.into_range())]);
        self.renderer.buffer.update()
    }
}
