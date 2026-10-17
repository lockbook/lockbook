//! Table tests: the cell range invariant over a fixed corpus (the property
//! tables run it over generated tables), and exact-behavior examples.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::super::MdRender;
use super::super::input::{Advance, Bound, Event, Increment, Location, Region};
use super::super::table::{DelimiterBounds, RowBounds, TableAction};
use super::super::widget::toolbar::{MarkdownToolbarAction as ToolbarAction, TableTool};
use super::harness::TestEditor;
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
    "| \\|\\| x | \\\\\\| y |\n| - | - |\n| \\|\\|\\| | z\\|\\|",
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

// ─── the caret ───────────────────────────────────────────────────────

const RIGHT: Region = arrow(Advance::By(Increment::Char), false);
const LEFT: Region = arrow(Advance::By(Increment::Char), true);
const DOWN: Region = arrow(Advance::By(Increment::Lines(1)), false);
const UP: Region = arrow(Advance::By(Increment::Lines(1)), true);

const fn arrow(advance: Advance, backwards: bool) -> Region {
    Region::ToAdvance { advance, backwards, extend_selection: false }
}

/// Byte offsets, which are grapheme offsets in the ASCII documents here.
fn caret_at(editor: &mut TestEditor, offset: usize) {
    editor.editor.edit.cursor.x_target = None;
    editor.push(Event::Select { region: Region::Location(Location::Grapheme(Grapheme(offset))) });
    editor.enter_frame();
}

fn press(editor: &mut TestEditor, region: Region) -> usize {
    editor.push(Event::Select { region });
    editor.enter_frame();
    caret(editor)
}

fn caret(editor: &TestEditor) -> usize {
    let selection = editor.editor.edit.renderer.buffer.current.selection;
    assert!(selection.is_empty(), "selection {selection:?} is not a caret");
    selection.1.0
}

fn type_text(editor: &mut TestEditor, text: &str) {
    editor.push(Event::Replace {
        region: Region::Selection,
        text: text.into(),
        advance_cursor: true,
    });
    editor.enter_frame();
}

/// The caret positions a table offers: every position in each cell's
/// content, as (row, column, offset).
fn cell_positions(editor: &TestEditor) -> Vec<(usize, usize, usize)> {
    let table = &editor.editor.edit.renderer.bounds.tables[0];
    let mut positions = Vec::new();
    for (r, row) in table.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            positions.extend((cell.content.0.0..=cell.content.1.0).map(|offset| (r, c, offset)));
        }
    }
    positions
}

const GRID: &str = "before\n\n| ab | c |  |\n| - | - | - |\n|| d | ef |\n| g |\n\nafter";

#[test]
fn right_arrow_visits_every_cell_position_in_order() {
    let mut editor = TestEditor::new(GRID);
    let expected: Vec<usize> = cell_positions(&editor)
        .into_iter()
        // the last row is short; arrowing past its cell writes the next
        .filter(|&(r, _, _)| r < 2)
        .map(|(_, _, offset)| offset)
        .collect();

    caret_at(&mut editor, GRID.find("before").unwrap() + 6);
    let mut visited = Vec::new();
    for _ in 0..expected.len() {
        visited.push(press(&mut editor, RIGHT));
        // step over the blank line between the paragraph and the table
        if visited.last() == Some(&7) {
            visited.pop();
            visited.push(press(&mut editor, RIGHT));
        }
    }
    assert_eq!(visited, expected);
}

#[test]
fn left_arrow_visits_every_cell_position_in_order() {
    let mut editor = TestEditor::new(GRID);
    let mut expected: Vec<usize> = cell_positions(&editor)
        .into_iter()
        .map(|(_, _, offset)| offset)
        .collect();
    expected.reverse();

    // the blank line after the table
    caret_at(&mut editor, GRID.find("\n\nafter").unwrap() + 1);
    let visited: Vec<usize> = (0..expected.len())
        .map(|_| press(&mut editor, LEFT))
        .collect();
    assert_eq!(visited, expected);
    // and out of the table, to the end of the line before it
    assert_eq!(press(&mut editor, LEFT), GRID.find("\n| ab").unwrap());
}

/// The caret's cell and its distance into the cell's content.
fn caret_cell(editor: &TestEditor) -> Option<(usize, usize, usize)> {
    let caret = Grapheme(caret(editor));
    let table = &editor.editor.edit.renderer.bounds.tables[0];
    let (row, column) = table.cell_at(caret)?;
    let content = table.rows[row].cells[column].content;
    content
        .contains_inclusive(caret)
        .then(|| (row, column, caret.0 - content.0.0))
}

#[test]
fn vertical_arrows_keep_the_column() {
    let md = "| xx | xx | xx |\n| - | - | - |\n| xx | xx | xx |\n| xx | xx | xx |\n\nafter";
    let mut editor = TestEditor::new(md);
    let content = |editor: &TestEditor, row: usize, column: usize| {
        editor.editor.edit.renderer.bounds.tables[0].rows[row].cells[column].content
    };
    for column in 0..3 {
        for into in 0..=2 {
            let top = content(&editor, 0, column).0.0 + into;
            caret_at(&mut editor, top);
            press(&mut editor, DOWN);
            assert_eq!(caret_cell(&editor), Some((1, column, into)));
            press(&mut editor, DOWN);
            assert_eq!(caret_cell(&editor), Some((2, column, into)));
            // out of the table at its bottom edge
            assert_eq!(press(&mut editor, DOWN), md.find("\nafter").unwrap());

            let bottom = content(&editor, 2, column).0.0 + into;
            caret_at(&mut editor, bottom);
            press(&mut editor, UP);
            assert_eq!(caret_cell(&editor), Some((1, column, into)));
            press(&mut editor, UP);
            assert_eq!(caret_cell(&editor), Some((0, column, into)));
            // the table starts the document: nowhere to go
            press(&mut editor, UP);
            assert_eq!(caret_cell(&editor), Some((0, column, into)));
        }
    }
}

#[test]
fn vertical_arrows_enter_the_cell_under_the_caret() {
    // the last column's text is short, so the middle column's is nearer the
    // caret in x; it still lands in the last column, which it is over
    let md = "x\n\n| a | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | c |\n| - | - | - |\n";
    let mut editor = TestEditor::new(md);
    let [_, _, last] = column_centers(&editor);

    let c = md.find("| c |").unwrap() + 2;
    caret_at(&mut editor, c);
    assert_eq!(press(&mut editor, UP), 2);
    editor.editor.edit.cursor.x_target = Some(last);
    assert!((c..=c + 1).contains(&press(&mut editor, DOWN)));
}

fn column_centers<const N: usize>(editor: &TestEditor) -> [f32; N] {
    let boxes = &editor.editor.edit.renderer.table_cell_boxes;
    std::array::from_fn(|column| boxes[column].rect.center().x)
}

#[test]
fn placing_the_caret_on_structure_puts_it_in_a_cell() {
    let md = "> | ab | c |\n> | - | - |\n> | d |  |";
    let mut editor = TestEditor::new(md);
    let ab = md.find("ab").unwrap();
    let d = md.find('d').unwrap();
    for (placed, lands) in [
        (0, ab),                                               // the quote's prefix
        (2, ab),                                               // the leading pipe
        (ab - 1, ab),                                          // padding
        (ab + 2, ab + 2),                                      // the content's end
        (ab + 3, ab + 2),                                      // a pipe between cells
        (ab + 4, ab + 5),                                      // padding before the next cell
        (md.find("\n").unwrap(), md.find("c |").unwrap() + 1), // the line's end
        (md.find("| - ").unwrap() + 2, ab),                    // the delimiter row
        (md.find("- |\n").unwrap(), md.find("c |").unwrap()),
        (d + 2, d + 1),
        (md.len(), md.len() - 2), // the blank cell, one space in
    ] {
        caret_at(&mut editor, placed);
        assert_eq!(caret(&editor), lands, "caret placed at {placed}");
    }
}

#[test]
fn a_pointer_in_a_cell_resolves_to_that_cell() {
    let md = "| a | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb |  |\n| - | - | - |\n| d | e | f |";
    let mut editor = TestEditor::new(md);
    let boxes = editor.editor.edit.renderer.table_cell_boxes.clone();
    assert_eq!(boxes.len(), 6);
    let table = editor.editor.edit.renderer.bounds.tables[0].clone();
    for cell_box in boxes {
        let row = table
            .rows
            .iter()
            .find(|row| row.line.start() == cell_box.row)
            .unwrap();
        let content = row.cells[cell_box.column].content;
        // just inside the cell's corners, over its padding, and its middle
        let rect = cell_box.rect.shrink(1.0);
        for pos in [rect.min, rect.max, rect.left_bottom(), rect.right_top(), rect.center()] {
            editor.push(Event::Select { region: Region::Location(Location::Pos(pos)) });
            editor.enter_frame();
            let caret = Grapheme(caret(&editor));
            assert!(
                content.contains_inclusive(caret),
                "{pos:?} in column {} landed at {caret:?}, outside {content:?}",
                cell_box.column
            );
        }
    }
}

#[test]
fn the_caret_entering_an_unwritten_cell_writes_it() {
    let md = "| a | b | c |\n| - | - | - |\n| d |\n| e | f";

    // right arrow from the row's last cell
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find('d').unwrap() + 1);
    press(&mut editor, RIGHT);
    assert_eq!(editor.get_text(), "| a | b | c |\n| - | - | - |\n| d |  |\n| e | f");
    assert_eq!(caret(&editor), md.find("d |").unwrap() + 4);

    // down arrow from the cell above; the row gains its closing pipe too
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find('c').unwrap());
    press(&mut editor, DOWN);
    assert_eq!(editor.get_text(), "| a | b | c |\n| - | - | - |\n| d |  |  |\n| e | f");
    assert_eq!(caret(&editor), md.find("d |").unwrap() + 7);
    press(&mut editor, DOWN);
    assert_eq!(editor.get_text(), "| a | b | c |\n| - | - | - |\n| d |  |  |\n| e | f |  |");
    assert_eq!(caret(&editor), editor.get_text().len() - 2);

    // a click in it
    let mut editor = TestEditor::new(md);
    let unwritten = editor.editor.edit.renderer.table_cell_boxes[5]
        .rect
        .center();
    editor.push(Event::Select { region: Region::Location(Location::Pos(unwritten)) });
    editor.enter_frame();
    assert_eq!(editor.get_text(), "| a | b | c |\n| - | - | - |\n| d |  |  |\n| e | f");
    assert_eq!(caret(&editor), md.find("d |").unwrap() + 7);

    // one undo unit
    editor.push(Event::Undo);
    editor.enter_frame();
    assert_eq!(editor.get_text(), md);
}

#[test]
fn a_space_typed_at_the_end_of_a_cell_shows_until_the_next_character() {
    let md = "| a | b |\n| - | - |\n| c | d |";
    let mut editor = TestEditor::new(md);
    let c = md.find('c').unwrap();
    caret_at(&mut editor, c + 1);
    let x = |editor: &TestEditor| {
        editor
            .editor
            .edit
            .cursor_line(Grapheme(caret(editor)))
            .unwrap()[0]
            .x
    };

    let before = x(&editor);
    type_text(&mut editor, " ");
    assert_eq!(caret(&editor), c + 2);
    let after_space = x(&editor);
    assert!(after_space > before, "the caret did not move past the typed space");
    type_text(&mut editor, "x");
    assert_eq!(editor.get_text(), "| a | b |\n| - | - |\n| c x | d |");
    assert!(x(&editor) > after_space);
}

#[test]
fn a_caret_stranded_on_a_pipe_moves_into_the_row() {
    let md = "x\n\n";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.len());
    // a paste: the caret ends after the last row's closing pipe
    type_text(&mut editor, "| a | b |\n| - | - |\n| c | d |");
    let text = editor.get_text().to_string();
    assert_eq!(caret(&editor), text.find("d |").unwrap() + 1);

    // a document that opens on a table, once its saved selection is restored
    let mut editor = TestEditor::new("| a | b |\n| - | - |");
    editor.enter_frame();
    assert_eq!(caret(&editor), 2);
}

#[test]
fn a_caret_typed_onto_the_delimiter_row_shows_the_table_as_source() {
    let md = "| a | b |\n|-|";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.len());
    type_text(&mut editor, "-");
    assert_eq!(editor.editor.edit.renderer.bounds.tables.len(), 1);
    // the caret stays where it was typed, and moves along the row as text
    assert_eq!(caret(&editor), md.len() + 1);
    assert!(
        editor
            .editor
            .edit
            .cursor_line(Grapheme(md.len() + 1))
            .is_some()
    );
    assert_eq!(press(&mut editor, LEFT), md.len());
    assert_eq!(press(&mut editor, LEFT), md.len() - 1);
    assert_eq!(press(&mut editor, arrow(Advance::To(Bound::Line), false)), md.len() + 1);
    // leaving the row puts the caret in a cell
    assert_eq!(press(&mut editor, UP), md.find('a').unwrap() + 1);
}

// ─── regressions ─────────────────────────────────────────────────────

const EMPTY_CELL_ROW: &str =
    "|Use Case|Required Behavior|\n|-|-|\n|iOS end-of-line cursor placement||";

/// #3789
#[test]
fn a_row_with_an_empty_cell_paints_its_text_once() {
    let mut editor = TestEditor::new(EMPTY_CELL_ROW);
    editor.enter_frame();
    let mut painted: Vec<_> = editor
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .map(|fragment| fragment.source_range)
        .filter(|range| !range.is_empty())
        .collect();
    painted.sort();
    for pair in painted.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "{:?} and {:?} paint the same text", pair[0], pair[1]);
    }
}

/// #3788
#[test]
fn bold_in_a_row_with_an_empty_cell() {
    use comrak::nodes::NodeValue::Strong;
    let mut editor = TestEditor::new(EMPTY_CELL_ROW);
    let start = EMPTY_CELL_ROW.find("iOS").unwrap();
    let end = EMPTY_CELL_ROW.find("||").unwrap();
    editor.push(Event::Select { region: (Grapheme(start), Grapheme(end)).into() });
    editor.enter_frame();
    editor.push(Event::ToggleStyle { region: Region::Selection, style: Strong });
    editor.enter_frame();
    assert_eq!(
        editor.get_text(),
        "|Use Case|Required Behavior|\n|-|-|\n|**iOS end-of-line cursor placement**||"
    );
    editor.push(Event::ToggleStyle { region: Region::Selection, style: Strong });
    editor.enter_frame();
    assert_eq!(editor.get_text(), EMPTY_CELL_ROW);
}

// ─── selection ───────────────────────────────────────────────────────

const SHIFT_RIGHT: Region = shift_arrow(Advance::By(Increment::Char), false);
const SHIFT_LEFT: Region = shift_arrow(Advance::By(Increment::Char), true);
const SHIFT_DOWN: Region = shift_arrow(Advance::By(Increment::Lines(1)), false);
const SHIFT_UP: Region = shift_arrow(Advance::By(Increment::Lines(1)), true);

const fn shift_arrow(advance: Advance, backwards: bool) -> Region {
    Region::ToAdvance { advance, backwards, extend_selection: true }
}

fn select(editor: &mut TestEditor, from: usize, to: usize) -> (usize, usize) {
    editor.editor.edit.cursor.x_target = None;
    editor.push(Event::Select { region: (Grapheme(from), Grapheme(to)).into() });
    editor.enter_frame();
    selection(editor)
}

fn selection(editor: &TestEditor) -> (usize, usize) {
    let selection = editor.editor.edit.renderer.buffer.current.selection;
    (selection.0.0, selection.1.0)
}

fn extend(editor: &mut TestEditor, region: Region) -> (usize, usize) {
    editor.push(Event::Select { region });
    editor.enter_frame();
    selection(editor)
}

/// The selected text.
fn selected(editor: &TestEditor) -> &str {
    let (a, b) = selection(editor);
    &editor.get_text()[a.min(b)..a.max(b)]
}

const SHEET: &str =
    "above\n\n| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |\n| gg | hh | ii |\n\nbelow";

#[test]
fn a_selection_snaps_to_cells_and_rows() {
    let mut editor = TestEditor::new(SHEET);
    let at = |text: &str| SHEET.find(text).unwrap();
    for (from, to, takes) in [
        // within a cell: ordinary, less any padding
        (at("dd"), at("dd") + 1, "d"),
        (at("dd") - 1, at("dd") + 3, "dd"),
        // across cells of a row: whole cells
        (at("dd") + 1, at("ee") + 1, "dd | ee"),
        (at("dd") + 2, at("ff"), "dd | ee | ff"),
        (at("| ee"), at("ee") + 1, "dd | ee"),
        // across rows: whole rows
        (at("dd") + 1, at("gg") + 1, "| dd | ee | ff |\n| gg | hh | ii |"),
        (at("bb"), at("ee"), "| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |"),
        // an end on the delimiter row counts as the header
        (at("| - |") + 2, at("ee"), "| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |"),
        // across the table's edge: whole rows from the edge
        (at("ove"), at("bb"), "ove\n\n| aa | bb | cc |"),
        (at("hh"), at("low"), "| gg | hh | ii |\n\nbe"),
        // around the table: untouched
        (at("ove"), at("low"), &SHEET[at("ove")..at("low")]),
    ] {
        select(&mut editor, from, to);
        assert_eq!(selected(&editor), takes, "selecting {from}..{to}");
        // and the same backwards, keeping its direction
        let (anchor, end) = select(&mut editor, to, from);
        assert_eq!(selected(&editor), takes, "selecting {to}..{from}");
        assert!(anchor > end, "selecting {to}..{from} flipped");
    }
}

#[test]
fn shift_arrows_extend_by_cell_and_by_row() {
    let mut editor = TestEditor::new(SHEET);
    let at = |text: &str| SHEET.find(text).unwrap();

    // right: through the cell, then cell by cell, then onto the next row
    caret_at(&mut editor, at("dd") + 1);
    extend(&mut editor, SHIFT_RIGHT);
    assert_eq!(selected(&editor), "d");
    extend(&mut editor, SHIFT_RIGHT);
    assert_eq!(selected(&editor), "dd | ee");
    extend(&mut editor, SHIFT_RIGHT);
    assert_eq!(selected(&editor), "dd | ee | ff");
    extend(&mut editor, SHIFT_RIGHT);
    assert_eq!(selected(&editor), "| dd | ee | ff |\n| gg | hh | ii |");
    // and back: a row, then cell by cell
    extend(&mut editor, SHIFT_UP);
    assert_eq!(selected(&editor), "| dd | ee | ff |");

    caret_at(&mut editor, at("dd") + 1);
    for (region, takes) in [
        (SHIFT_RIGHT, "d"),
        (SHIFT_RIGHT, "dd | ee"),
        (SHIFT_RIGHT, "dd | ee | ff"),
        (SHIFT_LEFT, "dd | ee"),
        (SHIFT_LEFT, "dd"),
        (SHIFT_LEFT, "d"),
        (SHIFT_LEFT, ""),
    ] {
        extend(&mut editor, region);
        assert_eq!(selected(&editor), takes);
    }

    // left from the last cell, then back right
    caret_at(&mut editor, at("ff") + 1);
    for (region, takes) in [
        (SHIFT_LEFT, "f"),
        (SHIFT_LEFT, "ee | ff"),
        (SHIFT_LEFT, "dd | ee | ff"),
        (SHIFT_RIGHT, "ee | ff"),
        (SHIFT_RIGHT, "ff"),
    ] {
        extend(&mut editor, region);
        assert_eq!(selected(&editor), takes);
    }

    // down and up by row, through the anchor's row and out of the table
    caret_at(&mut editor, at("ee") + 1);
    for (region, takes) in [
        (SHIFT_DOWN, "| dd | ee | ff |\n| gg | hh | ii |"),
        (SHIFT_UP, "| dd | ee | ff |"),
        (SHIFT_UP, "| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |"),
        (SHIFT_UP, "\n| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |"),
        (SHIFT_DOWN, "| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |"),
        (SHIFT_DOWN, "| dd | ee | ff |"),
        (SHIFT_DOWN, "| dd | ee | ff |\n| gg | hh | ii |"),
        (SHIFT_DOWN, "| dd | ee | ff |\n| gg | hh | ii |\n"),
    ] {
        extend(&mut editor, region);
        assert_eq!(selected(&editor), takes);
    }
}

// ─── editing ─────────────────────────────────────────────────────────

const BACKSPACE: Region =
    Region::SelectionOrAdvance { advance: Advance::By(Increment::Char), backwards: true };
const DELETE: Region =
    Region::SelectionOrAdvance { advance: Advance::By(Increment::Char), backwards: false };

fn delete(editor: &mut TestEditor, region: Region) {
    editor.push(Event::Delete { region });
    editor.enter_frame();
}

/// The content of each cell of the document's first table, row by row.
fn cells(editor: &TestEditor) -> Vec<Vec<String>> {
    let r = &editor.editor.edit.renderer;
    let Some(table) = r.bounds.tables.first() else { return Vec::new() };
    table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| r.buffer[cell.content].to_string())
                .collect()
        })
        .collect()
}

/// Tables in the styles a hand-written one has, each with cells `a`..`f`.
const STYLES: &[&str] = &[
    "| a | b | c |\n| - | - | - |\n| d | e | f |",
    "|a|b|c|\n|-|-|-|\n|d|e|f|",
    "a | b | c\n--|--|--\nd | e | f",
    "> | a | b | c |\n> | - | - | - |\n> | d | e | f |",
    "- | a | b | c |\n  | - | - | - |\n  | d | e | f |",
];

#[test]
fn typing_in_a_cell_stays_in_the_cell() {
    let typed = [
        "x",
        " ",
        "|",
        "||",
        "|||",
        "'",
        "''",
        "\\",
        "\\\\",
        "'|",
        "|'",
        "\\|",
        "x|y",
        "a\nb",
        "a\r\nb",
        "- ",
        "# ",
        "1. ",
        "> ",
        "|x|",
        "'x'",
        "\\'",
        "नमस्ते",
        "👨\u{200D}👩\u{200D}👧",
    ];
    for md in STYLES {
        for cell in ["a", "b", "c", "d", "e", "f"] {
            for into in 0..=1 {
                for text in typed {
                    let mut editor = TestEditor::new(md);
                    let before = cells(&editor);
                    caret_at(&mut editor, md.find(cell).unwrap() + into);
                    type_text(&mut editor, text);

                    let context = format!(
                        "typing {text:?} at {into} in {cell:?} of\n{md}\ngave\n{}",
                        editor.get_text()
                    );
                    table_ranges_check_md(editor.get_text())
                        .unwrap_or_else(|reason| panic!("{reason}\n{context}"));
                    let after = cells(&editor);
                    assert_eq!(after.len(), before.len(), "rows changed: {context}");
                    for (row, (after, before)) in after.iter().zip(&before).enumerate() {
                        assert_eq!(after.len(), before.len(), "row {row} resized: {context}");
                        for (after, before) in after.iter().zip(before) {
                            assert!(
                                after == before || before == cell,
                                "another cell changed: {context}"
                            );
                        }
                    }
                    // the caret is still in the cell it typed in
                    let table = &editor.editor.edit.renderer.bounds.tables[0];
                    let in_cell = table.cell_at(Grapheme(caret(&editor)));
                    let typed_cell = before.iter().flatten().position(|text| text == cell);
                    assert_eq!(
                        in_cell.map(|(row, column)| row * 3 + column),
                        typed_cell,
                        "{context}"
                    );
                }
            }
        }
    }
}

#[test]
fn typed_text_reads_as_typed() {
    for (md, at, text, gives) in [
        // a pipe is escaped
        ("| a | b |\n|-|-|", 3, "|", "| a\\| | b |\n|-|-|"),
        ("| a | b |\n|-|-|", 2, "x|y", "| x\\|ya | b |\n|-|-|"),
        // a newline becomes a space
        ("| a | b |\n|-|-|", 3, "b\nc", "| ab c | b |\n|-|-|"),
        // a spoiler's pipes pair, and stay
        ("| a | b |\n|-|-|", 3, " ||s||", "| a ||s|| | b |\n|-|-|"),
        // in an unpadded cell, what would take the cell's own pipes is padded off them
        ("|a|b|\n|-|-|", 2, "'", "|a' |b|\n|-|-|"),
        ("|a|b|\n|-|-|", 2, "\\", "|a\\ |b|\n|-|-|"),
        ("|a|b|\n|-|-|", 1, "'", "| 'a|b|\n|-|-|"),
        ("|a|b|\n|-|-|", 2, "|", "|a\\| |b|\n|-|-|"),
        // a first cell with no pipe before it gets one rather than start a list
        ("a | b\n--|--\nc | d", 12, "- ", "a | b\n--|--\n|- c | d"),
    ] {
        let mut editor = TestEditor::new(md);
        caret_at(&mut editor, at);
        type_text(&mut editor, text);
        assert_eq!(editor.get_text(), gives, "typing {text:?} at {at} in\n{md}");
    }
}

#[test]
fn backspace_and_delete_never_join_cells() {
    let md = "| ab | c |  |\n|-|-|-|\n|d|e|f|";
    let at = |text: &str| md.find(text).unwrap();
    for (caret, region, gives) in [
        // at a cell's edge: nothing
        (at("ab"), BACKSPACE, md),
        (at("ab") + 2, DELETE, md),
        (at("c"), BACKSPACE, md),
        (at("  |") + 1, BACKSPACE, md),
        (at("  |") + 1, DELETE, md),
        (at("d"), BACKSPACE, md),
        (at("f") + 1, DELETE, md),
        // inside: a character
        (at("ab") + 1, BACKSPACE, "| b | c |  |\n|-|-|-|\n|d|e|f|"),
        (at("ab") + 1, DELETE, "| a | c |  |\n|-|-|-|\n|d|e|f|"),
        // the last character of an unpadded cell leaves the cell its place
        (at("e") + 1, BACKSPACE, "| ab | c |  |\n|-|-|-|\n|d|  |f|"),
    ] {
        let mut editor = TestEditor::new(md);
        caret_at(&mut editor, caret);
        delete(&mut editor, region);
        assert_eq!(editor.get_text(), gives, "deleting from {caret}");
    }

    // against a table from outside, the caret steps in
    let md = "above\n| a |\n|-|\n| b |\n\nbelow";
    let mut editor = TestEditor::new(md);
    let last_cell = md.find("b |").unwrap() + 1;
    caret_at(&mut editor, 5);
    delete(&mut editor, DELETE);
    assert_eq!((editor.get_text(), caret(&editor)), (md, md.find("a |").unwrap()));
    caret_at(&mut editor, md.find("\nbelow").unwrap());
    delete(&mut editor, BACKSPACE);
    assert_eq!((editor.get_text(), caret(&editor)), (md, last_cell));
    // and the blank line that keeps text below from being a row stays
    for (at, region) in
        [(md.find("below").unwrap(), BACKSPACE), (md.find("\nbelow").unwrap(), DELETE)]
    {
        caret_at(&mut editor, at);
        delete(&mut editor, region);
        assert_eq!((editor.get_text(), caret(&editor)), (md, last_cell), "from {at}");
    }
}

#[test]
fn a_typed_space_can_be_deleted() {
    let md = "| a | b |\n|-|-|";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, 3);
    type_text(&mut editor, " ");
    type_text(&mut editor, " ");
    assert_eq!(editor.get_text(), "| a   | b |\n|-|-|");
    delete(&mut editor, BACKSPACE);
    assert_eq!((editor.get_text(), caret(&editor)), ("| a  | b |\n|-|-|", 4));
    // as a platform that names the range words it
    editor.push(Event::Replace {
        region: (Grapheme(3), Grapheme(4)).into(),
        text: "".into(),
        advance_cursor: true,
    });
    editor.enter_frame();
    assert_eq!((editor.get_text(), caret(&editor)), (md, 3));
    // and then the cell's edge holds
    editor.push(Event::Replace {
        region: (Grapheme(2), Grapheme(3)).into(),
        text: "".into(),
        advance_cursor: true,
    });
    editor.enter_frame();
    assert_eq!(editor.get_text(), "|  | b |\n|-|-|");
    editor.push(Event::Replace {
        region: (Grapheme(1), Grapheme(2)).into(),
        text: "".into(),
        advance_cursor: true,
    });
    editor.enter_frame();
    assert_eq!(editor.get_text(), "|  | b |\n|-|-|");
}

#[test]
fn deleting_whole_cells_empties_them() {
    let mut editor = TestEditor::new(SHEET);
    let at = |text: &str| SHEET.find(text).unwrap();
    select(&mut editor, at("dd") + 1, at("ee") + 1);
    delete(&mut editor, BACKSPACE);
    assert_eq!(
        editor.get_text(),
        "above\n\n| aa | bb | cc |\n| - | - | - |\n|  |  | ff |\n| gg | hh | ii |\n\nbelow"
    );
    assert_eq!(caret(&editor), at("dd"));
    editor.push(Event::Undo);
    editor.enter_frame();
    assert_eq!(editor.get_text(), SHEET);

    // typing over them empties them too, and goes in the first
    select(&mut editor, at("ee"), at("ff") + 1);
    type_text(&mut editor, "x");
    assert_eq!(
        editor.get_text(),
        "above\n\n| aa | bb | cc |\n| - | - | - |\n| dd | x |  |\n| gg | hh | ii |\n\nbelow"
    );
    assert_eq!(caret(&editor), at("ee") + 1);
}

#[test]
fn deleting_whole_rows_removes_them() {
    let at = |text: &str| SHEET.find(text).unwrap();
    for (from, to, gives) in [
        // body rows go
        (at("dd"), at("hh"), "above\n\n| aa | bb | cc |\n| - | - | - |\n\nbelow"),
        (
            at("| dd"),
            at("ff |") + 4,
            "above\n\n| aa | bb | cc |\n| - | - | - |\n| gg | hh | ii |\n\nbelow",
        ),
        // the header is emptied instead while rows remain under it
        (at("aa"), at("ee"), "above\n\n|  |  |  |\n| - | - | - |\n| gg | hh | ii |\n\nbelow"),
        // every row: the table goes
        (at("aa"), at("ii"), "above\n\n\n\nbelow"),
        // from above the table, the same; it keeps the line it starts on
        (at("ove"), at("ee"), "ab\n|  |  |  |\n| - | - | - |\n| gg | hh | ii |\n\nbelow"),
        (at("ove"), at("ii"), "ab\n\nbelow"),
        // and from a row to below it
        (at("hh"), at("low"), "above\n\n| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |\nlow"),
        (at("bb"), at("low"), "above\n\nlow"),
    ] {
        let mut editor = TestEditor::new(SHEET);
        select(&mut editor, from, to);
        delete(&mut editor, BACKSPACE);
        assert_eq!(editor.get_text(), gives, "deleting {from}..{to}");
        table_ranges_check_md(editor.get_text()).unwrap();
        editor.push(Event::Undo);
        editor.enter_frame();
        assert_eq!(editor.get_text(), SHEET, "undoing the deletion of {from}..{to}");
    }
}

#[test]
fn typing_over_whole_rows_empties_them() {
    let at = |text: &str| SHEET.find(text).unwrap();
    for (from, to, gives) in [
        (
            at("dd"),
            at("hh"),
            "above\n\n| aa | bb | cc |\n| - | - | - |\n| x |  |  |\n|  |  |  |\n\nbelow",
        ),
        (
            at("ove"),
            at("ee"),
            "abx\n|  |  |  |\n| - | - | - |\n|  |  |  |\n| gg | hh | ii |\n\nbelow",
        ),
        (
            at("hh"),
            at("low"),
            "above\n\n| aa | bb | cc |\n| - | - | - |\n| dd | ee | ff |\n| x |  |  |\nlow",
        ),
        // a table taken whole is text
        (at("aa"), at("ii"), "above\n\nx\n\nbelow"),
    ] {
        let mut editor = TestEditor::new(SHEET);
        select(&mut editor, from, to);
        type_text(&mut editor, "x");
        assert_eq!(editor.get_text(), gives, "typing over {from}..{to}");
        table_ranges_check_md(editor.get_text()).unwrap();
    }
}

// ─── keys ────────────────────────────────────────────────────────────

fn tab(editor: &mut TestEditor, shift: bool) {
    editor.push(Event::Indent { deindent: shift });
    editor.enter_frame();
}

fn enter(editor: &mut TestEditor, shift: bool) {
    editor.push(Event::Newline { shift });
    editor.enter_frame();
}

#[test]
fn tab_moves_through_cells_and_selects_them() {
    let md = "| aa | bb |\n| - | - |\n| cc |  |\n| dd |";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find("aa").unwrap() + 1);
    for takes in ["bb", "cc", ""] {
        tab(&mut editor, false);
        assert_eq!(selected(&editor), takes);
    }
    assert_eq!(caret_cell(&editor), Some((1, 1, 0)));
    tab(&mut editor, false);
    assert_eq!(selected(&editor), "dd");
    // into a cell the row lacks: it is written
    tab(&mut editor, false);
    assert_eq!(editor.get_text(), "| aa | bb |\n| - | - |\n| cc |  |\n| dd |  |");
    assert_eq!(caret_cell(&editor), Some((2, 1, 0)));
    // from the last cell: a new row
    tab(&mut editor, false);
    assert_eq!(editor.get_text(), "| aa | bb |\n| - | - |\n| cc |  |\n| dd |  |\n|  |  |");
    assert_eq!(caret_cell(&editor), Some((3, 0, 0)));

    // and back
    for (cell, takes) in [
        ((2, 1), ""),
        ((2, 0), "dd"),
        ((1, 1), ""),
        ((1, 0), "cc"),
        ((0, 1), "bb"),
        ((0, 0), "aa"),
        ((0, 0), "aa"),
    ] {
        tab(&mut editor, true);
        assert_eq!(selected(&editor), takes);
        let (_, end) = selection(&editor);
        let table = &editor.editor.edit.renderer.bounds.tables[0];
        assert_eq!(table.cell_at(Grapheme(end)), Some(cell));
    }
}

#[test]
fn tab_there_and_back_returns_the_caret() {
    let md = "| a | b | c |\n| - | - | - |\n| d | e | f |\n| g | h | i |";
    for start in ["a", "b", "e", "g"] {
        for n in 1..=8 {
            let mut editor = TestEditor::new(md);
            caret_at(&mut editor, md.find(start).unwrap());
            tab(&mut editor, false);
            tab(&mut editor, true);
            let home = selection(&editor);
            for _ in 0..n {
                tab(&mut editor, false);
            }
            if editor.get_text() != md {
                continue; // tabbed off the end: a row was added
            }
            for _ in 0..n {
                tab(&mut editor, true);
            }
            assert_eq!(selection(&editor), home, "{n} tabs from {start:?}");
        }
    }
}

#[test]
fn return_moves_down_and_extends_or_leaves() {
    let md = "| aa | bb |\n| - | - |\n| cc | dd |";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find("bb").unwrap());
    enter(&mut editor, false);
    assert_eq!(selected(&editor), "dd");
    // from the last row: a new row, same column
    enter(&mut editor, false);
    assert_eq!(editor.get_text(), "| aa | bb |\n| - | - |\n| cc | dd |\n|  |  |");
    assert_eq!(caret_cell(&editor), Some((2, 1, 0)));
    // shift goes up
    enter(&mut editor, true);
    assert_eq!(selected(&editor), "dd");
    enter(&mut editor, true);
    enter(&mut editor, true);
    assert_eq!(selected(&editor), "bb");
    // from a blank last row: the row goes and the caret leaves the table
    let blank_row = editor.get_text().len() - 2;
    caret_at(&mut editor, blank_row);
    enter(&mut editor, false);
    assert_eq!(editor.get_text(), "| aa | bb |\n| - | - |\n| cc | dd |\n\n");
    assert_eq!(caret(&editor), editor.get_text().len());
    type_text(&mut editor, "after");
    assert_eq!(editor.editor.edit.renderer.bounds.tables[0].rows.len(), 2);

    // with a blank line already below, the caret takes it
    let md = "> | aa |\n> | - |\n> |  |\n>\n> after";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find("  |").unwrap() + 1);
    enter(&mut editor, false);
    assert_eq!(editor.get_text(), "> | aa |\n> | - |\n> \n>\n> after");
    assert_eq!(caret(&editor), editor.get_text().find(">\n> after").unwrap() + 1);
}

#[test]
fn return_on_a_hand_typed_delimiter_row_starts_the_first_row() {
    let md = "| a | b |\n|-|";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.len());
    type_text(&mut editor, "-");
    enter(&mut editor, false);
    assert_eq!(editor.get_text(), "| a | b |\n|-|-\n|  |  |");
    assert_eq!(caret_cell(&editor), Some((1, 0, 0)));
}

#[test]
fn new_rows_keep_their_container() {
    for (md, gives) in [
        ("> | a |\n> | - |", "> | a |\n> | - |\n> |  |"),
        ("- | a |\n  | - |", "- | a |\n  | - |\n  |  |"),
        ("1. x\n\n   | a |\n   | - |", "1. x\n\n   | a |\n   | - |\n   |  |"),
        ("> - | a |\n>   | - |", "> - | a |\n>   | - |\n>   |  |"),
    ] {
        let mut editor = TestEditor::new(md);
        caret_at(&mut editor, md.find('a').unwrap());
        enter(&mut editor, false);
        assert_eq!(editor.get_text(), gives);
        table_ranges_check_md(editor.get_text()).unwrap();
        assert_eq!(caret_cell(&editor), Some((1, 0, 0)));
    }
}

#[test]
fn cut_and_copy_take_rows_without_their_container() {
    let md = "> | aa | bb |\n> | - | - |\n> | cc | dd |\n> | ee | ff |";
    // rows with their header copy as a table
    let mut editor = TestEditor::new(md);
    select(&mut editor, md.find("aa").unwrap(), md.find("cc").unwrap());
    editor.push(Event::Copy);
    editor.enter_frame();
    assert_eq!(editor.copied.as_deref(), Some("| aa | bb |\n| - | - |\n| cc | dd |"));
    assert_eq!(editor.get_text(), md);

    // with no selection, cut takes the caret's row
    caret_at(&mut editor, md.find("dd").unwrap());
    editor.push(Event::Cut);
    editor.enter_frame();
    assert_eq!(editor.copied.as_deref(), Some("| cc | dd |"));
    assert_eq!(editor.get_text(), "> | aa | bb |\n> | - | - |\n> | ee | ff |");

    // cutting the header empties it: the rows under it stay a table
    caret_at(&mut editor, md.find("aa").unwrap());
    editor.push(Event::Cut);
    editor.enter_frame();
    assert_eq!(editor.copied.as_deref(), Some("| aa | bb |"));
    assert_eq!(editor.get_text(), "> |  |  |\n> | - | - |\n> | ee | ff |");
}

// ─── structure ───────────────────────────────────────────────────────

fn act(editor: &mut TestEditor, action: TableAction) {
    editor.push(Event::Table(action));
    editor.enter_frame();
}

/// What the caret is in: its cell's text, and how far into it.
fn caret_in(editor: &TestEditor) -> Option<(String, usize)> {
    let (row, column, into) = caret_cell(editor)?;
    Some((cells(editor)[row][column].clone(), into))
}

#[test]
fn structure_edits() {
    use TableAction::*;
    use comrak::nodes::TableAlignment::{Center, Left, None as Unaligned, Right};
    let md = "| a | b | c |\n| - | :-: | --: |\n| d | e | f |\n| g | h | i |";
    for (cell, action, gives) in [
        (
            "e",
            InsertRow { below: true },
            "| a | b | c |\n| - | :-: | --: |\n| d | e | f |\n|  |  |  |\n| g | h | i |",
        ),
        (
            "e",
            InsertRow { below: false },
            "| a | b | c |\n| - | :-: | --: |\n|  |  |  |\n| d | e | f |\n| g | h | i |",
        ),
        (
            "h",
            InsertRow { below: true },
            "| a | b | c |\n| - | :-: | --: |\n| d | e | f |\n| g | h | i |\n|  |  |  |",
        ),
        (
            "b",
            InsertRow { below: true },
            "| a | b | c |\n| - | :-: | --: |\n|  |  |  |\n| d | e | f |\n| g | h | i |",
        ),
        // a row above the header is a new header
        (
            "b",
            InsertRow { below: false },
            "|  |  |  |\n| - | :-: | --: |\n| a | b | c |\n| d | e | f |\n| g | h | i |",
        ),
        ("e", DeleteRow, "| a | b | c |\n| - | :-: | --: |\n| g | h | i |"),
        ("h", DeleteRow, "| a | b | c |\n| - | :-: | --: |\n| d | e | f |"),
        ("b", DeleteRow, md),
        (
            "e",
            MoveRow { down: true },
            "| a | b | c |\n| - | :-: | --: |\n| g | h | i |\n| d | e | f |",
        ),
        (
            "h",
            MoveRow { down: false },
            "| a | b | c |\n| - | :-: | --: |\n| g | h | i |\n| d | e | f |",
        ),
        ("h", MoveRow { down: true }, md),
        // a row moved above the header trades cells with it; the header does not move down
        (
            "e",
            MoveRow { down: false },
            "| d | e | f |\n| - | :-: | --: |\n| a | b | c |\n| g | h | i |",
        ),
        ("b", MoveRow { down: true }, md),
        ("b", MoveRow { down: false }, md),
        (
            "e",
            InsertColumn { right: true },
            "| a | b |  | c |\n| - | :-: | --- | --: |\n| d | e |  | f |\n| g | h |  | i |",
        ),
        (
            "e",
            InsertColumn { right: false },
            "| a |  | b | c |\n| - | --- | :-: | --: |\n| d |  | e | f |\n| g |  | h | i |",
        ),
        (
            "d",
            InsertColumn { right: false },
            "|  | a | b | c |\n| --- | - | :-: | --: |\n|  | d | e | f |\n|  | g | h | i |",
        ),
        (
            "f",
            InsertColumn { right: true },
            "| a | b | c |  |\n| - | :-: | --: | --- |\n| d | e | f |  |\n| g | h | i |  |",
        ),
        ("e", DeleteColumn, "| a | c |\n| - | --: |\n| d | f |\n| g | i |"),
        ("d", DeleteColumn, "| b | c |\n| :-: | --: |\n| e | f |\n| h | i |"),
        ("f", DeleteColumn, "| a | b |\n| - | :-: |\n| d | e |\n| g | h |"),
        (
            "e",
            MoveColumn { right: true },
            "| a | c | b |\n| - | --: | :-: |\n| d | f | e |\n| g | i | h |",
        ),
        (
            "e",
            MoveColumn { right: false },
            "| b | a | c |\n| :-: | - | --: |\n| e | d | f |\n| h | g | i |",
        ),
        ("d", MoveColumn { right: false }, md),
        ("f", MoveColumn { right: true }, md),
        ("d", Align(Left), "| a | b | c |\n| :- | :-: | --: |\n| d | e | f |\n| g | h | i |"),
        ("e", Align(Right), "| a | b | c |\n| - | --: | --: |\n| d | e | f |\n| g | h | i |"),
        ("f", Align(Center), "| a | b | c |\n| - | :-: | :-: |\n| d | e | f |\n| g | h | i |"),
        ("e", Align(Unaligned), "| a | b | c |\n| - | --- | --: |\n| d | e | f |\n| g | h | i |"),
    ] {
        let mut editor = TestEditor::new(md);
        caret_at(&mut editor, md.find(cell).unwrap() + 1);
        act(&mut editor, action);
        assert_eq!(editor.get_text(), gives, "{action:?} at {cell:?}");
        table_ranges_check_md(editor.get_text()).unwrap();
        // the caret is where it was in its cell, if the cell is still there
        let gone = matches!(action, DeleteRow | DeleteColumn) && editor.get_text() != md;
        if !gone {
            assert_eq!(caret_in(&editor), Some((cell.to_string(), 1)), "{action:?} at {cell:?}");
        }
        // one undo unit
        editor.push(Event::Undo);
        editor.enter_frame();
        assert_eq!(editor.get_text(), md, "undoing {action:?} at {cell:?}");
    }
}

#[test]
fn deleting_the_last_column_deletes_the_table() {
    let md = "above\n\n| a |\n| - |\n| b |\n\nbelow";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find("b |").unwrap());
    act(&mut editor, TableAction::DeleteColumn);
    assert_eq!(editor.get_text(), "above\n\n\n\nbelow");
}

#[test]
fn structure_edits_round_trip() {
    use TableAction::*;
    enum Step {
        Edit(TableAction),
        Return { shift: bool },
        Tab { shift: bool },
    }
    use Step::*;
    // an edit, then its inverse: the bytes come back. A new row or column is
    // deleted from within it.
    let round_trips = [
        vec![Edit(InsertRow { below: true }), Return { shift: false }, Edit(DeleteRow)],
        vec![Edit(InsertRow { below: false }), Return { shift: true }, Edit(DeleteRow)],
        vec![Edit(InsertColumn { right: true }), Tab { shift: false }, Edit(DeleteColumn)],
        vec![Edit(InsertColumn { right: false }), Tab { shift: true }, Edit(DeleteColumn)],
        vec![Edit(MoveRow { down: true }), Edit(MoveRow { down: false })],
        vec![Edit(MoveRow { down: false }), Edit(MoveRow { down: true })],
        vec![Edit(MoveColumn { right: true }), Edit(MoveColumn { right: false })],
        vec![Edit(MoveColumn { right: false }), Edit(MoveColumn { right: true })],
    ];
    let tables = [
        "| a | b | c |\n| - | - | - |\n| d | e | f |\n| g | h | i |",
        "|a|b|c|\n|-|-|-|\n|d|e|f|\n|g|h|i|",
        "> |  a  | b |   c |\n> |:--|--:|:-:|\n> | d | \\| | `f` |\n> | **g** |  | i |",
        "- | a | b | c |\n  | - | - | - |\n  | d | e | f |\n  | g | h | i |",
    ];
    for md in tables {
        for (i, steps) in round_trips.iter().enumerate() {
            for cell in 0..9 {
                // the header neither moves nor is deleted: no inverse there
                let on_header =
                    cell < 3 || (cell < 6 && matches!(steps[0], Edit(MoveRow { down: false })));
                if on_header
                    && matches!(steps[0], Edit(MoveRow { .. } | InsertRow { below: false }))
                {
                    continue;
                }
                let mut editor = TestEditor::new(md);
                let content = editor.editor.edit.renderer.bounds.tables[0].rows[cell / 3].cells
                    [cell % 3]
                    .content;
                caret_at(&mut editor, content.0.0);
                for (n, step) in steps.iter().enumerate() {
                    match step {
                        Edit(action) => act(&mut editor, *action),
                        Return { shift } => enter(&mut editor, *shift),
                        Tab { shift } => tab(&mut editor, *shift),
                    }
                    // at the table's edge a move is no edit, and has no inverse
                    if n == 0 && editor.get_text() == md {
                        let moves = matches!(step, Edit(MoveRow { .. } | MoveColumn { .. }));
                        assert!(moves, "round trip {i} from cell {cell}: a step did nothing");
                        break;
                    }
                }
                assert_eq!(editor.get_text(), md, "round trip {i} from cell {cell}");
            }
        }
    }
}

#[test]
fn structure_edits_leave_the_rest_alone() {
    use TableAction::*;
    // ragged rows, missing pipes, and containers: whatever the edit, the table is
    // then rectangular, every other cell reads as it did, and nothing around it moved
    let tables = [
        "| a | b | c |\n| - | - | - |\n| d |\n| g | h | i | extra |",
        "a | b | c\n--|--|--\nd | e\ng | h | i",
        "|a|b|c|\n|-|-|-|\n||e|\n|g|h|i|",
        "> | a | b | c |\n> | - | - | - |\n> | d | e\n> | 'g' | \\| | `i` |",
    ];
    let actions = [
        InsertRow { below: true },
        InsertRow { below: false },
        DeleteRow,
        MoveRow { down: true },
        MoveRow { down: false },
        InsertColumn { right: true },
        InsertColumn { right: false },
        DeleteColumn,
        MoveColumn { right: true },
        MoveColumn { right: false },
    ];
    let sorted = |mut texts: Vec<String>| {
        texts.retain(|text| !text.is_empty());
        texts.sort();
        texts
    };
    for table in tables {
        let md = format!("above\n\n{table}\n\nbelow");
        for action in actions {
            let mut starts: Vec<usize> = Vec::new();
            {
                let editor = TestEditor::new(&md);
                let rows = &editor.editor.edit.renderer.bounds.tables[0].rows;
                starts.extend(
                    rows.iter()
                        .flat_map(|row| row.cells.iter().map(|c| c.content.0.0)),
                );
            }
            for start in starts {
                let mut editor = TestEditor::new(&md);
                caret_at(&mut editor, start);
                let before = cells(&editor);
                act(&mut editor, action);
                let text = editor.get_text().to_string();
                let context = format!("{action:?} at {start} of\n{md}\ngave\n{text}");
                assert!(text.starts_with("above\n\n") && text.ends_with("\n\nbelow"), "{context}");
                table_ranges_check_md(&text).unwrap_or_else(|reason| panic!("{reason}\n{context}"));

                let after = cells(&editor);
                let column_edit =
                    matches!(action, InsertColumn { .. } | DeleteColumn | MoveColumn { .. });
                if column_edit && text != md {
                    let width = after[0].len();
                    assert!(after.iter().all(|row| row.len() == width), "ragged: {context}");
                }
                // cells keep their text: all of them, less a deleted row or column's
                let (before, after) = (sorted(before.concat()), sorted(after.concat()));
                if matches!(action, DeleteRow | DeleteColumn) {
                    assert!(after.iter().all(|text| before.contains(text)), "{context}");
                } else {
                    assert_eq!(after, before, "{context}");
                }
            }
        }
    }
}

#[test]
fn insert_table() {
    for (md, at, gives, lands) in [
        ("", 0, "|  |  |  |\n| --- | --- | --- |\n|  |  |  |", 2),
        // after a line of text, with a blank line between
        ("text", 2, "text\n\n|  |  |  |\n| --- | --- | --- |\n|  |  |  |", 8),
        // on a blank line under text, on the line after
        ("text\n", 5, "text\n\n|  |  |  |\n| --- | --- | --- |\n|  |  |  |", 8),
        ("text\n\n", 6, "text\n\n|  |  |  |\n| --- | --- | --- |\n|  |  |  |", 8),
        // and a blank line before what follows
        ("text\nmore", 2, "text\n\n|  |  |  |\n| --- | --- | --- |\n|  |  |  |\n\nmore", 8),
        ("\nmore", 0, "|  |  |  |\n| --- | --- | --- |\n|  |  |  |\n\nmore", 2),
        // inside a container
        ("> text", 4, "> text\n> \n> |  |  |  |\n> | --- | --- | --- |\n> |  |  |  |", 14),
        ("- text", 4, "- text\n  \n  |  |  |  |\n  | --- | --- | --- |\n  |  |  |  |", 14),
    ] {
        let mut editor = TestEditor::new(md);
        caret_at(&mut editor, at);
        act(&mut editor, TableAction::Insert);
        assert_eq!(editor.get_text(), gives, "inserting a table at {at} of {md:?}");
        assert_eq!(caret(&editor), lands, "inserting a table at {at} of {md:?}");
        assert_eq!(table_ranges_check_md(editor.get_text()), Ok(1));
        assert_eq!(caret_cell(&editor), Some((0, 0, 0)));
        editor.push(Event::Undo);
        editor.enter_frame();
        assert_eq!(editor.get_text(), md);
    }
}

// ─── layout ──────────────────────────────────────────────────────────

/// The rect of each cell of the document's first table, row by row.
fn cell_rects(editor: &TestEditor) -> Vec<Vec<egui::Rect>> {
    let r = &editor.editor.edit.renderer;
    let table = &r.bounds.tables[0];
    table
        .rows
        .iter()
        .map(|row| {
            let mut boxes: Vec<_> = r
                .table_cell_boxes
                .iter()
                .filter(|cell_box| cell_box.row == row.line.start())
                .collect();
            boxes.sort_by_key(|cell_box| cell_box.column);
            boxes.iter().map(|cell_box| cell_box.rect).collect()
        })
        .collect()
}

/// The rects of what is painted for the text `needle`.
fn rects_of(editor: &TestEditor, needle: &str) -> Vec<egui::Rect> {
    let start = editor.get_text().find(needle).unwrap();
    let range = (Grapheme(start), Grapheme(start + needle.chars().count()));
    editor.editor.edit.renderer.range_rects(range)
}

#[test]
fn columns_fit_their_content() {
    let md = "| a | a much longer cell |\n| - | - |\n| bb | c |";
    let editor = TestEditor::new(md);
    let rects = cell_rects(&editor);
    // a column is as wide in every row, and the wider cell's is the wider
    assert_eq!(rects[0][0].x_range(), rects[1][0].x_range());
    assert_eq!(rects[0][1].x_range(), rects[1][1].x_range());
    assert!(rects[0][1].width() > 2.0 * rects[0][0].width());
    // snug: each column holds its widest cell on one row, without room for two
    for (text, column) in [("bb", 0), ("a much longer cell", 1)] {
        let text = rects_of(&editor, text);
        assert_eq!(text.len(), 1, "{text:?} wrapped");
        let spare = rects[0][column].width() - text[0].width();
        assert!((0.0..2.0 * text[0].width()).contains(&spare), "{spare} to spare around {text:?}");
    }
    // so the table is narrower than the note
    assert!(rects[0][1].right() < 400.0);
}

#[test]
fn cells_wrap_before_the_table_outgrows_the_note() {
    let long = "word ".repeat(60);
    let md = format!("| a | {long}|\n| - | - |\n| b | c |");
    let editor = TestEditor::new(&md);
    let rects = cell_rects(&editor);
    let note = editor.editor.edit.renderer.width;
    let table = rects[0][0].left()..=rects[0][1].right();
    assert!(table.end() - table.start() <= note + 0.5, "table wider than the note's {note}");
    assert!(table.end() - table.start() > note - 1.0, "table narrower than it need be");
    // the long cell wrapped; the short column kept its text on a row
    assert!(rects[0][0].height() > 3.0 * rects[1][0].height());
    assert_eq!(rects_of(&editor, "b |").len(), 1);
}

#[test]
fn nothing_overflows_a_cell_too_narrow_for_its_words() {
    let wide = "unbreakable".repeat(8);
    let md = format!("| {wide} | {wide} | {wide} |\n| - | - | - |\n| a | b | c |");
    let editor = TestEditor::new(&md);
    let r = &editor.editor.edit.renderer;
    let rects = cell_rects(&editor);
    assert!(rects[0][2].right() <= r.width + rects[0][0].left() + 0.5);
    let header = &r.bounds.tables[0].rows[0];
    for (cell, rect) in header.cells.iter().zip(&rects[0]) {
        for painted in r.range_rects(cell.content) {
            assert!(
                rect.expand(0.5).contains_rect(painted),
                "{painted:?} outside its cell {rect:?}"
            );
        }
    }
}

#[test]
fn cells_align_as_the_delimiter_row_says() {
    let md = "| left | center | right | none |\n| :-- | :-: | --: | --- |\n| a much longer cell | a much longer cell | a much longer cell | a much longer cell |\n|  |  |  |  |";
    let mut editor = TestEditor::new(md);
    let rects = cell_rects(&editor);
    let padding = editor.editor.edit.renderer.layout.block_padding;
    let gaps = |editor: &TestEditor, text: &str, column: usize| {
        let text = rects_of(editor, text)[0];
        let cell = rects[0][column].shrink(padding);
        (text.left() - cell.left(), cell.right() - text.right())
    };
    let (left, right) = gaps(&editor, "left", 0);
    assert!(left.abs() < 0.5 && right > 20.0);
    let (left, right) = gaps(&editor, "center", 1);
    assert!((left - right).abs() < 1.0 && left > 10.0);
    let (left, right) = gaps(&editor, "right", 2);
    assert!(right.abs() < 0.5 && left > 20.0);
    let (left, right) = gaps(&editor, "none", 3);
    assert!(left.abs() < 0.5 && right > 20.0);

    // a blank cell's caret sits where its text would start
    let blank_row = &editor.editor.edit.renderer.bounds.tables[0].rows[2].clone();
    for (column, side) in [(0, 0.0), (1, 0.5), (2, 1.0), (3, 0.0)] {
        caret_at(&mut editor, blank_row.cells[column].content.0.0);
        let caret = editor
            .editor
            .edit
            .cursor_line(blank_row.cells[column].content.0)
            .unwrap()[0]
            .x;
        let cell = rects[2][column].shrink(padding);
        let expected = cell.left() + cell.width() * side;
        assert!(
            (caret - expected).abs() < 1.0,
            "column {column}: caret at {caret}, not {expected}"
        );
    }
}

/// #5071
#[test]
fn an_image_is_clamped_to_its_cell() {
    use super::harness::build_lb;
    let wide = "| a | b | c | d |\n| - | - | - | - |\n| ![big](big.png) | ![sized\\|900x300](big.png) | text text text text text text text text text text text text text text text text text | x |";
    let (mut editor, embeds) = TestEditor::with_test_embeds(build_lb(), wide);
    embeds.complete("big.png", egui::Vec2::new(4000.0, 1000.0));
    editor.enter_frame();
    editor.enter_frame();
    let r = &editor.editor.edit.renderer;
    let rects = cell_rects(&editor);
    let padding = r.layout.block_padding;
    let images: Vec<_> = r
        .fragments
        .iter()
        .filter(|fragment| {
            matches!(
                fragment.content,
                crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent::Embed { .. }
            )
        })
        .collect();
    assert_eq!(images.len(), 2);
    for (image, cell) in images.iter().zip(&rects[1]) {
        assert!(
            cell.shrink(padding).expand(0.5).contains_rect(image.rect),
            "image {:?} outside its cell {cell:?}",
            image.rect
        );
    }
    assert!(rects[1][3].right() <= r.width + rects[1][0].left() + 0.5);
}

// ─── toolbar ─────────────────────────────────────────────────────────

fn tools(editor: &TestEditor) -> Vec<TableTool> {
    let groups = editor.editor.table_tools();
    groups.into_iter().flatten().flatten().collect()
}

fn tools_where(editor: &TestEditor, pick: impl Fn(&TableTool) -> bool) -> Vec<ToolbarAction> {
    let picked = tools(editor).into_iter().filter(pick);
    picked.map(|tool| tool.action).collect()
}

fn use_tool(editor: &mut TestEditor, action: ToolbarAction) {
    let event = editor.editor.toolbar_action(action);
    editor.push(event);
    editor.enter_frame();
}

#[test]
fn the_toolbar_offers_the_table_set_in_a_table() {
    use ToolbarAction::*;
    let md = "p\n\n| a | b |\n| :-: | - |\n| c | d |\n| e | f |\n";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, 0);
    assert_eq!(editor.editor.table_tools(), None);

    // the header stays the header, and its first cell is at an edge
    caret_at(&mut editor, md.find('a').unwrap());
    assert_eq!(
        tools_where(&editor, |tool| !tool.enabled),
        [DeleteRow, MoveRowUp, MoveRowDown, MoveColumnLeft]
    );
    assert_eq!(tools_where(&editor, |tool| tool.applied), [AlignCenter]);

    caret_at(&mut editor, md.find('f').unwrap());
    assert_eq!(tools_where(&editor, |tool| !tool.enabled), [MoveRowDown, MoveColumnRight]);
    assert_eq!(tools_where(&editor, |tool| tool.applied), []);

    // a touch keyboard has no Tab
    assert_eq!(tools(&editor)[0].action, RowAbove);
    editor.editor.edit.renderer.touch_mode = true;
    assert_eq!(tools_where(&editor, |_| true)[..3], [PreviousCell, NextCell, NextRow]);

    let state = editor.editor.android_toolbar_state();
    assert!(state.in_table);
    assert_eq!(state.disabled, [MoveRowDown, MoveColumnRight]);
}

/// A tool is enabled exactly where using it changes something.
#[test]
fn a_table_tool_is_enabled_where_it_does_something() {
    let md = "| a | b |\n| - | - |\n| c | d |\n| e | f |\n";
    for cell in ['a', 'b', 'c', 'd', 'e', 'f'] {
        let mut editor = TestEditor::new(md);
        editor.editor.edit.renderer.touch_mode = true;
        caret_at(&mut editor, md.find(cell).unwrap());
        for tool in tools(&editor) {
            let mut editor = TestEditor::new(md);
            caret_at(&mut editor, md.find(cell).unwrap());
            let before = selection(&editor);
            use_tool(&mut editor, tool.action);
            let changed = editor.get_text() != md || selection(&editor) != before;
            assert_eq!(changed, tool.enabled, "{:?} in {cell}", tool.action);
        }
    }
}

#[test]
fn an_alignment_tool_sets_its_alignment_or_takes_it_away() {
    use ToolbarAction::*;
    let md = "| a | b |\n| --- | --- |\n| c | d |\n";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find('d').unwrap());
    use_tool(&mut editor, AlignRight);
    assert_eq!(editor.get_text(), "| a | b |\n| --- | --: |\n| c | d |\n");
    assert_eq!(tools_where(&editor, |tool| tool.applied), [AlignRight]);
    use_tool(&mut editor, AlignCenter);
    assert_eq!(editor.get_text(), "| a | b |\n| --- | :-: |\n| c | d |\n");
    use_tool(&mut editor, AlignCenter);
    assert_eq!(editor.get_text(), md);
    assert_eq!(tools_where(&editor, |tool| tool.applied), []);
}

#[test]
fn the_table_tool_inserts_a_table() {
    let mut editor = TestEditor::new("text");
    caret_at(&mut editor, 4);
    use_tool(&mut editor, ToolbarAction::Table);
    assert_eq!(editor.get_text(), "text\n\n|  |  |  |\n| --- | --- | --- |\n|  |  |  |");
    assert_eq!(caret_cell(&editor), Some((0, 0, 0)));
    assert!(editor.editor.table_tools().is_some());
}

// ─── block styles ────────────────────────────────────────────────────

#[test]
fn a_block_style_takes_the_whole_table() {
    use comrak::nodes::{ListType, NodeHeading, NodeList};
    let toggle = |editor: &mut TestEditor, style: NodeValue| {
        editor.push(Event::ToggleStyle { region: Region::Selection, style });
        editor.enter_frame();
    };
    let bullets =
        || NodeValue::List(NodeList { list_type: ListType::Bullet, ..Default::default() });
    let md = "| a | b |\n| - | - |\n| c | d |\n| e | f |\n";
    let listed = "* | a | b |\n  | - | - |\n  | c | d |\n  | e | f |\n";

    // from a caret, from cells of a row, and from rows
    let (c, d, f) = (md.find('c').unwrap(), md.find('d').unwrap(), md.find('f').unwrap());
    for (from, to) in [(c, c), (c, d + 1), (c, f + 1)] {
        let mut editor = TestEditor::new(md);
        select(&mut editor, from, to);
        toggle(&mut editor, bullets());
        assert_eq!(editor.get_text(), listed, "{from}..{to}");
        assert_eq!(table_ranges_check_md(listed), Ok(1));
        toggle(&mut editor, bullets());
        assert_eq!(editor.get_text(), md, "{from}..{to}");
    }

    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, c);
    toggle(&mut editor, NodeValue::BlockQuote);
    assert_eq!(editor.get_text(), "> | a | b |\n> | - | - |\n> | c | d |\n> | e | f |\n");
    assert_eq!(caret_in(&editor), Some(("c".into(), 0)));

    // a cell holds no heading
    let quoted = editor.get_text().to_string();
    toggle(&mut editor, NodeValue::Heading(NodeHeading { level: 1, ..Default::default() }));
    assert_eq!(editor.get_text(), quoted);
}

// ─── handles and drags ───────────────────────────────────────────────

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    let (button, modifiers) = (egui::PointerButton::Primary, Default::default());
    egui::Event::PointerButton { pos, button, pressed, modifiers }
}

fn click(editor: &mut TestEditor, pos: egui::Pos2) {
    editor.enter_frame_with_input(vec![egui::Event::PointerMoved(pos)]);
    editor.enter_frame_with_input(vec![button(pos, true)]);
    editor.enter_frame_with_input(vec![button(pos, false)]);
    editor.enter_frame();
}

/// Presses at `from`, moves to `to` over a few frames, and lets go.
fn drag(editor: &mut TestEditor, from: egui::Pos2, to: egui::Pos2) {
    editor.enter_frame_with_input(vec![egui::Event::PointerMoved(from)]);
    editor.enter_frame_with_input(vec![button(from, true)]);
    for step in 1..=4 {
        let pos = from + (to - from) * (step as f32 / 4.0);
        editor.enter_frame_with_input(vec![egui::Event::PointerMoved(pos)]);
    }
    editor.enter_frame_with_input(vec![button(to, false)]);
    editor.enter_frame();
    editor.enter_frame();
}

/// The rect of each row of the table, header first.
fn row_rects(editor: &TestEditor) -> Vec<egui::Rect> {
    let rows = cell_rects(editor);
    let union = |row: &Vec<egui::Rect>| row.iter().copied().reduce(egui::Rect::union).unwrap();
    rows.iter().map(union).collect()
}

const DRAGGED: &str =
    "| a | bb | ccc |\n| - | :-: | --: |\n| d | e |\n| g | h | i |\n| j | k | l |";

#[test]
fn dragging_a_row_handle_reorders_the_body_rows() {
    let mut editor = TestEditor::new(DRAGGED);
    let down = egui::Vec2::new(0.0, 4.0);

    // a row's handle is on the table's left border; past the last row
    let rows = row_rects(&editor);
    drag(&mut editor, rows[1].left_center(), rows[3].left_bottom() + down);
    let moved = "| a | bb | ccc |\n| - | :-: | --: |\n| g | h | i |\n| j | k | l |\n| d | e |";
    assert_eq!(editor.get_text(), moved);

    // back to above the first body row
    let rows = row_rects(&editor);
    drag(&mut editor, rows[3].left_center(), rows[1].left_top() + down);
    assert_eq!(editor.get_text(), DRAGGED);

    // dropped on its own slot, a row stays; the header's handle drags nothing
    let rows = row_rects(&editor);
    drag(&mut editor, rows[2].left_center(), rows[2].left_center() + 2.0 * down);
    drag(&mut editor, rows[0].left_center(), rows[2].left_center());
    assert_eq!(editor.get_text(), DRAGGED);
}

#[test]
fn dragging_a_column_handle_reorders_the_columns() {
    let mut editor = TestEditor::new(DRAGGED);
    let along = egui::Vec2::new(2.0, 0.0);

    // a column's handle is on the table's top border; a ragged row is padded
    let header = cell_rects(&editor).swap_remove(0);
    drag(&mut editor, header[0].center_top(), header[2].center_top() + along);
    let moved = "| bb | ccc | a |\n| :-: | --: | - |\n| e |  | d |\n| h | i | g |\n| k | l | j |";
    assert_eq!(editor.get_text(), moved);
    table_ranges_check_md(moved).unwrap();

    let header = cell_rects(&editor).swap_remove(0);
    drag(&mut editor, header[2].center_top(), header[0].center_top() - along);
    let back = "| a | bb | ccc |\n| - | :-: | --: |\n| d | e |  |\n| g | h | i |\n| j | k | l |";
    assert_eq!(editor.get_text(), back);

    // dropped on its own slot, a column stays
    let header = cell_rects(&editor).swap_remove(0);
    drag(&mut editor, header[1].center_top(), header[1].center_top() + 4.0 * along);
    assert_eq!(editor.get_text(), back);
}

#[test]
fn clicking_a_handle_selects_its_row_or_column() {
    let mut editor = TestEditor::new(DRAGGED);
    let (rows, cells) = (row_rects(&editor), cell_rects(&editor));
    let table = editor.editor.edit.renderer.bounds.tables[0].clone();

    click(&mut editor, rows[3].left_center());
    assert_eq!(selection(&editor), (table.rows[3].line.0.0, table.rows[3].line.1.0));
    // a click off the menu the handle opened puts it away
    click(&mut editor, rows[1].center());

    click(&mut editor, cells[0][1].center_top());
    assert_eq!(selected(&editor), "bb");
    click(&mut editor, rows[1].center());

    // a table that can't be edited has no handles: a click there is a click
    editor.editor.edit.renderer.readonly = true;
    click(&mut editor, rows[3].left_center());
    let (start, end) = selection(&editor);
    assert_eq!(start, end);
}

/// The platform's own long press, as iOS delivers it.
#[test]
fn a_long_press_lifts_a_body_row_or_a_header_cells_column() {
    let mut editor = TestEditor::new(DRAGGED);
    editor.editor.edit.renderer.touch_mode = true;
    editor.enter_frame();
    let lift = |editor: &mut TestEditor, from: egui::Pos2, to: egui::Pos2| {
        assert!(editor.editor.edit.reorder_can_start(from));
        assert!(editor.editor.edit.reorder_start(from));
        editor.enter_frame();
        editor.editor.edit.reorder_move(to);
        editor.enter_frame();
        editor.editor.edit.reorder_end(to, false);
        editor.enter_frame();
        editor.enter_frame();
    };

    // a header cell takes its column along the row
    let cells = cell_rects(&editor);
    lift(&mut editor, cells[0][1].center(), cells[0][0].center() - egui::Vec2::new(2.0, 0.0));
    let moved = "| bb | a | ccc |\n| :-: | - | --: |\n| e | d |  |\n| h | g | i |\n| k | j | l |";
    assert_eq!(editor.get_text(), moved);

    // a body row moves among the body rows
    let cells = cell_rects(&editor);
    lift(&mut editor, cells[3][0].center(), cells[1][0].center_top());
    let moved = "| bb | a | ccc |\n| :-: | - | --: |\n| k | j | l |\n| e | d |  |\n| h | g | i |";
    assert_eq!(editor.get_text(), moved);

    // one of a kind has nowhere to go
    let mut editor = TestEditor::new("| a |\n| - |\n| b |");
    editor.editor.edit.renderer.touch_mode = true;
    editor.enter_frame();
    for cell in cell_rects(&editor).concat() {
        assert!(!editor.editor.edit.reorder_can_start(cell.center()));
    }
}
