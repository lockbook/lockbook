//! Table tests: the cell range invariant over a fixed corpus (the property
//! tables run it over generated tables), and exact-behavior examples.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::super::MdRender;
use super::super::table::{DelimiterBounds, RowBounds};
use super::render_props::test_renderer;

/// Tables whose cell positions the editor depends on. Each also runs inside
/// every container in [`CONTAINERS`].
const CORPUS: &[&str] = &[
    // padded, unpadded
    "| a | b |\n| - | - |\n| c | d |",
    "|a|b|\n|-|-|\n|c|d|",
    // empty cells: first, last, adjacent
    "| | b | c |\n| - | - | - |\n| a | b | |\n| a | | |\n| | | |",
    "|| b |\n|-|-|\n|| d |",
    "| a | b |\n| - | - |\n|    |\t|\n|\t\t| |",
    // `||` is a spoiler delimiter, so only a row's first two pipes can
    // enclose an empty cell
    "| a | b |\n| - | - |\n|||\n| c ||\n| c |  || d |",
    "| ||s|| | x |\n| - | - |\n| c | d |",
    // short rows
    "| a | b | c |\n| - | - | - |\n| x |\n| x | y\n|| |\n| x | \nx\n   x   ",
    // long rows
    "| a | b |\n| - | - |\n| c | d | e |\n| | | e | f |",
    // escaped pipes
    "| a \\| x | b |\n| - | - |\n| c \\| y z | *d* |\n| \\| | \\|a\\| |",
    "| \\| **b** `c` | x |\n| - | - |\n| a \\| \\| *e* f | \\|",
    "| a \\\\ \\| b &amp; c | x |\n| - | - |\n| é \\| ü ö | [[a\\|b]] c |",
    // a pipe in a code span splits the cell unless escaped
    "| `p\\|q` r | b |\n| - | - |\n| `c` | `d\\|e\\|f` g |",
    // rows without a leading or trailing pipe
    "a | b\n--|--\nc | d",
    "a | b |\n--|--|\nc | d |\ne |",
    "| a | b\n|--|--\n| c | d\n| e",
    // one column, with and without pipes
    "| a |\n| - |\n| b |\n| |\n||",
    "a\n:-:\nb",
    // alignments, delimiter row spacing
    "| a | b | c | d |\n|:-|-:|:-:|---|\n| 1 | 2 | 3 | 4 |",
    "| a | b |\n|  :---   | ---:|   \n| 1 | 2 |",
    "| a | b |\n-|-\n| 1 | 2 |",
    // space after rows, tabs as padding
    "| a | b |  \n| - | - | \n| c | d |   \n|\tc\t|\td\t|\n| c | d | \t",
    // inline content
    "| **a** b | [l](u) |\n| - | - |\n| `c` | ~~d~~ e |\n| ![i](u) | <b>x</b> :smile: |",
    // complex scripts
    "| नमस्ते | 你好 |\n| - | - |\n| é | 👨\u{200D}👩\u{200D}👧 x |",
    // header only, then a second table
    "| a | b |\n| - | - |\n\n| c |\n| - |\n| d |",
    // preceded by a paragraph line, followed by a row without pipes
    "p\n| a | b |\n| - | - |\n| c | d |\npara",
];

/// Tables with indented rows. Not run in containers, whose own indentation
/// would add up to more than a row may have.
const INDENTED: &[&str] =
    &["  | a | b |\n  | - | - |\n   | c | d |\n | e |", "| a | b |\n   |:-:| - |\n| c | d |"];

/// Container prefixes: the first line's, then every later line's.
const CONTAINERS: &[(&str, &str)] = &[
    ("", ""),
    ("> ", "> "),
    (">\t", ">\t"),
    ("- ", "  "),
    ("- ", "\t"),
    ("- ", "    "),
    ("1. ", "   "),
    ("- - ", "      "),
    ("> - ", ">   "),
    ("- x\n\n  ", "  "),
];

fn contain(md: &str, (first, rest): (&str, &str)) -> String {
    md.split('\n')
        .enumerate()
        .map(|(i, line)| format!("{}{line}", if i == 0 { first } else { rest }))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn corpus_ranges() {
    for md in CORPUS {
        for container in CONTAINERS {
            let md = contain(md, *container);
            let tables = table_ranges_check_md(&md)
                .unwrap_or_else(|reason| panic!("{reason}\nmarkdown:\n{md}"));
            assert!(tables > 0, "no table in:\n{md}");
        }
    }
    for md in INDENTED {
        let tables =
            table_ranges_check_md(md).unwrap_or_else(|reason| panic!("{reason}\nmarkdown:\n{md}"));
        assert!(tables > 0, "no table in:\n{md}");
    }
}

#[test]
fn corpus_ranges_crlf() {
    for md in CORPUS.iter().chain(INDENTED) {
        let md = md.replace('\n', "\r\n");
        let tables = table_ranges_check_md(&md)
            .unwrap_or_else(|reason| panic!("{reason}\nmarkdown:\n{md:?}"));
        assert!(tables > 0, "no table in:\n{md:?}");
    }
}

fn is_cell_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0b' | '\x0c')
}

macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            return Err(format!($($arg)*));
        }
    };
}

/// The range invariant. For every table row: its cells are in order, do
/// not overlap, and with the pipes between them tile the row's line after
/// the container prefix; they number the table's column count unless the
/// row is short; each cell's content is its span less padding; and the
/// parser's cell and inline nodes agree with all of it. The delimiter row's
/// cells tile its line the same way. Returns the number of tables checked.
pub(super) fn table_ranges_check_md(md: &str) -> Result<usize, String> {
    let mut r = test_renderer(md);
    let arena = Arena::new();
    let root = r.reparse(&arena);

    let nodes: Vec<_> = root
        .descendants()
        .filter(|node| matches!(node.data.borrow().value, NodeValue::Table(_)))
        .collect();
    ensure!(
        nodes.len() == r.bounds.tables.len(),
        "{} table nodes, {} table bounds",
        nodes.len(),
        r.bounds.tables.len()
    );

    for (node, table) in nodes.iter().zip(&r.bounds.tables) {
        let NodeValue::Table(node_table) = &node.data.borrow().value else { unreachable!() };
        let columns = node_table.num_columns;
        ensure!(table.columns() == columns, "{} columns, parser has {columns}", table.columns());
        delimiter_check(&r, &table.delimiter)?;

        let row_nodes: Vec<_> = node.children().collect();
        ensure!(row_nodes.len() == table.rows.len(), "row count differs from the parser's");
        for (i, (row_node, row)) in row_nodes.iter().zip(&table.rows).enumerate() {
            ensure!(row.cells.len() == columns || i != 0, "header row is short");
            row_check(&r, row, columns).map_err(|reason| format!("row {i}: {reason}"))?;
            row_nodes_check(&r, row_node, row).map_err(|reason| format!("row {i}: {reason}"))?;
        }
    }
    Ok(nodes.len())
}

fn char_at(r: &MdRender, offset: Grapheme) -> &str {
    if offset < r.last_cursor_position() { &r.buffer[(offset, offset + 1)] } else { "" }
}

fn row_check(r: &MdRender, row: &RowBounds, columns: usize) -> Result<(), String> {
    let line = row.line;
    ensure!(!row.cells.is_empty(), "no cells");
    ensure!(row.cells.len() <= columns, "{} cells in {columns} columns", row.cells.len());

    let indentation = r.buffer[line]
        .chars()
        .take_while(|&c| is_cell_space(c))
        .count();
    let mut at = line.start() + indentation;
    if row.leading_pipe {
        ensure!(char_at(r, at) == "|", "no leading pipe at {at:?}");
        at += 1;
    }
    for (i, cell) in row.cells.iter().enumerate() {
        if i > 0 {
            ensure!(char_at(r, at) == "|", "no pipe before cell {i} at {at:?}");
            at += 1;
        }
        let (span, content) = (cell.span, cell.content);
        ensure!(span.0 == at, "cell {i} spans {span:?} but the row is tiled up to {at:?}");
        ensure!(span.0 <= span.1, "cell {i} span {span:?} runs backwards");
        ensure!(span.1 <= line.end(), "cell {i} span {span:?} leaves its line {line:?}");
        at = span.1;

        let text = &r.buffer[span];
        ensure!(!text.contains(['\n', '\r']), "cell {i} spans a line break");
        let trimmed = text.trim_matches(is_cell_space);
        ensure!(
            span.0 <= content.0 && content.0 <= content.1 && content.1 <= span.1,
            "cell {i} content {content:?} outside its span {span:?}"
        );
        if trimmed.is_empty() {
            let one_space_in = span.0 + if span.is_empty() { 0 } else { 1 };
            ensure!(content == (one_space_in, one_space_in), "blank cell {i} content {content:?}");
        } else {
            ensure!(&r.buffer[content] == trimmed, "cell {i} content {:?}", &r.buffer[content]);
        }
    }
    if row.trailing_pipe {
        ensure!(char_at(r, at) == "|", "no trailing pipe at {at:?}");
        at += 1;
    } else {
        ensure!(at == line.end(), "no trailing pipe, yet the last cell ends before the line");
    }
    if row.cells.len() < columns {
        let rest = &r.buffer[(at, line.end())];
        ensure!(rest.chars().all(is_cell_space), "short row has more to parse: {rest:?}");
    }
    Ok(())
}

/// The parser's nodes agree with the row's bounds: a written cell's node
/// has the cell's span and inline children that tile its content; a cell
/// the parser added to pad the row is empty.
fn row_nodes_check<'a>(
    r: &MdRender, row_node: &'a AstNode<'a>, row: &RowBounds,
) -> Result<(), String> {
    let row_end = r.node_range(row_node).end();
    for (i, cell_node) in row_node.children().enumerate() {
        let range = r.node_range(cell_node);
        let Some(cell) = row.cells.get(i) else {
            ensure!(r.table_cell_autocompleted(cell_node), "cell {i} is unwritten, yet parsed");
            ensure!(range == (row_end, row_end), "unwritten cell {i} has range {range:?}");
            ensure!(cell_node.first_child().is_none(), "unwritten cell {i} has content");
            continue;
        };
        ensure!(range == cell.span, "cell {i} node range {range:?}, span {:?}", cell.span);

        let mut at = cell.content.start();
        for inline in cell_node.children() {
            let inline_range = r.node_range(inline);
            ensure!(
                inline_range.0 == at && inline_range.0 <= inline_range.1,
                "cell {i} inline {inline_range:?} ({:?}) does not continue from {at:?}",
                &r.buffer[inline_range]
            );
            at = inline_range.1;
        }
        ensure!(at == cell.content.end(), "cell {i} inlines end at {at:?}, content {cell:?}");
    }
    Ok(())
}

fn delimiter_check(r: &MdRender, delimiter: &DelimiterBounds) -> Result<(), String> {
    let line = delimiter.line;
    ensure!(delimiter.cells.len() == delimiter.alignments.len(), "delimiter cell count");

    let indentation = r.buffer[line]
        .chars()
        .take_while(|&c| is_cell_space(c))
        .count();
    let mut at = line.start() + indentation;
    if char_at(r, at) == "|" {
        at += 1;
    }
    for (i, (cell, alignment)) in delimiter
        .cells
        .iter()
        .zip(&delimiter.alignments)
        .enumerate()
    {
        if i > 0 {
            ensure!(char_at(r, at) == "|", "no pipe before delimiter cell {i}");
            at += 1;
        }
        ensure!(
            cell.0 == at && cell.0 <= cell.1,
            "delimiter cell {i} is {cell:?}, tiled to {at:?}"
        );
        at = cell.1;

        let text = r.buffer[*cell].trim_matches(is_cell_space);
        let dashes = text.trim_start_matches(':').trim_end_matches(':');
        ensure!(
            !dashes.is_empty() && dashes.chars().all(|c| c == '-'),
            "delimiter cell {i} is {text:?}"
        );
        let written = match (text.starts_with(':'), text.ends_with(':')) {
            (true, true) => TableAlignment::Center,
            (true, false) => TableAlignment::Left,
            (false, true) => TableAlignment::Right,
            (false, false) => TableAlignment::None,
        };
        ensure!(written == *alignment, "delimiter cell {i} {text:?} aligns {alignment:?}");
    }
    if char_at(r, at) == "|" {
        at += 1;
    }
    let rest = &r.buffer[(at, line.end())];
    ensure!(rest.chars().all(is_cell_space), "delimiter row has more to lex: {rest:?}");
    Ok(())
}

// ═════════════════════════ exact-behavior examples ═════════════════════════

/// The source text of each cell's content, row by row, with `None` for a
/// cell the row does not have.
fn cell_texts(md: &str) -> Vec<Vec<Option<String>>> {
    let mut r = test_renderer(md);
    let arena = Arena::new();
    r.reparse(&arena);
    let table = &r.bounds.tables[0];
    table
        .rows
        .iter()
        .map(|row| {
            (0..table.columns())
                .map(|i| {
                    row.cells
                        .get(i)
                        .map(|cell| r.buffer[cell.content].to_string())
                })
                .collect()
        })
        .collect()
}

fn texts(rows: &[&[Option<&str>]]) -> Vec<Vec<Option<String>>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| cell.map(str::to_string)).collect())
        .collect()
}

#[test]
fn cells_in_list_item_continuation_lines() {
    for indent in ["  ", "\t", "    "] {
        let md = format!("- | a | b |\n{indent}| - | - |\n{indent}| c | d |");
        assert_eq!(
            cell_texts(&md),
            texts(&[&[Some("a"), Some("b")], &[Some("c"), Some("d")]]),
            "continuation indent {indent:?}"
        );
    }
}

#[test]
fn short_row_cells_are_absent() {
    assert_eq!(
        cell_texts("| a | b | c |\n| - | - | - |\n| x |\n| x | y\n|| z |"),
        texts(&[
            &[Some("a"), Some("b"), Some("c")],
            &[Some("x"), None, None],
            &[Some("x"), Some("y"), None],
            &[Some(""), Some("z"), None],
        ])
    );
}

#[test]
fn long_row_cells_are_truncated() {
    assert_eq!(
        cell_texts("| a | b |\n| - | - |\n| c | d | e |"),
        texts(&[&[Some("a"), Some("b")], &[Some("c"), Some("d")]])
    );
}

/// The text each of a document's inline leaves covers.
fn inline_leaf_texts(md: &str) -> Vec<String> {
    let mut r = test_renderer(md);
    let arena = Arena::new();
    let root = r.reparse(&arena);
    root.descendants()
        .filter(|node| matches!(node.data.borrow().value, NodeValue::Text(_) | NodeValue::Code(_)))
        .map(|node| r.buffer[r.node_range(node)].to_string())
        .collect()
}

#[test]
fn inlines_after_an_escaped_pipe() {
    assert_eq!(
        inline_leaf_texts("| a \\| **b** `c\\|d` é | x |\n| - | - |\n| \\\\ \\| y | z |"),
        ["a \\| ", "b", " ", "`c\\|d`", " é", "x", "\\", " \\| y", "z"]
    );
}
