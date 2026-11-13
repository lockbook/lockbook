//! The cursor over the editor's controls, from the first frame a pointer
//! lands on one: a control the page shows only on hover has no hit-test
//! from the frame before.

use egui::{CursorIcon, Pos2, Rect, pos2};

use super::super::MdRender;
use crate::test_utils::ws::TestWs;

fn open(md: &str) -> TestWs {
    let mut t = TestWs::new();
    let note = t.doc("note.md", md);
    t.ws.open_file(note, true, true);
    t.idle(note);
    for _ in 0..3 {
        t.frame();
    }
    t
}

fn renderer(t: &mut TestWs) -> &MdRender {
    &t.ws
        .current_tab_mut()
        .unwrap()
        .markdown_mut()
        .unwrap()
        .edit
        .renderer
}

/// Where the fragment showing `text` is drawn.
fn word(t: &mut TestWs, text: &str) -> Rect {
    let r = renderer(t);
    r.fragments
        .iter()
        .find(|f| &r.buffer[f.source_range] == text)
        .map(|f| f.rect)
        .unwrap()
}

/// The cursor on the frame the pointer arrives at `pos` from off the page.
fn landing(t: &mut TestWs, pos: Pos2) -> CursorIcon {
    t.frame_with(vec![egui::Event::PointerMoved(pos2(2.0, 2.0))]);
    t.frame();
    t.frame_with(vec![egui::Event::PointerMoved(pos)]);
    t.cursor
}

#[test]
fn fold_button_shows_the_arrow() {
    let mut t = open("body\n\n# Heading\n\nunder it\n");
    let (left, title) = (word(&mut t, "body").left(), word(&mut t, "Heading"));
    let indent = renderer(&mut t).layout.indent;
    // the button sits in the gutter, an indent left of the heading's text
    let button = pos2(left - indent * 0.6, title.center().y);
    assert_eq!(landing(&mut t, button), CursorIcon::Default);
    assert_eq!(landing(&mut t, title.center()), CursorIcon::Text);
}

#[test]
fn heading_link_button_shows_the_arrow() {
    let mut t = open("body\n\n# Heading\n\nunder it\n");
    let title = word(&mut t, "Heading");
    let layout = &renderer(&mut t).layout;
    let button =
        pos2(title.right() + layout.inline_padding * 2.0 + layout.indent * 0.4, title.center().y);
    assert_eq!(landing(&mut t, button), CursorIcon::Default);
}

#[test]
fn code_copy_button_shows_the_arrow() {
    let mut t = open("body\n\n```rust\nfn main() {}\n```\n");
    let (left, row) = (word(&mut t, "body").left(), word(&mut t, "fn"));
    let r = renderer(&mut t);
    let (right, layout) = (left + r.width, &r.layout);
    // over the end of the block's first row
    let size = layout.row_height + layout.row_spacing;
    let inset = layout.block_padding - layout.row_spacing / 2.0;
    let button = pos2(right - inset - size / 2.0, row.center().y);
    assert_eq!(landing(&mut t, button), CursorIcon::Default);
}

/// A column's handle drags it; a row's, with no other body row to trade
/// with, is a plain control.
#[test]
fn table_handles_show_their_cursor() {
    let mut t = open("body\n\n| a | b |\n| --- | --- |\n| c | d |\n");
    let cells = renderer(&mut t).table_cell_boxes.clone();
    let column = cells
        .iter()
        .filter(|c| c.column == 0)
        .map(|c| c.rect)
        .reduce(Rect::union)
        .unwrap();
    let body = cells.iter().map(|c| c.row).max().unwrap();
    let row = cells
        .iter()
        .filter(|c| c.row == body)
        .map(|c| c.rect)
        .reduce(Rect::union)
        .unwrap();
    assert_eq!(landing(&mut t, column.center_top()), CursorIcon::Grab);
    assert_eq!(landing(&mut t, row.left_center()), CursorIcon::Default);
}
