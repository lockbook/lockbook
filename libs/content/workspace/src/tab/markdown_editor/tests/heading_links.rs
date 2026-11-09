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
/// on screen or far down the note. One that is a block of its own is there
/// in the first frame it paints; one inside a quote waits a frame for its row
/// to be laid out, and the two spots agree. Reading on touch, so the caret
/// stays behind and its row can't stand in.
#[test]
fn a_far_heading_lands_where_a_near_one_does() {
    let filler = "line\n\n".repeat(300);
    let md = format!("intro\n\n# Near\n\n{filler}# Far\n\n{filler}> # Quoted\n\n{filler}");
    let mut ws = TestEditor::new(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.editor.editing = false;
    ws.enter_frame();
    let landed_top = |ws: &mut TestEditor, heading: &str, frames: usize| {
        ws.editor.open_at_heading(&heading.to_lowercase());
        for _ in 0..frames {
            ws.enter_frame();
        }
        let at = Grapheme(md.find(&format!("# {heading}")).unwrap() + 2);
        let top = |ws: &TestEditor| {
            ws.editor
                .edit
                .renderer
                .fragment_at_offset(at)
                .unwrap()
                .rect
                .top()
        };
        let landed = top(ws);
        ws.enter_frame();
        assert_eq!(landed, top(ws), "{heading} moved after landing");
        landed
    };
    let near = landed_top(&mut ws, "Near", 1);
    let far = landed_top(&mut ws, "Far", 1);
    let quoted = landed_top(&mut ws, "Quoted", 2);
    assert!((near - far).abs() < 0.5, "near {near}, far {far}");
    assert!((near - quoted).abs() < 0.5, "near {near}, quoted {quoted}");
}

/// The link glyph sits after a heading's last letter, not after the spaces
/// that trail it.
#[test]
fn the_link_glyph_follows_a_headings_text_not_its_spaces() {
    let md = format!("# Title{}\n\nbody\n", " ".repeat(14));
    let mut ws = TestEditor::new(&md);
    // the caret after the spaces, as when they were just typed
    let end = super::super::input::Location::Grapheme(Grapheme(7 + 14));
    let region = super::super::input::Region::BetweenLocations { start: end, end };
    ws.push(super::super::input::Event::Select { region });
    ws.enter_frame();
    let [top, bottom] = ws.editor.edit.cursor_line(Grapheme(7)).unwrap();
    let layout = &ws.editor.edit.renderer.layout;
    let size = layout.indent * 0.8;
    let pos =
        egui::pos2(top.x + layout.inline_padding * 2.0 + size / 2.0, (top.y + bottom.y) / 2.0);
    let (button, modifiers) = (egui::PointerButton::Primary, egui::Modifiers::default());
    ws.enter_frame_with_input(vec![egui::Event::PointerMoved(pos)]);
    ws.enter_frame_with_input(vec![
        egui::Event::PointerButton { pos, button, pressed: true, modifiers },
        egui::Event::PointerButton { pos, button, pressed: false, modifiers },
    ]);
    assert!(ws.editor.edit.renderer.heading_copied.is_some(), "the glyph at {pos:?} copies");
}
