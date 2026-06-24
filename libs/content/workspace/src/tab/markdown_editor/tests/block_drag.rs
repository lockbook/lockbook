//! Drag-to-reorder tests for list items. The interaction surface (pointer
//! events, marker hit-testing) is hard to drive headlessly, so these
//! exercise the pure move planner ([`MdRender::plan_block_move`]) and
//! the [`MdEdit::move_block`] buffer op it produces — the parts that
//! carry correctness.
//!
//! Style: self-referential invariants (round trip, conservation,
//! multiset) over a differential oracle.

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::harness::TestEditor;

/// All list items in document order, paired with their parent `List`
/// node range start (the sibling-group identifier).
fn items(ws: &mut TestEditor) -> Vec<((Grapheme, Grapheme), Grapheme)> {
    let arena = Arena::new();
    let root: &AstNode = ws.editor.edit.renderer.reparse(&arena);
    let r = &ws.editor.edit.renderer;
    let mut out = Vec::new();
    for node in root.descendants() {
        if matches!(node.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_)) {
            let parent = node.parent().unwrap();
            out.push((r.node_range(node), r.node_range(parent).start()));
        }
    }
    out
}

fn ws_text(ws: &TestEditor) -> String {
    ws.editor.edit.renderer.buffer.current.text.clone()
}

/// Grapheme range → byte range (ASCII test docs).
fn byte_lossy(r: &(Grapheme, Grapheme)) -> std::ops::Range<usize> {
    r.start().0..r.end().0
}

fn find_item_with(ws: &mut TestEditor, needle: &str) -> (Grapheme, Grapheme) {
    let text = ws_text(ws);
    items(ws)
        .into_iter()
        .find(|(r, _)| text[byte_lossy(r)].contains(needle))
        .unwrap_or_else(|| panic!("no list item containing {needle:?}"))
        .0
}

#[test]
fn move_then_move_back_round_trips() {
    let doc = "- alpha\n- bravo\n- charlie\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();
    let original = ws_text(&ws);

    let all = items(&mut ws);
    let alpha = all[0].0;
    let charlie_start = all[2].0.start();
    ws.editor.edit.move_block(alpha, charlie_start);
    ws.enter_frame();

    assert_ne!(ws_text(&ws), original, "move should change the document");

    // Move "alpha" back to the top.
    let alpha_now = find_item_with(&mut ws, "alpha");
    ws.editor.edit.move_block(alpha_now, Grapheme(0));
    ws.enter_frame();

    assert_eq!(ws_text(&ws), original, "moving an item out and back restores the doc");
}

#[test]
fn multiset_of_items_preserved() {
    let doc = "- one\n- two\n- three\n- four\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();

    let text = ws_text(&ws);
    let mut before: Vec<String> = items(&mut ws)
        .into_iter()
        .map(|(r, _)| text[byte_lossy(&r)].trim().to_string())
        .collect();
    before.sort();

    let three = find_item_with(&mut ws, "three");
    ws.editor.edit.move_block(three, Grapheme(0));
    ws.enter_frame();

    let text = ws_text(&ws);
    let mut after: Vec<String> = items(&mut ws)
        .into_iter()
        .map(|(r, _)| text[byte_lossy(&r)].trim().to_string())
        .collect();
    after.sort();

    assert_eq!(before, after, "reorder must preserve the multiset of items");
}

#[test]
fn moving_item_carries_nested_children() {
    let doc = "- first\n- second\n  - nested\n- third\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();
    assert!(ws_text(&ws).contains("  - nested"));

    // The "second" item owns the nested child in its node range.
    let second = find_item_with(&mut ws, "second");
    let second_text = ws_text(&ws)[byte_lossy(&second)].to_string();
    assert!(
        second_text.contains("nested"),
        "item node range includes nested children: {second_text:?}"
    );

    // Move "second" (with its nested child) to the top.
    ws.editor.edit.move_block(second, Grapheme(0));
    ws.enter_frame();

    let text = ws_text(&ws);
    let second_pos = text.find("second").unwrap();
    let nested_pos = text.find("nested").unwrap();
    assert!(nested_pos > second_pos, "nested child travels with its parent item");
    let first_pos = text.find("first").unwrap();
    assert!(second_pos < first_pos, "second moved above first");
}

#[test]
fn reordering_tight_list_keeps_it_tight() {
    let doc = "- one\n- two\n- three\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();
    let one = find_item_with(&mut ws, "one");
    let three_start = find_item_with(&mut ws, "three").start();
    ws.editor.edit.move_block(one, three_start);
    ws.enter_frame();
    assert_eq!(ws_text(&ws), "- two\n- one\n- three\n");
}

#[test]
fn appending_to_tight_list_stays_tight() {
    use crate::tab::markdown_editor::widget::block::drag::BlockDrag;
    let doc = "- one\n- two\n- three\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.edit.renderer.interactive = true;
    ws.enter_frame();

    // Find "one"'s indexed box so we can compute a drop gap past the
    // end of its sibling group.
    let r = &ws.editor.edit.renderer;
    let one_box = r
        .block_boxes
        .iter()
        .find(|b| {
            let t = &r.buffer.current.text[byte_lossy(&b.node_range)];
            t.contains("one")
        })
        .copied()
        .expect("one's box is indexed once it renders");
    let drag = BlockDrag {
        section_range: one_box.node_range,
        grabbed: one_box.node_range,
        parent_start: one_box.parent_start,
        grab_offset: one_box.rect.center() - one_box.rect.left_top(),
    };
    let past_end = egui::Pos2::new(one_box.rect.center().x, 1.0e6);
    let gap = r
        .drop_gap_for(&drag, past_end)
        .expect("a trailing gap exists");

    ws.editor
        .edit
        .move_block(one_box.node_range, gap.insert_offset);
    ws.enter_frame();
    assert_eq!(ws_text(&ws), "- two\n- three\n- one\n");
}

#[test]
fn drop_gap_geometry_for_list() {
    use crate::tab::markdown_editor::widget::block::drag::BlockDrag;
    let doc = "- one\n- two\n- three\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.edit.renderer.interactive = true;
    ws.enter_frame();

    let r = &ws.editor.edit.renderer;
    let boxes = &r.block_boxes;
    assert!(boxes.len() >= 3, "every list item gets a box: {}", boxes.len());
    // All three items share the same `List` parent.
    let parent_starts: std::collections::HashSet<_> =
        boxes.iter().map(|b| b.parent_start).collect();
    assert_eq!(parent_starts.len(), 1, "all items share one parent List");
    // Boxes are vertically ordered by source offset.
    let mut sorted = boxes.clone();
    sorted.sort_by_key(|b| b.node_range.start());
    for w in sorted.windows(2) {
        assert!(w[0].rect.top() <= w[1].rect.top());
    }

    // Dragging "one" with the pointer near the bottom resolves to a
    // gap past the last sibling (gap_index == sibling count).
    let one = sorted[0];
    let drag = BlockDrag {
        section_range: one.node_range,
        grabbed: one.node_range,
        parent_start: one.parent_start,
        grab_offset: one.rect.center() - one.rect.left_top(),
    };
    let low = egui::Pos2::new(one.rect.center().x, 100_000.0);
    let gap = r.drop_gap_for(&drag, low).expect("a gap exists");
    assert_eq!(gap.gap_index, sorted.len(), "pointer past the end picks the trailing gap");
}

// Hovering within the dragged item's own span is a cancel: no drop gap.
#[test]
fn drag_within_own_span_is_a_cancel() {
    use crate::tab::markdown_editor::widget::block::drag::BlockDrag;
    let doc = "- one\n- two\n- three\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.edit.renderer.interactive = true;
    ws.enter_frame();

    let r = &ws.editor.edit.renderer;
    let one = r
        .block_boxes
        .iter()
        .find(|b| r.buffer.current.text[byte_lossy(&b.node_range)].contains("one"))
        .copied()
        .unwrap();
    let drag = BlockDrag {
        section_range: one.node_range,
        grabbed: one.node_range,
        parent_start: one.parent_start,
        // grab at the marker (top-left of the source row, x at the marker center)
        grab_offset: egui::Vec2::new(one.rect.width() / 2.0, 0.0),
    };
    let within = one.rect.center();
    assert!(r.drop_gap_for(&drag, within).is_none(), "hovering own span cancels");
}

// Task items reorder the same way bullets do.
#[test]
fn moving_task_item_works() {
    let doc = "- [ ] one\n- [x] two\n- [ ] three\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();

    let one = find_item_with(&mut ws, "one");
    let three_start = find_item_with(&mut ws, "three").start();
    ws.editor.edit.move_block(one, three_start);
    ws.enter_frame();
    assert_eq!(ws_text(&ws), "- [x] two\n- [ ] one\n- [ ] three\n");
}
