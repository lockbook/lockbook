//! The table cell range invariant, which the property tests state over
//! generated tables, and the examples that only a test would catch:
//! shapes dogfooding does not reach, where a break is silent.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::super::MdRender;
use super::super::input::{Event, Location, Region};
use super::super::table::{DelimiterBounds, RowBounds, TableAction};
use super::harness::TestEditor;
use super::render_props::test_renderer;

/// What the parser trims from either side of a cell.
fn is_cell_space(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// What its row scanner skips after a pipe, and pads the delimiter row with.
fn is_pipe_space(c: char) -> bool {
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
            let before = &r.buffer[(row.cells[i - 1].span.0, at)];
            let backslashes = before.len() - before.trim_end_matches('\\').len();
            let escaped = !backslashes.is_multiple_of(2);
            ensure!(!escaped, "the pipe before cell {i} at {at:?} is escaped");
            at += 1;
        }
        let (span, content) = (cell.span, cell.content);
        ensure!(span.0 == at, "cell {i} spans {span:?} but the row is tiled up to {at:?}");
        ensure!(span.0 <= span.1, "cell {i} span {span:?} runs backwards");
        ensure!(span.1 <= line.end(), "cell {i} span {span:?} leaves its line {line:?}");
        at = span.1;

        let text = &r.buffer[span];
        ensure!(!text.contains(['\n', '\r']), "cell {i} spans a line break");
        let opened = i > 0 || row.leading_pipe;
        let trimmed = text.trim_start_matches(if opened { is_pipe_space } else { is_cell_space });
        let trimmed = trimmed.trim_end_matches(is_cell_space);
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
        ensure!(rest.chars().all(is_pipe_space), "short row has more to parse: {rest:?}");
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
    let leading_pipe = char_at(r, at) == "|";
    at += leading_pipe as usize;
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

        let text = r.buffer[*cell].trim_matches(is_pipe_space);
        let dashes = text.trim_start_matches(':').trim_end_matches(':');
        ensure!(
            !dashes.is_empty() && dashes.chars().all(|c| c == '-'),
            "delimiter cell {i} is {text:?}"
        );
        // the parser reads the colons of a cell trimmed as any cell is
        let opened = i > 0 || leading_pipe;
        let read = r.buffer[*cell].trim_end_matches(is_cell_space);
        let read = read.trim_start_matches(if opened { is_pipe_space } else { is_cell_space });
        let written = match (read.starts_with(':'), read.ends_with(':')) {
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
    ensure!(rest.chars().all(is_pipe_space), "delimiter row has more to lex: {rest:?}");
    Ok(())
}

// ═════════════════════════ exact-behavior examples ═════════════════════════

// ═════════════════════════ examples ═════════════════════════

/// Byte offsets, which are grapheme offsets in the ASCII documents here.
fn caret_at(editor: &mut TestEditor, offset: usize) {
    editor.editor.edit.cursor.x_target = None;
    editor.push(Event::Select { region: Region::Location(Location::Grapheme(Grapheme(offset))) });
    editor.enter_frame();
}

fn type_text(editor: &mut TestEditor, text: &str) {
    editor.push(Event::Replace {
        region: Region::Selection,
        text: text.into(),
        advance_cursor: true,
    });
    editor.enter_frame();
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

fn act(editor: &mut TestEditor, action: TableAction) {
    editor.push(Event::Table(action));
    editor.enter_frame();
}

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

/// A table of more columns than a note has room for.
fn wide() -> String {
    let row = |cell: &dyn Fn(usize) -> String| {
        let cells: Vec<String> = (0..24).map(cell).collect();
        format!("| {} |", cells.join(" | "))
    };
    let header = row(&|column| format!("column{column:02}"));
    let delimiter = row(&|_| "-".into());
    let body = row(&|column| format!("{column},204,000"));
    format!("{header}\n{delimiter}\n{body}\n{body}")
}

/// The columns of the first table that show, and the span they show in.
fn showing(editor: &TestEditor) -> (Vec<usize>, egui::Rangef) {
    let r = &editor.editor.edit.renderer;
    let header = r.bounds.tables[0].rows[0].line.0;
    let boxes = r.table_cell_boxes.iter();
    let boxes: Vec<_> = boxes.filter(|cell_box| cell_box.row == header).collect();
    let span = boxes
        .iter()
        .map(|cell_box| cell_box.rect)
        .reduce(egui::Rect::union);
    (boxes.iter().map(|cell_box| cell_box.column).collect(), span.unwrap().x_range())
}

fn paste(editor: &mut TestEditor, text: &str) {
    let event = editor.editor.edit.paste_event(text.into());
    editor.push(event.unwrap());
    editor.enter_frame();
}

/// A paste after "Paste as table" was chosen.
fn paste_as_table(editor: &mut TestEditor, text: &str) {
    let edit = &editor.editor.edit;
    let now = edit.renderer.ctx.input(|i| i.time);
    edit.paste_as_table.set(Some(now));
    paste(editor, text);
}

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

#[test]
fn paste_as_table_reads_comma_separated_text() {
    for (csv, gives) in [
        ("a,b\nc,d", "| a | b |\n| --- | --- |\n| c | d |"),
        // quoted fields hold commas, quotes, and line breaks
        (
            "name,note\n\"Smith, J\",\"said \"\"hi\"\"\"\nx,\"two\nlines\"\n",
            "| name | note |\n| --- | --- |\n| Smith, J | said \"hi\" |\n| x | two lines |",
        ),
        // a pipe is escaped; a short row is padded; a quote mid-field is text
        ("a|b,c\nd", "| a\\|b | c |\n| --- | --- |\n| d |  |"),
        ("5\" nail,x", "| 5\" nail | x |\n| --- | --- |"),
        ("a\tb,c\nd\te", "| a | b,c |\n| --- | --- |\n| d | e |"),
    ] {
        let mut editor = TestEditor::new("");
        paste_as_table(&mut editor, csv);
        assert_eq!(editor.get_text(), gives, "{csv:?}");
        table_ranges_check_md(gives).unwrap();
    }

    // the next paste is an ordinary one
    let mut editor = TestEditor::new("x");
    caret_at(&mut editor, 1);
    paste_as_table(&mut editor, "a,b");
    caret_at(&mut editor, 0);
    paste(&mut editor, "c,d");
    assert_eq!(editor.get_text(), "c,dx\n\n| a | b |\n| --- | --- |");
}

#[test]
fn keystrokes_of_one_frame_land_in_the_order_typed() {
    let mut editor = TestEditor::new("| a | x |\n|-|-|");
    caret_at(&mut editor, 3);
    for text in [" ", "b"] {
        let (region, text) = (Region::Selection, text.to_string());
        editor.push(Event::Replace { region, text, advance_cursor: true });
    }
    editor.enter_frame();
    assert_eq!(editor.get_text(), "| a b | x |\n|-|-|");
}

#[test]
fn a_pipe_written_in_a_cell_shows_as_a_pipe() {
    // two that flank text, which the parser pairs up; and one in a code span
    let md = "| fast\\|slow\\|auto | x |\n| - | - |\n| `p\\|q` | y |\n\na |b| c";
    let mut editor = TestEditor::new(md);
    caret_at(&mut editor, md.find('y').unwrap());
    let r = &editor.editor.edit.renderer;
    let painted = |at: usize| !r.range_rects((Grapheme(at), Grapheme(at + 1))).is_empty();
    let escapes: Vec<usize> = md.match_indices("\\|").map(|(at, _)| at).collect();
    assert_eq!(escapes.len(), 3);
    for escape in escapes {
        assert!(!painted(escape), "the backslash at {escape} shows");
        assert!(painted(escape + 1), "the pipe at {} is hidden", escape + 1);
    }
    // a pair of pipes in a paragraph is text too
    let pair = md.find("|b|").unwrap();
    assert!(painted(pair) && painted(pair + 2));
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

#[test]
fn a_find_match_in_a_column_out_of_view_brings_it_in() {
    let md = wide();
    let mut editor = TestEditor::new(&md);
    caret_at(&mut editor, md.find("column00").unwrap());
    editor.editor.find.open_requested = true;
    editor.enter_frame();
    editor.editor.find.term = Some("column23".into());
    editor.enter_frame_with_input(vec![]);
    editor.editor.find.current_match = Some(0);
    for _ in 0..3 {
        editor.enter_frame_with_input(vec![]);
    }
    let found = md.find("column23").unwrap();
    let r = &editor.editor.edit.renderer;
    assert!(showing(&editor).0.contains(&23));
    assert!(
        !r.range_rects((Grapheme(found), Grapheme(found + 8)))
            .is_empty()
    );
}

#[test]
fn a_table_the_parser_places_oddly_shows_as_its_source() {
    // a header on a lazily continued line: its cells are not where it says
    for md in ["- text\n a | b\n  -|-\n  c | d", ">\t- x\n>\ta | b\n>\t  -|-\n>\t  c | d"] {
        let mut editor = TestEditor::new(md);
        assert!(editor.editor.edit.renderer.bounds.tables.is_empty(), "{md:?}");
        assert_eq!(rects_of(&editor, "a | b").len(), 1, "{md:?}");

        // and is text to every edit
        caret_at(&mut editor, md.find('a').unwrap());
        act(&mut editor, TableAction::InsertColumn { right: true });
        assert_eq!(editor.get_text(), md);
        type_text(&mut editor, "x");
        assert_eq!(editor.get_text(), md.replacen('a', "xa", 1));
    }
}

#[test]
fn selecting_a_row_in_view_leaves_the_page_where_it_is() {
    let mut md = String::new();
    for n in 0..60 {
        md += &format!("above {n}\n\n");
    }
    md += "| a | b |\n| - | - |\n";
    for n in 0..5 {
        md += &format!("| cell {n} | cell {n} |\n");
    }
    for n in 0..80 {
        md += &format!("\nbelow {n}\n");
    }
    let mut editor = TestEditor::new(&md);
    editor.enter_frame();
    // the table's top just above the view
    let below = md.find("below 5").unwrap();
    editor.push(Event::Select { region: Region::Location(Location::Grapheme(Grapheme(below))) });
    editor.enter_frame();
    let row = editor.editor.edit.renderer.bounds.tables[0].rows[3].line;
    let before = editor.editor.edit.scroll_area.stored_offset();
    editor.push(Event::Select { region: row.into() });
    editor.enter_frame();
    assert_eq!(editor.editor.edit.scroll_area.stored_offset(), before);
}
