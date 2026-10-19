//! `Event::CopyRendered`: the selection as drawn (plain) plus its HTML.

use egui::{Key, Modifiers};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::tests::harness::{TestEditor, key_press};
use crate::tab::{CopiedRendered, ExtendedOutput as _};

fn select(ws: &mut TestEditor, start: usize, end: usize) {
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(start)),
            end: Location::Grapheme(Grapheme(end)),
        },
    });
    ws.enter_frame();
}

fn copy_rendered(ws: &mut TestEditor) -> CopiedRendered {
    ws.push(Event::CopyRendered);
    ws.enter_frame();
    ws.editor.edit.renderer.ctx.pop_copied_rendered().unwrap()
}

fn rendered(doc: &str) -> CopiedRendered {
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 0, doc.chars().count());
    copy_rendered(&mut ws)
}

#[test]
fn inline_code_span_drops_backticks() {
    let doc = "run `ls -la` now";
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 4, 12);
    assert_eq!(copy_rendered(&mut ws).plain, "ls -la");
}

#[test]
fn selection_across_inline_syntax() {
    assert_eq!(rendered("run `ls -la` **now**").plain, "run ls -la now");
}

#[test]
fn revealed_syntax_still_drops() {
    let doc = "run `ls -la` now";
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 6, 6); // caret inside the span reveals the backticks
    ws.enter_frame();
    select(&mut ws, 0, doc.len());
    assert_eq!(copy_rendered(&mut ws).plain, "run ls -la now");
}

#[test]
fn caret_copies_the_line() {
    let doc = "# Title\n\nbody `x`";
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 2, 2);
    assert_eq!(copy_rendered(&mut ws).plain, "Title");
    select(&mut ws, 12, 12);
    assert_eq!(copy_rendered(&mut ws).plain, "body x");
}

#[test]
fn review_cases() {
    assert_eq!(rendered("---\na: 1\n---\n\n# Title\n\ntext").plain, "a: 1\n\nTitle\n\ntext");
    assert_eq!(rendered("```\n\tx\n```").plain, "\tx");
    assert_eq!(rendered("don't -- \"ok\"").html, "don't -- &quot;ok&quot;");
    let doc = "| a | b |\n|---|---|\n| c | d |";
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 22, 23);
    assert_eq!(copy_rendered(&mut ws).plain, "c");
    select(&mut ws, 26, 27);
    assert_eq!(copy_rendered(&mut ws).plain, "d");
}

#[test]
fn blocks() {
    assert_eq!(rendered("# Title\n\ntext").plain, "Title\n\ntext");
    assert_eq!(rendered("- one\n- two\n  - three").plain, "- one\n- two\n  - three");
    assert_eq!(rendered("1. one\n2. two").plain, "1. one\n2. two");
    assert_eq!(rendered("- [ ] todo").plain, "- [ ] todo");
    assert_eq!(rendered("> quoted\n> lines").plain, "quoted\nlines");
    assert_eq!(rendered("```sh\necho hi\n```").plain, "echo hi");
    assert_eq!(rendered("    indented").plain, "indented");
    assert_eq!(rendered("[text](http://x) and [[note]]").plain, "text and note");
    assert_eq!(rendered("| a | b |\n|---|---|\n| c | d |").plain, "a\tb\nc\td");
    assert_eq!(rendered("one\n\n---\n\ntwo").plain, "one\n\n\n\ntwo");
}

#[test]
fn html_flavor() {
    assert_eq!(
        rendered("run `ls -la` **now**").html,
        "run <code>ls -la</code> <strong>now</strong>"
    );
    let html = rendered("one\n\ntwo").html;
    assert_eq!(html, "<p>one</p>\n<p>two</p>\n");
}

#[test]
fn shortcuts() {
    let doc = "run ls now";
    let mut ws = TestEditor::new(doc);
    select(&mut ws, 4, 6);
    ws.enter_frame_with_input(key_press(Key::C, Modifiers::COMMAND | Modifiers::SHIFT));
    assert_eq!(
        ws.editor
            .edit
            .renderer
            .ctx
            .pop_copied_rendered()
            .unwrap()
            .plain,
        "ls"
    );
    assert_eq!(ws.get_text(), doc);

    ws.enter_frame_with_input(key_press(Key::E, Modifiers::COMMAND));
    assert_eq!(ws.get_text(), "run `ls` now");
}

/// Desktop hosts deliver ⇧⌘C / ⇧⌘X as bare `Copy` / `Cut` events with
/// shift still held.
#[test]
fn shifted_copy_event_copies_rendered() {
    let mut ws = TestEditor::new("run `ls` now");
    select(&mut ws, 4, 8);
    let shifted = Modifiers::COMMAND | Modifiers::SHIFT;
    ws.enter_frame_with_input_held(vec![egui::Event::Copy], shifted);
    assert_eq!(
        ws.editor
            .edit
            .renderer
            .ctx
            .pop_copied_rendered()
            .unwrap()
            .plain,
        "ls"
    );

    ws.enter_frame_with_input_held(vec![egui::Event::Cut], shifted);
    assert_eq!(ws.get_text(), "run ~~`ls`~~ now");
}
