//! Tables. A table is a GFM pipe table: a header row, a delimiter row, and
//! body rows, one source line each. [`TableBounds`] records where each
//! row's cells and pipes are; everything the editor does to a table is
//! arithmetic on it.

use comrak::nodes::{AstNode, NodeValue, Sourcepos, TableAlignment};
use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _};

use super::MdRender;
use super::widget::utils::NodeValueExt as _;

/// One table's rows, cells, and pipes in the source.
#[derive(Clone, Debug, PartialEq)]
pub struct TableBounds {
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

impl TableBounds {
    pub fn columns(&self) -> usize {
        self.delimiter.cells.len()
    }
}

/// The space characters the parser's row scanner pads cells with.
fn is_cell_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0b' | '\x0c')
}

impl<'ast> MdRender {
    /// Recompute [`super::bounds::Bounds::tables`]. Depends on text only.
    pub fn calc_table_bounds(&mut self, root: &'ast AstNode<'ast>) {
        let mut tables = Vec::new();
        for node in root.descendants() {
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
        let delimiter_line = self
            .bounds
            .source_lines
            .get(self.node_first_line_idx(table) + 1)?;
        let line = self.node_line(table, *delimiter_line);
        Some(TableBounds {
            rows: table.children().map(|row| self.row_bounds(row)).collect(),
            delimiter: DelimiterBounds { line, cells: self.delimiter_cells(line), alignments },
        })
    }

    pub fn row_bounds(&self, row: &'ast AstNode<'ast>) -> RowBounds {
        let line = self.node_line(row, self.node_first_line(row));
        let cells: Vec<CellBounds> = row
            .children()
            .take_while(|cell| !self.table_cell_autocompleted(cell))
            .map(|cell| {
                let span = self.node_range(cell);
                CellBounds { span, content: self.cell_content(span) }
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
    fn cell_content(&self, span: (Grapheme, Grapheme)) -> (Grapheme, Grapheme) {
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
            if escape && content.get(source + 1) == Some(&b'|') {
                source += 1; // the backslash comrak dropped
            }
            escaped = escape;
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
