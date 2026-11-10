//! Editing a note through an embed of it: a checkbox toggled in the embed
//! is one edit of the embedded note, made by whoever holds its buffer. An
//! embed of a heading of the note itself hands it to the note; an embed of
//! another note saves it.

use egui::{Event as Egui, Modifiers, PointerButton, Pos2};
use lb_rs::Uuid;
use lb_rs::model::file::ShareMode;
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
/// card's band, retitled in the note. The caret stays in the card.
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
    // the blank line under the heading goes; the heading's own line stays
    let after = "# Chairs\nfour oak\n\ntwo blue\n\n# Board\n\n![[#Chairs]]\n";
    assert_eq!(text(&mut t), after);
    assert_eq!(md(&mut t).edit.renderer.tab_embeds.shown_texts(), vec![after.to_string()]);
    assert!(!host_focused(&mut t), "the card keeps the caret");
}

/// Up from a card's first line puts the caret before the embed in the note,
/// down from its last after it, and Escape selects the embed: each in one
/// press.
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
        // one press: the blank line under the heading isn't shown, so it's no stop
        t.frame_with(vec![key(key_pressed, Modifiers::NONE)]);
        t.frame();
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

/// Closing the note right after typing in an embed of another note still
/// saves what was typed.
#[test]
fn closing_the_note_saves_its_embeds() {
    let mut t = TestWs::new();
    let plan = t.doc("Plan.md", "Ship it.\n");
    let note = t.doc("note.md", "![[Plan]]\n");
    open(&mut t, note, 0);
    frames_until(&mut t, |t| !md(t).edit.renderer.tab_embeds.shown_words().is_empty());
    t.frame();
    let end = after_word(&mut t, "it.");
    click(&mut t, end);
    t.frame_with(vec![Egui::Text(" Today.".into())]);
    t.ws.close_tabs_for_dest(note);
    frames_until(&mut t, |t| disk(t, plan) == "Ship it. Today.\n");
}

/// In an embed of another note's heading, the caret can't wander into the
/// note's other sections: down out of the section it comes back to the
/// note around the embed.
#[test]
fn the_caret_leaves_an_embed_of_another_notes_heading() {
    let mut t = TestWs::new();
    let plan = "# Goals\n\nship\n\n# Risks\n\nnone\n";
    t.doc("Plan.md", plan);
    let note = "above\n\n![[Plan#Goals]]\n\nbelow\n";
    let id = t.doc("note.md", note);
    open(&mut t, id, 0);
    frames_until(&mut t, |t| !md(t).edit.renderer.tab_embeds.shown_words().is_empty());
    t.frame();
    let end = after_word(&mut t, "ship");
    click(&mut t, end);
    assert!(!host_focused(&mut t));
    for _ in 0..3 {
        t.frame_with(vec![key(egui::Key::ArrowDown, Modifiers::NONE)]);
        t.frame();
        if host_focused(&mut t) {
            break;
        }
    }
    assert!(host_focused(&mut t));
    let embed_end = Grapheme(note.find("]]").unwrap() + 2);
    assert_eq!(selection(&mut t), (embed_end, embed_end));
}

/// Another account's `team/` folder, shared to this one with `folder` and
/// linked into this account's root, holding `Board.md` (embedding `Plan`
/// and its own `Chairs`), shared on top with `board`. `Plan.md` is in the
/// folder too, or this account's own at its root when `plan_mine`.
/// Returns the board and Plan.
fn team(
    t: &mut TestWs, folder: ShareMode, board: Option<ShareMode>, plan_mine: bool,
) -> (Uuid, Uuid) {
    use crate::file_cache::FileCache;
    use crate::test_utils::ws::test_lb;
    use lb_rs::model::file_metadata::FileType;
    let owner = test_lb();
    let me = t.ws.core.get_account().unwrap().username.clone();
    let root = owner.get_root().unwrap().id;
    let dir = owner
        .create_file("team", &root, FileType::Folder)
        .unwrap()
        .id;
    let board_id = owner
        .create_file("Board.md", &dir, FileType::Document)
        .unwrap()
        .id;
    let plan_id = match plan_mine {
        true => t.doc("Plan.md", "- [ ] order\n"),
        false => {
            owner
                .create_file("Plan.md", &dir, FileType::Document)
                .unwrap()
                .id
        }
    };
    let text = "# Chairs\n\n- [ ] four\n\n# Board\n\n![[#Chairs]]\n\n![[Plan]]\n";
    owner.write_document(board_id, text.as_bytes()).unwrap();
    if !plan_mine {
        owner.write_document(plan_id, b"- [ ] order\n").unwrap();
    }
    owner.share_file(dir, &me, folder).unwrap();
    if let Some(mode) = board {
        owner.share_file(board_id, &me, mode).unwrap();
    }
    owner.sync().unwrap();
    t.ws.core.sync().unwrap();
    t.ws.core.create_link_at_path("/team", dir).unwrap();
    t.ws.core.sync().unwrap();
    let cache = FileCache::new(&t.ws.core).unwrap();
    t.ws.files.write().unwrap().replace(cache);
    (board_id, plan_id)
}

/// Whether the board's embed of `Plan` checks, and whether its own-heading
/// card does, each tried on a fresh board: (Plan's box, Chairs' box).
fn checks(folder: ShareMode, board: Option<ShareMode>, plan_mine: bool) -> (bool, bool) {
    let try_box = |item: &str| {
        let mut t = TestWs::new();
        let (board_id, _) = team(&mut t, folder, board, plan_mine);
        t.ws.open_file(board_id, true, true);
        let words = |t: &mut TestWs| {
            let md = t.ws.current_tab_mut()?.markdown_mut()?;
            let words = md.edit.renderer.tab_embeds.shown_words();
            let both = ["four", "order"]
                .iter()
                .all(|w| words.iter().any(|(s, _)| s.contains(w)));
            both.then_some(words)
        };
        // landed: both items shown, in the same place two frames running
        let (mut last, start) = (None, Instant::now());
        let words = loop {
            t.frame();
            let now = words(&mut t);
            if let Some(landed) = now.as_ref().filter(|_| now == last) {
                break landed.clone();
            }
            last = now;
            assert!(start.elapsed() < Duration::from_secs(10), "the board never landed");
            std::thread::sleep(Duration::from_millis(2));
        };
        // the box sits in the item's gutter, just left of its text
        let (_, rect) = words.iter().find(|(s, _)| s.contains(item)).unwrap();
        let shown = |t: &mut TestWs| (md(t).edit.renderer.tab_embeds.shown_texts(), text(t));
        let before = shown(&mut t);
        click(&mut t, rect.left_center() - egui::vec2(14.0, 0.0));
        t.frame();
        // a read-only embed in a read-only note opens on a click, editing nothing
        let still =
            t.ws.current_tab()
                .is_some_and(|tab| tab.destination.id() == board_id);
        still && shown(&mut t) != before
    };
    (try_box("order"), try_box("four"))
}

/// An embed of another note is edited by that note's permission, whatever
/// the note around it allows; a card of the note's own heading edits the
/// note, by the note's permission.
#[test]
fn an_embed_is_edited_by_its_own_files_permission() {
    use ShareMode::{Read, Write};
    // (folder, extra share on the board, Plan is this account's) → (Plan, Chairs)
    let rows = [
        // the board and Plan writable
        ((Write, None, false), (true, true)),
        // the board writable, Plan only readable
        ((Read, Some(Write), false), (false, true)),
        // the board only readable, Plan writable
        ((Read, None, true), (true, false)),
        // both only readable
        ((Read, None, false), (false, false)),
    ];
    for ((folder, board, mine), want) in rows {
        assert_eq!(checks(folder, board, mine), want, "{folder:?} {board:?} {mine}");
    }
}

/// A write to a file this account can only read is refused by lb-rs, not
/// only hidden by the embed.
#[test]
fn a_write_to_a_read_share_is_refused() {
    let mut t = TestWs::new();
    let (_, plan) = team(&mut t, ShareMode::Read, None, false);
    let hmac = t.ws.core.document_hmac(plan).unwrap();
    let write =
        t.ws.core
            .safe_write(plan, hmac, b"- [x] order\n".to_vec(), None);
    assert!(write.is_err(), "{write:?}");
    assert_eq!(disk(&t, plan), "- [ ] order\n");
}

/// Each level of nested embeds is edited by its own file's permission: a
/// writable note inside a read-only one inside a writable one still checks.
#[test]
fn a_nested_embed_is_edited_by_its_own_files_permission() {
    use crate::file_cache::FileCache;
    use crate::test_utils::ws::test_lb;
    use lb_rs::model::file_metadata::FileType;
    let mut t = TestWs::new();
    let leaf = t.doc("Leaf.md", "- [ ] deep\n");
    let host = t.doc("Host.md", "![[Mid]]\n");
    let owner = test_lb();
    let me = t.ws.core.get_account().unwrap().username.clone();
    let root = owner.get_root().unwrap().id;
    let dir = owner
        .create_file("team", &root, FileType::Folder)
        .unwrap()
        .id;
    let mid = owner
        .create_file("Mid.md", &dir, FileType::Document)
        .unwrap()
        .id;
    owner.write_document(mid, b"![[Leaf]]\n").unwrap();
    owner.share_file(dir, &me, ShareMode::Read).unwrap();
    owner.sync().unwrap();
    t.ws.core.sync().unwrap();
    t.ws.core.create_link_at_path("/team", dir).unwrap();
    t.ws.core.sync().unwrap();
    let cache = FileCache::new(&t.ws.core).unwrap();
    t.ws.files.write().unwrap().replace(cache);

    t.ws.open_file(host, true, true);
    // the leaf's item, two embeds down, once it has stopped moving
    let deep = |t: &mut TestWs| {
        let md = t.ws.current_tab_mut()?.markdown_mut()?;
        md.edit.renderer.tab_embeds.nested_word("deep")
    };
    let (mut last, start) = (None, Instant::now());
    let rect = loop {
        t.frame();
        let now = deep(&mut t);
        if let Some(landed) = now.filter(|_| now == last) {
            break landed;
        }
        last = now;
        assert!(start.elapsed() < Duration::from_secs(10), "the leaf never showed");
        std::thread::sleep(Duration::from_millis(2));
    };
    click(&mut t, rect.left_center() - egui::vec2(14.0, 0.0));
    frames_until(&mut t, |t| disk(t, leaf) == "- [x] deep\n");
    assert_eq!(disk(&t, mid), "![[Leaf]]\n");
}

/// A search preview shows its embeds and edits none, even of a note this
/// account owns; the same note open in a tab does.
#[test]
fn a_previews_embeds_edit_nothing() {
    let mut t = TestWs::new();
    t.doc("Plan.md", "- [ ] order\n");
    let note = t.doc("note.md", "![[Plan]]\n");
    let edits = |tab: &crate::tab::Tab| tab.markdown().map(|md| md.edit.renderer.tab_embeds.edits);
    t.ws.set_preview(Some(note));
    frames_until(&mut t, |t| {
        t.ws.promote_preview();
        t.ws.preview.as_ref().and_then(edits).is_some()
    });
    assert_eq!(t.ws.preview.as_ref().and_then(edits), Some(false));
    t.ws.open_file(note, true, true);
    frames_until(&mut t, |t| t.ws.current_tab().and_then(edits).is_some());
    assert_eq!(t.ws.current_tab().and_then(edits), Some(true));
}
