//! Geometry and unit answers the iOS host relies on, checked headlessly.

use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::harness::TestEditor;

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
