//! Dragging an image to another place in the note, driven by pointer
//! events through a whole frame loop: the press on the image, the drag,
//! and the release where the pointer lets go; on touch, the long press
//! that lifts it.

use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::harness::{TestEditor, build_lb};

const IMAGE: &str = "https://x.test/a.png";

fn editor(md: &str) -> TestEditor {
    let (mut ws, embeds) = TestEditor::with_test_embeds(build_lb(), md);
    embeds.complete(IMAGE, egui::vec2(40.0, 40.0));
    ws.enter_frame();
    ws
}

/// Where a click puts the caret just after `needle`.
fn after(ws: &TestEditor, needle: &str) -> Pos2 {
    let offset = Grapheme(ws.get_text().find(needle).unwrap() + needle.len());
    let [top, bot] = ws.editor.edit.cursor_line(offset).unwrap();
    Pos2::new(top.x - 1.0, (top.y + bot.y) / 2.0)
}

/// A touch drag let go at `to`: the frames until the text moves.
fn drop_frames(ws: &mut TestEditor, from: Pos2, to: Pos2, md: &str) {
    assert!(ws.editor.edit.reorder_start(from), "a long press lifts it");
    ws.enter_frame();
    ws.editor.edit.reorder_move(to);
    ws.enter_frame();
    ws.editor.edit.reorder_end(to, false);
    for _ in 0..3 {
        ws.enter_frame();
        let edit = &ws.editor.edit;
        let floating = edit.in_progress_block_drag.is_some();
        assert!(ws.get_text() != md || floating, "a frame drew it back where it was");
    }
    assert_ne!(ws.get_text(), md, "the drop moved it");
}

#[test]
fn a_dropped_list_item_never_draws_back_where_it_was() {
    let md = "- one\n- two\n- three\n";
    let mut ws = editor(md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let below = |ws: &TestEditor, needle: &str| after(ws, needle) + egui::vec2(0.0, 8.0);
    let (from, to) = (after(&ws, "- on"), below(&ws, "three"));
    drop_frames(&mut ws, from, to, md);
}
