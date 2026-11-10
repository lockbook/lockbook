//! Editing a note through an embed of it: a checkbox toggled in the embed
//! is one edit of the embedded note, made by whoever holds its buffer. An
//! embed of a heading of the note itself hands it to the note; an embed of
//! another note saves it.

use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::Grapheme;
use web_time::{Duration, Instant};

use crate::tab::markdown_editor::input::Event;
use crate::tab::{ExtendedInput as _, Markdown};
use crate::test_utils::ws::TestWs;

fn frames_until(t: &mut TestWs, done: impl Fn(&mut TestWs) -> bool) {
    let start = Instant::now();
    while !done(t) {
        assert!(start.elapsed() < Duration::from_secs(10), "never got there");
        t.frame();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn click(t: &mut TestWs, pos: Pos2) {
    let button = |pressed| Egui::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    t.frame_with(vec![Egui::PointerMoved(pos), button(true), button(false)]);
    t.frame();
}

fn md(t: &mut TestWs) -> &mut Markdown {
    t.ws.current_tab_mut().unwrap().markdown_mut().unwrap()
}

fn text(t: &mut TestWs) -> String {
    md(t).edit.renderer.buffer.current.text.clone()
}

fn disk(t: &TestWs, id: Uuid) -> String {
    String::from_utf8(t.ws.core.read_document(id, false).unwrap()).unwrap()
}

/// The checkboxes the current note's embeds show, once they show `n`
/// inside their slots and stay put for a frame: the embed has landed.
fn embedded_checkboxes(t: &mut TestWs, n: usize) -> Vec<Pos2> {
    let boxes = |t: &mut TestWs| {
        let embeds = &t
            .ws
            .current_tab_mut()?
            .markdown_mut()?
            .edit
            .renderer
            .tab_embeds;
        let (boxes, rects) = (embeds.shown_checkboxes(), embeds.rects());
        let inside = boxes
            .iter()
            .all(|(_, p)| rects.iter().any(|r| r.contains(*p)));
        let boxes: Vec<Pos2> = boxes.into_iter().map(|(_, pos)| pos).collect();
        (boxes.len() == n && inside).then_some(boxes)
    };
    let start = Instant::now();
    let mut last = None;
    loop {
        t.frame();
        let now = boxes(t);
        if let Some(boxes) = now.as_ref().filter(|_| now == last) {
            return boxes.clone();
        }
        last = now;
        assert!(start.elapsed() < Duration::from_secs(10), "the embeds never landed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Opens `note` and waits for its embeds to show `n` checkboxes.
fn open(t: &mut TestWs, note: Uuid, n: usize) -> Vec<Pos2> {
    t.ws.open_file(note, true, true);
    embedded_checkboxes(t, n)
}

/// A checkbox in an embed of another note checks it there: the note on
/// disk changes, the note embedding it doesn't, and nothing takes the caret.
#[test]
fn a_checkbox_in_an_embed_checks_the_embedded_note() {
    let mut t = TestWs::new();
    let plan = t.doc("Plan.md", "# Goals\n\n- [ ] chairs\n- [ ] tables\n");
    let note = t.doc("note.md", "above\n\n![[Plan]]\n\nbelow\n");
    let boxes = open(&mut t, note, 2);
    let (host_text, host_selection) =
        (text(&mut t), md(&mut t).edit.renderer.buffer.current.selection);

    click(&mut t, boxes[1]);
    frames_until(&mut t, |t| disk(t, plan) == "# Goals\n\n- [ ] chairs\n- [x] tables\n");
    assert_eq!(text(&mut t), host_text);
    assert_eq!(md(&mut t).edit.renderer.buffer.current.selection, host_selection);
    assert_eq!(t.ctx.memory(|m| m.focused()), Some(md(&mut t).id()));
}

/// The note open in a tab of its own, with an edit of its own not yet
/// saved, takes the change too, and both edits reach the disk.
#[test]
fn a_checkbox_in_an_embed_checks_the_notes_open_tab() {
    let mut t = TestWs::new();
    let plan = t.doc("Plan.md", "- [ ] chairs\n");
    let note = t.doc("note.md", "![[Plan]]\n");
    t.ws.open_file(plan, true, true);
    t.settle();
    let end = Grapheme(13);
    let typed = "- [ ] tables\n".to_string();
    let replace = Event::Replace { region: (end, end).into(), text: typed, advance_cursor: true };
    t.ctx.push_markdown_event(replace);
    t.frame();
    let boxes = open(&mut t, note, 1);
    click(&mut t, boxes[0]);
    let both = "- [x] chairs\n- [ ] tables\n";
    frames_until(&mut t, |t| {
        let tab =
            t.ws.tabs
                .values()
                .find(|tab| tab.destination.backing_file() == Some(plan));
        let shown = tab
            .and_then(|tab| tab.markdown())
            .map(|md| &md.edit.renderer.buffer.current.text);
        disk(t, plan) == both && shown.is_some_and(|text| text == both)
    });
}

/// A checkbox in an embed of the note's own heading is an edit of the note's
/// buffer: the same edit a click on the heading's own checkbox makes, undone
/// as one, and the embed shows it.
#[test]
fn a_checkbox_in_a_heading_embed_checks_the_note() {
    let mut t = TestWs::new();
    let before = "# Chairs\n\n- [ ] four\n- [ ] blue\n\n# Board\n\n![[#Chairs]]\n";
    let board = t.doc("Board.md", before);
    let boxes = open(&mut t, board, 2);
    click(&mut t, boxes[1]);
    let after = "# Chairs\n\n- [ ] four\n- [x] blue\n\n# Board\n\n![[#Chairs]]\n";
    assert_eq!(text(&mut t), after);
    // the same edit, made directly
    let direct = {
        let mut d = TestWs::new();
        let board = d.doc("Board.md", before);
        d.ws.open_file(board, true, true);
        d.settle();
        let check = md(&mut t).edit.renderer.tab_embeds.shown_checkboxes()[1].0;
        d.ctx.push_markdown_event(Event::ToggleTask { check });
        d.frame();
        text(&mut d)
    };
    assert_eq!(after, direct);
    t.frame();
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![after.to_string()]);

    t.ctx.push_markdown_event(Event::Undo);
    t.frame();
    t.frame();
    assert_eq!(text(&mut t), before);
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![before.to_string()]);
}

/// A heading embedded in a table cell, as a board's card, checks the same.
#[test]
fn a_checkbox_in_a_heading_embed_in_a_table_checks_the_note() {
    let mut t = TestWs::new();
    let board = t.doc(
        "Board.md",
        "| Todo | Done |\n| --- | --- |\n| ![[#Chairs]] | |\n\n# Chairs\n\n- [ ] four\n",
    );
    let boxes = open(&mut t, board, 1);
    click(&mut t, boxes[0]);
    assert!(text(&mut t).ends_with("# Chairs\n\n- [x] four\n"), "{}", text(&mut t));
}

/// On touch a tap on an embed's checkbox is egui's, not the platform text
/// view's, and checks it: nothing native is involved.
#[test]
fn a_tap_on_a_checkbox_in_an_embed_checks_it_on_touch() {
    use crate::tab::markdown_editor::TouchTarget;
    for os in [egui::os::OperatingSystem::IOS, egui::os::OperatingSystem::Android] {
        let mut t = TestWs::new();
        t.ctx.set_os(os);
        let board = t.doc("Board.md", "# Chairs\n\n- [ ] four\n\n# Board\n\n![[#Chairs]]\n");
        let boxes = open(&mut t, board, 1);
        let target = md(&mut t).edit.touch_target_at(boxes[0]);
        assert!(matches!(target, Some(TouchTarget::Popup)), "{os:?}: {target:?}");
        click(&mut t, boxes[0]);
        assert!(text(&mut t).starts_with("# Chairs\n\n- [x] four\n"), "{os:?}");
    }
}
