//! Heading links: where opening one lands the caret, what it unfolds.

use lb_rs::model::text::offset_types::Grapheme;

use super::super::ScrollTarget;
use super::super::fold::FOLD_TAG;
use super::harness::TestEditor;

/// The caret lands after the heading's text, before its fold tag, and the
/// line is pinned to the top. A heading inside a fold unfolds it for real,
/// and a fragment that names nothing goes to the top, caret left alone.
#[test]
fn opening_at_a_heading() {
    let md =
        format!("# Top{FOLD_TAG}\n\nbody\n\n## Inner\n\nmore\n\n# Next steps{FOLD_TAG}\n\nhidden");
    let mut ws = TestEditor::new(&md);
    ws.enter_frame();
    let selection = |ws: &TestEditor| ws.editor.edit.renderer.buffer.current.selection;

    ws.editor.open_at_heading("next-steps");
    let caret = Grapheme(md.find("Next steps").unwrap() + "Next steps".len());
    assert_eq!(selection(&ws), (caret, caret));
    assert_eq!(ws.editor.edit.pending_scroll, Some(ScrollTarget::Heading));
    assert_eq!(ws.get_text().matches(FOLD_TAG).count(), 2, "its own fold stays");

    ws.editor.open_at_heading("Inner");
    let text = ws.get_text().to_string();
    assert_eq!(text.matches(FOLD_TAG).count(), 1, "the fold hiding it opens");
    let caret = Grapheme(text.find("Inner").unwrap() + "Inner".len());
    assert_eq!(selection(&ws), (caret, caret));

    ws.editor.edit.pending_scroll = None;
    ws.editor.open_at_heading("nowhere");
    assert_eq!(selection(&ws), (caret, caret));
    assert_eq!(ws.editor.edit.pending_scroll, Some(ScrollTarget::Heading));
}

/// A heading lands at one spot below the top of the view whether it starts
/// on screen or far down the note, in the frame that scrolls to it: its row
/// is laid out before the scroll, never landed by estimate and corrected.
/// Reading on touch, so the caret stays behind and its row can't stand in.
#[test]
fn a_far_heading_lands_where_a_near_one_does() {
    let filler = "line\n\n".repeat(300);
    let md = format!("intro\n\n# Near\n\n{filler}# Far\n\n{filler}");
    let mut ws = TestEditor::new(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.editor.keyboard_visible = false;
    ws.enter_frame();
    let landed_top = |ws: &mut TestEditor, heading: &str| {
        ws.editor.open_at_heading(&heading.to_lowercase());
        ws.enter_frame(); // lays out its row and scrolls
        ws.enter_frame(); // paints there
        let at = Grapheme(md.find(&format!("# {heading}")).unwrap() + 2);
        let top = ws
            .editor
            .edit
            .renderer
            .fragment_at_offset(at)
            .unwrap()
            .rect
            .top();
        ws.enter_frame();
        let again = ws
            .editor
            .edit
            .renderer
            .fragment_at_offset(at)
            .unwrap()
            .rect
            .top();
        assert_eq!(top, again, "{heading} moved after landing");
        top
    };
    let near = landed_top(&mut ws, "Near");
    let far = landed_top(&mut ws, "Far");
    assert!((near - far).abs() < 0.5, "near {near}, far {far}");
}
