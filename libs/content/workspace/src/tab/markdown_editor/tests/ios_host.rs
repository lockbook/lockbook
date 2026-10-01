//! Geometry and unit answers the iOS host relies on, checked headlessly.

use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;

fn select(ws: &mut TestEditor, g: usize) {
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(g)),
            end: Location::Grapheme(Grapheme(g)),
        },
    });
    ws.enter_frame();
}

/// A selection ending right after a newline has no glyph on the next line,
/// but the platform draws the end handle in the last rect, so that rect
/// sits on the next line where the caret would.
#[test]
fn selection_rects_end_on_a_blank_line() {
    let mut ws = TestEditor::new("aaa\n\nbbb\n\n\nccc");
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let [blank_top, _] = edit.cursor_line(Grapheme(9)).unwrap();
    let rects = edit.selection_rects((Grapheme(5), Grapheme(9)));
    let last = rects.last().unwrap();
    assert!(
        last.y_range().contains(blank_top.y + 4.0),
        "last rect {last:?} not on the blank line at y {}",
        blank_top.y
    );
    assert!((last.min.x - blank_top.x).abs() < 1.0);
    // A range ending mid-row adds nothing.
    assert_eq!(edit.selection_rects((Grapheme(5), Grapheme(7))).len(), 1);
    // A range ending before the newline adds nothing either.
    assert_eq!(edit.selection_rects((Grapheme(5), Grapheme(8))).len(), 1);
}

/// A tap past the end of a line lands after its trailing space, where the
/// next sentence starts, so the keyboard capitalizes what follows.
#[test]
fn tap_past_line_end_lands_after_trailing_space() {
    let md = "First sentence. \nSecond line";
    let mut ws = TestEditor::new(md);
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let [top, bottom] = edit.cursor_line(Grapheme(5)).unwrap();
    let p = Pos2::new(700.0, (top.y + bottom.y) / 2.0);
    let offset = edit.pos_to_char_offset(p);
    assert_eq!(offset, Grapheme("First sentence. ".len()), "tap past the line end");
}

/// A platform's edit is applied before its frame, so the unfold the frame
/// does for an edit inside folded contents runs on that path too: a caret
/// left in a section, then folded over, types the section open again.
#[test]
fn platform_typing_inside_a_fold_unfolds_it() {
    use super::super::fold::FOLD_TAG;
    let doc = format!("# Heading {FOLD_TAG}\n\ntext\n\n");
    let mut ws = TestEditor::new(&doc);
    let end = doc.chars().count();
    select(&mut ws, end);
    assert!(ws.get_text().contains(FOLD_TAG));
    ws.editor.edit.apply_platform_event(Event::Replace {
        region: Region::Selection,
        text: "x".into(),
        advance_cursor: true,
    });
    ws.enter_frame();
    assert!(!ws.get_text().contains(FOLD_TAG), "{}", ws.get_text());
}

/// A tap to the right of a heading's fold chip places the caret after the
/// fold tag, at the end of the heading line, not before the chip.
#[test]
fn tap_right_of_a_fold_chip_lands_after_it() {
    use super::super::fold::FOLD_TAG;
    let heading = format!("# Heading {FOLD_TAG}");
    let doc = format!("{heading}\n\nhidden\n");
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let tag_end = Grapheme(heading.chars().count());
    let [top, bottom] = edit.cursor_line(tag_end).unwrap();
    let p = Pos2::new(top.x + 30.0, (top.y + bottom.y) / 2.0);
    assert_eq!(edit.pos_to_char_offset(p), tag_end, "caret x after chip = {}", top.x);
}
