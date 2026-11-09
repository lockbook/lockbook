//! Dragging an image to another place in the note, driven by pointer
//! events through a whole frame loop: the press on the image, the drag,
//! and the release where the pointer lets go; on touch, the long press
//! that lifts it.

use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::Event;
use super::super::widget::utils::wrap_layout::FragmentContent;
use super::harness::{TestEditor, build_lb};
use super::tables::table_ranges_check_md;

const IMAGE: &str = "https://x.test/a.png";

fn editor(md: &str) -> TestEditor {
    let (mut ws, embeds) = TestEditor::with_test_embeds(build_lb(), md);
    embeds.complete(IMAGE, egui::vec2(40.0, 40.0));
    ws.enter_frame();
    ws
}

fn image_center(ws: &TestEditor) -> Pos2 {
    ws.editor
        .edit
        .renderer
        .fragments
        .iter()
        .find(|f| matches!(f.content, FragmentContent::Embed { .. }))
        .expect("the image draws")
        .rect
        .center()
}

/// Where a click puts the caret just after `needle`.
fn after(ws: &TestEditor, needle: &str) -> Pos2 {
    let offset = Grapheme(ws.get_text().find(needle).unwrap() + needle.len());
    let [top, bot] = ws.editor.edit.cursor_line(offset).unwrap();
    Pos2::new(top.x - 1.0, (top.y + bot.y) / 2.0)
}

/// A mouse drag from `from` to `to`, then down until the drop aims its
/// farthest above the pointer, just above the image, so at `to`, and let go
/// there.
fn drag(ws: &mut TestEditor, from: Pos2, to: Pos2) {
    let button = |pos, pressed| Egui::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    ws.enter_frame_with_input(vec![Egui::PointerMoved(from)]);
    ws.enter_frame_with_input(vec![button(from, true)]);
    ws.enter_frame_with_input(vec![Egui::PointerMoved(from + egui::vec2(20.0, 20.0))]);
    let edit = &ws.editor.edit;
    let grab = edit
        .in_progress_image_drag
        .as_ref()
        .expect("it lifted")
        .grab_offset;
    let lift = egui::vec2(0.0, grab.y + edit.renderer.drop_lift());
    ws.enter_frame_with_input(vec![Egui::PointerMoved(to)]);
    ws.enter_frame_with_input(vec![Egui::PointerMoved(to + lift)]);
    ws.enter_frame_with_input(vec![button(to + lift, false)]);
    ws.enter_frame();
}

#[test]
fn image_drags_to_another_cell() {
    let md = format!("| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);

    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    drag(&mut ws, from, to);

    let moved = format!("|  | b |\n| - | - |\n| c | d![a]({IMAGE}) |\n");
    assert_eq!(ws.get_text(), moved);
    table_ranges_check_md(&moved).unwrap();

    ws.push(Event::Undo);
    ws.enter_frame();
    assert_eq!(ws.get_text(), md, "one undo puts the image back");
}

#[test]
fn sized_image_drags_into_a_cell_escaped() {
    let md = format!("![a|20]({IMAGE})\n\n| b | c |\n| - | - |\n");
    let mut ws = editor(&md);

    let (from, to) = (image_center(&ws), after(&ws, "| b"));
    drag(&mut ws, from, to);

    let moved = format!("\n\n| b![a\\|20]({IMAGE}) | c |\n| - | - |\n");
    assert_eq!(ws.get_text(), moved);
    assert_eq!(table_ranges_check_md(&moved), Ok(1));
}

/// Where a finger holding the lifted image goes for the drop to aim at
/// `to`, having first moved down far enough that the drop aims its farthest
/// above the finger, just above the image, which covers what's under it.
fn aimed_at(ws: &mut TestEditor, from: Pos2, to: Pos2) -> Pos2 {
    let edit = &ws.editor.edit;
    let grab = edit
        .in_progress_image_drag
        .as_ref()
        .expect("it lifted")
        .grab_offset;
    let lift = egui::vec2(0.0, grab.y + edit.renderer.drop_lift());
    ws.editor.edit.reorder_move(from + lift);
    ws.enter_frame();
    to + lift
}

#[test]
fn touch_long_press_drags_image_to_another_cell() {
    let md = format!("| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();

    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    assert!(ws.editor.edit.reorder_can_start(from), "a long press on the image lifts it");
    assert!(ws.editor.edit.reorder_start(from));
    ws.enter_frame();
    let to = aimed_at(&mut ws, from, to);
    ws.editor.edit.reorder_move(to);
    ws.enter_frame();
    ws.editor.edit.reorder_end(to, false);
    ws.enter_frame();
    ws.enter_frame();

    assert_eq!(ws.get_text(), format!("|  | b |\n| - | - |\n| c | d![a]({IMAGE}) |\n"));
}

#[test]
fn a_touch_drag_that_never_went_down_drops_under_the_finger() {
    let md = format!("| b | c |\n| - | - |\n| ![a]({IMAGE}) | d |\n");
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let (from, to) = (image_center(&ws), after(&ws, "| b"));
    assert!(ws.editor.edit.reorder_start(from));
    ws.enter_frame();
    ws.editor.edit.reorder_move(to);
    ws.enter_frame();
    ws.editor.edit.reorder_end(to, false);
    ws.enter_frame();
    ws.enter_frame();
    assert_eq!(ws.get_text(), format!("| b![a]({IMAGE}) | c |\n| - | - |\n|  | d |\n"));
}

#[test]
fn image_drags_into_an_empty_cell() {
    let md = format!("| a | b |\n| - | - |\n|    | ![a]({IMAGE}) |\n");
    let mut ws = editor(&md);

    let (from, to) = (image_center(&ws), super::tables::cell_rects(&ws)[1][0].center());
    drag(&mut ws, from, to);

    let moved = format!("| a | b |\n| - | - |\n| ![a]({IMAGE})   |  |\n");
    assert_eq!(ws.get_text(), moved);
}

#[test]
fn an_image_dropped_below_a_table_is_one_more_row() {
    let md = format!("| a | b |\n| - | - |\n| ![a]({IMAGE}) | x |\n");
    let mut ws = editor(&md);

    let below = super::tables::cell_rects(&ws)[1][0].left_bottom() + egui::vec2(10.0, 15.0);
    let from = image_center(&ws);
    drag(&mut ws, from, below);

    // the line under a table's last row is a row, as typed there it would be
    let moved = format!("| a | b |\n| - | - |\n|  | x |\n![a]({IMAGE})");
    assert_eq!(ws.get_text(), moved);
    table_ranges_check_md(&moved).unwrap();
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
        let floating =
            edit.in_progress_image_drag.is_some() || edit.in_progress_block_drag.is_some();
        assert!(ws.get_text() != md || floating, "a frame drew it back where it was");
    }
    assert_ne!(ws.get_text(), md, "the drop moved it");
}

#[test]
fn a_dropped_image_never_draws_back_where_it_was() {
    let md = format!("| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    drop_frames(&mut ws, from, to, &md);
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

#[test]
fn a_drop_leaves_the_caret_where_it_was() {
    let md = format!("caret\n\n| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    let caret = Grapheme("car".len());
    ws.editor
        .edit
        .renderer
        .buffer
        .queue(vec![lb_rs::model::text::operation_types::Operation::Select((caret, caret))]);
    ws.editor.edit.renderer.buffer.update();
    ws.enter_frame();

    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    drag(&mut ws, from, to);

    assert_ne!(ws.get_text(), md, "the drop moved it");
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (caret, caret));
}

#[test]
fn a_drag_at_a_wide_tables_edge_scrolls_it_along() {
    let word = "x".repeat(40);
    let md = format!(
        "| ![a]({IMAGE}) | {word} | {word} | {word} |\n| - | - | - | - |\n| c | {word} | {word} | {word} |\n"
    );
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let viewport = super::tables::cell_rects(&ws)[0][0].left();
    let r = &ws.editor.edit.renderer;
    let (table, _) = *r
        .row_scrollers
        .first()
        .expect("the table is wider than the note");
    assert!(viewport >= table.left());

    let from = image_center(&ws);
    assert!(ws.editor.edit.reorder_start(from));
    ws.enter_frame();
    let edge = Pos2::new(table.right() - 2.0, from.y);
    for _ in 0..5 {
        ws.editor.edit.reorder_move(edge);
        ws.enter_frame();
    }
    assert!(ws.editor.edit.renderer.table_scrolled(Grapheme(0)) > 0.0, "it scrolled along");
}

#[test]
fn an_image_dropped_in_code_lands_at_its_nearer_edge() {
    let cases = [
        (
            format!("`some code here`\n\n![a]({IMAGE})\n"),
            format!("![a]({IMAGE})`some code here`\n\n\n"),
        ),
        (
            format!("para\n\n```\nsome code here\n```\n\n![a]({IMAGE})\n"),
            format!("para\n![a]({IMAGE})\n```\nsome code here\n```\n\n\n"),
        ),
    ];
    for (md, moved) in cases {
        let mut ws = editor(&md);
        let (from, to) = (image_center(&ws), after(&ws, "some co"));
        drag(&mut ws, from, to);
        assert_eq!(ws.get_text(), moved);
    }
}

#[test]
fn a_drag_past_a_wide_tables_edge_keeps_scrolling_it() {
    let word = "x".repeat(40);
    let md = format!(
        "| ![a]({IMAGE}) | {word} | {word} | {word} |\n| - | - | - | - |\n| c | {word} | {word} | {word} |\n"
    );
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let (table, _) = *ws
        .editor
        .edit
        .renderer
        .row_scrollers
        .first()
        .expect("the table is wider than the note");
    let from = image_center(&ws);
    assert!(ws.editor.edit.reorder_start(from));
    ws.enter_frame();
    let past = Pos2::new(table.right() + 10.0, from.y);
    for _ in 0..5 {
        ws.editor.edit.reorder_move(past);
        ws.enter_frame();
    }
    assert!(ws.editor.edit.renderer.table_scrolled(Grapheme(0)) > 0.0, "it scrolled along");
}

#[test]
fn an_image_off_its_own_line_leaves_the_line_empty() {
    let md = format!("one\n![a]({IMAGE})\ntwo\n\n| b | c |\n| - | - |\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "| b"));
    drag(&mut ws, from, to);
    assert_eq!(ws.get_text(), format!("one\n\ntwo\n\n| b![a]({IMAGE}) | c |\n| - | - |\n"));
}

#[test]
fn an_image_never_runs_into_a_url_before_it() {
    let md = format!("see https://x.test/page. now\n\n![a]({IMAGE})\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "https://x.test/page."));
    drag(&mut ws, from, to);
    assert_eq!(ws.get_text(), format!("see ![a]({IMAGE})https://x.test/page. now\n\n\n"));
}

#[test]
fn an_image_moved_to_the_next_line_leaves_its_line() {
    let md = format!("a\n![a]({IMAGE})\nb\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "a.png)\n"));
    drag(&mut ws, from, to);
    assert_eq!(ws.get_text(), format!("a\n\n![a]({IMAGE})b\n"));
}

#[test]
fn an_image_dropped_in_a_link_lands_at_its_nearer_edge() {
    let md = format!("see https://x.test/page now\n\n![a]({IMAGE})\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "https://x.te"));
    drag(&mut ws, from, to);
    // its end would run the url on into the image
    assert_eq!(ws.get_text(), format!("see ![a]({IMAGE})https://x.test/page now\n\n\n"));
    ws.enter_frame();
    image_center(&ws);
}

#[test]
fn an_image_dropped_inside_a_word_lands_at_its_nearer_edge() {
    let md = format!("# A heading to drop into\n\n![a]({IMAGE})\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "# A heading to dro"));
    drag(&mut ws, from, to);
    assert_eq!(ws.get_text(), format!("# A heading to drop![a]({IMAGE}) into\n\n\n"));
}

#[test]
fn an_undo_after_a_tap_elsewhere_comes_back_to_the_image() {
    use super::super::input::{Location, Region};
    let md = format!("caret\n\n| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    drag(&mut ws, from, to);
    assert_ne!(ws.get_text(), md, "the drop moved it");

    // a tap elsewhere, as the one that brings a keyboard up
    let elsewhere = Location::Grapheme(Grapheme(2));
    ws.push(Event::Select { region: Region::Location(elsewhere) });
    ws.enter_frame();
    ws.push(Event::Undo);
    ws.enter_frame();

    assert_eq!(ws.get_text(), md, "one undo puts the image back");
    let start = md.find("![a]").unwrap();
    let image = (Grapheme(start), Grapheme(start + format!("![a]({IMAGE})").len()));
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, image, "selected where it was");
}

#[test]
fn no_frame_of_a_drop_moves_the_caret() {
    let md = format!("caret\n\n| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    let caret = (Grapheme(3), Grapheme(3));
    ws.editor
        .edit
        .renderer
        .buffer
        .queue(vec![lb_rs::model::text::operation_types::Operation::Select(caret)]);
    ws.editor.edit.renderer.buffer.update();
    ws.enter_frame();

    let (from, to) = (image_center(&ws), after(&ws, "| d"));
    let button = |pos, pressed| Egui::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let frames = [
        vec![Egui::PointerMoved(from)],
        vec![button(from, true)],
        vec![Egui::PointerMoved(from + egui::vec2(20.0, 20.0))],
        vec![Egui::PointerMoved(to)],
        vec![button(to, false)],
        vec![],
        vec![],
    ];
    for input in frames {
        ws.enter_frame_with_input(input);
        // a platform reads the selection between frames: it must not see one move
        assert_eq!(ws.editor.edit.renderer.buffer.current.selection, caret);
    }
    assert_ne!(ws.get_text(), md, "the drop moved it");
}

/// A move is a cut and a paste of the image's characters and nothing else:
/// the note reads as the source with them spliced out and back in.
#[test]
fn a_move_changes_only_the_images_characters() {
    let image = format!("![a]({IMAGE})");
    let cases = [
        (format!("- item\n\npara {image}\n"), "- item\n"),
        (format!("para\n\n{image}\n\nnext\n"), "para\n"),
        (format!("para\n\n```\ncode\n```\n\n{image}\n"), "para\n"),
    ];
    for (md, needle) in cases {
        let mut ws = editor(&md);
        let (from, to) = (image_center(&ws), after(&ws, needle));
        drag(&mut ws, from, to);
        let at = md.find(needle).unwrap() + needle.len();
        let was = md.find(&image).unwrap();
        let removed = md.replacen(&image, "", 1);
        let at = if was < at { at - image.len() } else { at };
        let spliced = format!("{}{image}{}", &removed[..at], &removed[at..]);
        assert_eq!(ws.get_text(), spliced, "from {md:?}");
    }
}

/// Text edited while an image is held, as a sync or a key would: the drag
/// still moves the image, not the characters that took its place.
#[test]
fn an_edit_during_a_drag_still_moves_the_image() {
    use super::super::input::{Location, Region};
    let md = format!("abc\n\n![a]({IMAGE})\n\nend here\n");
    let mut ws = editor(&md);
    let (from, to) = (image_center(&ws), after(&ws, "end"));
    let button = |pos, pressed| Egui::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    ws.enter_frame_with_input(vec![Egui::PointerMoved(from)]);
    ws.enter_frame_with_input(vec![button(from, true)]);
    ws.enter_frame_with_input(vec![Egui::PointerMoved(from + egui::vec2(20.0, 20.0))]);
    let grab = ws
        .editor
        .edit
        .in_progress_image_drag
        .as_ref()
        .expect("it lifted")
        .grab_offset;
    let lift = egui::vec2(0.0, grab.y + ws.editor.edit.renderer.drop_lift());
    ws.push(Event::Replace {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(3)),
        },
        text: "".into(),
        advance_cursor: false,
    });
    ws.enter_frame();
    ws.enter_frame_with_input(vec![Egui::PointerMoved(to)]);
    ws.enter_frame_with_input(vec![Egui::PointerMoved(to + lift)]);
    ws.enter_frame_with_input(vec![button(to + lift, false)]);
    ws.enter_frame();
    assert_eq!(ws.get_text(), format!("\n\n\n\nend![a]({IMAGE}) here\n"));
}

/// A drop the platform drives lands where the finger let go, whatever
/// pointer egui still tracks.
#[test]
fn a_platform_driven_drop_lands_where_the_finger_let_go() {
    let md = format!("caret\n\n| ![a]({IMAGE}) | b |\n| - | - |\n| c | d |\n");
    let mut ws = editor(&md);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();
    let (from, to, elsewhere) = (image_center(&ws), after(&ws, "| d"), after(&ws, "caret"));
    assert!(ws.editor.edit.reorder_start(from));
    ws.enter_frame();
    let grab = ws
        .editor
        .edit
        .in_progress_image_drag
        .as_ref()
        .expect("it lifted")
        .grab_offset;
    let lift = egui::vec2(0.0, grab.y + ws.editor.edit.renderer.drop_lift());
    ws.editor.edit.reorder_move(from + lift);
    ws.enter_frame();
    ws.editor.edit.reorder_move(to + lift);
    ws.enter_frame();
    ws.enter_frame_with_input(vec![Egui::PointerMoved(elsewhere)]);
    ws.editor.edit.reorder_end(to + lift, false);
    ws.enter_frame();
    ws.enter_frame();
    assert_eq!(ws.get_text(), format!("caret\n\n|  | b |\n| - | - |\n| c | d![a]({IMAGE}) |\n"));
}
