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

fn key(key: egui::Key, modifiers: Modifiers) -> Egui {
    Egui::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// A point just past the end of the embed's word `word`.
fn after_word(t: &mut TestWs, word: &str) -> Pos2 {
    let words = md(t).edit.renderer.tab_embeds.shown_words();
    let (_, rect) = words
        .iter()
        .find(|(text, _)| text.ends_with(word))
        .unwrap_or_else(|| panic!("no {word} in {words:?}"));
    rect.right_center() + egui::vec2(1.0, 0.0)
}

/// Just inside the start of the embed's word `word`.
fn before_word(t: &mut TestWs, word: &str) -> Pos2 {
    let words = md(t).edit.renderer.tab_embeds.shown_words();
    let (_, rect) = words
        .iter()
        .find(|(text, _)| text.starts_with(word))
        .unwrap();
    rect.left_center() + egui::vec2(1.0, 0.0)
}

fn host_focused(t: &mut TestWs) -> bool {
    let id = md(t).id();
    t.ctx.memory(|m| m.focused()) == Some(id)
}

fn selection(t: &mut TestWs) -> (Grapheme, Grapheme) {
    md(t).edit.renderer.buffer.current.selection
}

const BOARD: &str = "# Chairs\n\nfour oak\n\ntwo blue\n\n# Board\n\n![[#Chairs]]\n";

/// Opens a board whose card shows its Chairs section.
fn board(t: &mut TestWs) {
    let board = t.doc("Board.md", BOARD);
    open(t, board, 0);
    frames_until(t, |t| md(t).edit.renderer.tab_embeds.shown_words().len() >= 2);
    t.frame();
}

/// Typing in a heading card is typing in the note: the same text as the
/// same keys typed at that place in the note, and undone as one history,
/// from the card.
#[test]
fn typing_in_a_heading_card_types_in_the_note() {
    let mut t = TestWs::new();
    board(&mut t);
    let end = after_word(&mut t, "blue");
    click(&mut t, end);
    assert!(!host_focused(&mut t), "the card has the caret");
    t.frame_with(vec![Egui::Text(" chairs".into())]);
    t.frame();
    let typed = text(&mut t);

    let direct = {
        let mut d = TestWs::new();
        let board = d.doc("Board.md", BOARD);
        d.ws.open_file(board, true, true);
        d.settle();
        let at = Grapheme(BOARD.find("two blue").unwrap() + "two blue".len());
        d.ctx
            .push_markdown_event(Event::Select { region: (at, at).into() });
        d.frame();
        d.frame_with(vec![Egui::Text(" chairs".into())]);
        d.frame();
        text(&mut d)
    };
    assert_eq!(typed, direct);
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![typed.clone()]);

    t.frame_with(vec![key(egui::Key::Z, Modifiers::COMMAND)]);
    t.frame();
    t.frame();
    assert_eq!(text(&mut t), BOARD);
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![BOARD.to_string()]);
}

/// Backspace can't join a card's text into its heading: the heading is the
/// card's band, retitled in the note.
#[test]
fn a_heading_card_keeps_its_heading() {
    let mut t = TestWs::new();
    board(&mut t);
    let start = before_word(&mut t, "four");
    click(&mut t, start);
    let backspace = || key(egui::Key::Backspace, Modifiers::NONE);
    for _ in 0..4 {
        t.frame_with(vec![backspace()]);
    }
    t.frame();
    let after = text(&mut t);
    assert!(after.starts_with("# Chairs\n"), "{after:?}");
    assert!(after.contains("four oak"), "{after:?}");
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![after]);
}

/// Up from a card's first line puts the caret before the embed in the note,
/// down from its last after it, and Escape selects the embed.
#[test]
fn the_caret_leaves_a_heading_card_around_its_embed() {
    let embed = Grapheme(BOARD.find("![[").unwrap());
    let embed_end = Grapheme(BOARD.len() - 1);
    for (word, key_pressed, want) in [
        ("four", egui::Key::ArrowUp, (embed, embed)),
        ("blue", egui::Key::ArrowDown, (embed_end, embed_end)),
        ("blue", egui::Key::Escape, (embed, embed_end)),
    ] {
        let mut t = TestWs::new();
        board(&mut t);
        let at = after_word(&mut t, word);
        click(&mut t, at);
        for _ in 0..2 {
            t.frame_with(vec![key(key_pressed, Modifiers::NONE)]);
            t.frame();
            if host_focused(&mut t) {
                break;
            }
        }
        assert!(host_focused(&mut t), "{key_pressed:?}");
        assert_eq!(selection(&mut t), want, "{key_pressed:?}");
        assert_eq!(text(&mut t), BOARD);
    }
}

/// Typing in an embed of another note edits that note and saves it, as its
/// own tab would.
#[test]
fn typing_in_an_embed_of_another_note_saves_it() {
    let mut t = TestWs::new();
    let plan = t.doc("Plan.md", "Ship it.\n");
    let note = t.doc("note.md", "above\n\n![[Plan]]\n\nbelow\n");
    open(&mut t, note, 0);
    frames_until(&mut t, |t| !md(t).edit.renderer.tab_embeds.shown_words().is_empty());
    t.frame();
    let end = after_word(&mut t, "it.");
    click(&mut t, end);
    t.frame_with(vec![Egui::Text(" Today.".into())]);
    frames_until(&mut t, |t| disk(t, plan) == "Ship it. Today.\n");
    assert_eq!(text(&mut t), "above\n\n![[Plan]]\n\nbelow\n");
}

/// The toolbar acts on the embed with the caret: what it inserts lands at
/// the card's caret, and its undo takes it back in the note's history.
#[test]
fn the_toolbar_acts_on_the_card_with_the_caret() {
    use crate::tab::markdown_editor::input::Region;
    let mut t = TestWs::new();
    board(&mut t);
    let end = after_word(&mut t, "blue");
    click(&mut t, end);
    let text_in =
        Event::Replace { region: Region::Selection, text: "s".into(), advance_cursor: true };
    t.ctx.push_markdown_event(text_in);
    t.frame();
    t.frame();
    assert!(text(&mut t).contains("two blues\n"), "{}", text(&mut t));
    t.ctx.push_markdown_event(Event::Undo);
    t.frame();
    t.frame();
    assert_eq!(text(&mut t), BOARD);
}
