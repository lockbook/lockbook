//! Table tests: the cell range invariant over a fixed corpus (the property
//! tables run it over generated tables), and exact-behavior examples.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::super::MdRender;
use super::super::input::{Advance, Bound, Event, Increment, Location, Region};
use super::super::table::{DelimiterBounds, RowBounds};
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
