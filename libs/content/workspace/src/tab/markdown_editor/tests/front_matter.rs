//! The front matter grid's product rules, one table each: keys in the
//! grid, keys in source, when completions open, and how the block
//! collapses. The properties in `edit_props` carry typing into cells,
//! list items, row actions, and line endings.

use super::super::input::{Event, FrontMatterAction, Location, Region};
use super::harness::TestEditor;
use lb_rs::model::text::offset_types::Grapheme;

fn at(g: usize) -> Event {
    Event::Select { region: (Grapheme(g), Grapheme(g)).into() }
}

/// `keys` typed into `md` of a note in the vault, from the selection
/// `select`, or from where the note opens.
fn typing_over(md: &str, select: Option<(usize, usize)>, script: &str) -> String {
    let mut t = in_vault(md);
    if let Some((a, b)) = select {
        t.push(Event::Select { region: (Grapheme(a), Grapheme(b)).into() });
    }
    t.enter_frame();
    keys(&mut t, script);
    shown(&t)
}

/// Rule: a key in the grid edits one cell or one row, never the syntax
/// between them, and never joins two rows or a key and its value.
#[test]
fn keys_in_the_grid_edit_one_cell_or_row() {
    let c = |g: usize| Some((g, g));
    let hello = "---\ntitle: Hello\n---\n\nBody";
    let ab = "---\na: 1\nb: 2\n---";
    let rows = "---\na: 1\nb: two\nc: 3\n---\nBody";
    let cells = "---\nab: cd\nef: gh\n---\n";
    for (md, select, script, want) in [
        // Backspace at a value's or an item's start goes to the key, and on
        // an empty value takes the separator
        (hello, c(11), "{bs}", "---\ntitle|: Hello\n---\n\nBody"),
        ("---\ntags:\n  - a\n---\n", c(14), "{bs}", "---\ntags|:\n  - a\n---\n"),
        ("---\nkey: \n---", c(9), "{bs}", "---\nkey|\n---"),
        // a caret put down in a key's separator types into the value, but a
        // space typed at a key's end stays in the key
        ("---\ndue: x\n---", c(7), " date", "---\ndue date|: x\n---"),
        ("---\nk: v\n---\n", c(6), "x", "---\nk: x|v\n---\n"),
        ("---\nk: true\n---\n", c(6), "x", "---\nk: x|true\n---\n"),
        // Return starts a row; a colon, Return, or Tab ends a key
        (hello, c(16), "{ret}", "---\ntitle: Hello\n|\n---\n\nBody"),
        (hello, c(16), "{ret}status:x", "---\ntitle: Hello\nstatus: x|\n---\n\nBody"),
        ("---\nstatus\n---", c(10), "{Tab}", "---\nstatus: |\n---"),
        ("---\nkey:\n---", c(8), "v", "---\nkey: v|\n---"),
        // a span inside hidden syntax edits at its edge: Delete at a value's
        // end takes nothing, and a selection of the hidden end types into
        // the value
        ("---\nt: a \nb: c\n---\n", c(8), "{Delete}", "---\nt: a| \nb: c\n---\n"),
        ("---\nt: \"a\"\nb: c\n---\n", c(9), "{S-ArrowRight}x", "---\nt: \"ax|\"\nb: c\n---\n"),
        // Backspace in a flow list's first item leaves an empty item, then
        // takes it
        ("---\ntags: [x, a]\n---\n", c(12), "{bs}", "---\ntags: [|, a]\n---\n"),
        ("---\ntags: [x, a]\n---\n", c(12), "{bs}{bs}", "---\ntags: [|a]\n---\n"),
        // Return on a list's filled item starts a row after the list
        ("---\ntags: [work, home]\n---\n", c(20), "{ret}", "---\ntags: [work, home]\n|\n---\n"),
        // Tab goes by cell ends, key, value, the next key, and past the
        // last a new row; Shift+Tab back
        (ab, c(4), "{Tab}", "---\na: 1|\nb: 2\n---"),
        (ab, c(4), "{Tab}{Tab}", "---\na: 1\nb|: 2\n---"),
        (ab, c(4), "{Tab}{Tab}{Tab}", "---\na: 1\nb: 2|\n---"),
        (ab, c(4), "{Tab}{Tab}{Tab}{Tab}", "---\na: 1\nb: 2\n|\n---"),
        (ab, c(12), "{S-Tab}", "---\na: 1\nb|: 2\n---"),
        (ab, c(9), "{S-Tab}", "---\na: 1|\nb: 2\n---"),
        // ⌘→ and ⌘← go to a cell's end, then the next cell's; a row's last
        // cell holds it
        (cells, c(5), "{cmd-ArrowRight}", "---\nab|: cd\nef: gh\n---\n"),
        (cells, c(5), "{cmd-ArrowRight}{cmd-ArrowRight}", "---\nab: cd|\nef: gh\n---\n"),
        (
            cells,
            c(5),
            "{cmd-ArrowRight}{cmd-ArrowRight}{cmd-ArrowRight}",
            "---\nab: cd|\nef: gh\n---\n",
        ),
        (cells, c(9), "{cmd-ArrowLeft}", "---\nab: |cd\nef: gh\n---\n"),
        (cells, c(9), "{cmd-ArrowLeft}{cmd-ArrowLeft}", "---\n|ab: cd\nef: gh\n---\n"),
        // the last empty row: Return leaves for the body, Backspace on the
        // only one takes the block, and the line after it stays
        ("---\ntitle: x\n\n---\n\nBody", c(13), "{ret}", "---\ntitle: x\n---\n\n|Body"),
        ("---\n\n---\n\nBody", c(4), "{bs}", "|Body"),
        ("---\n\n---\n# Title\n\nBody", c(4), "{bs}", "|# Title\n\nBody"),
        // rows never join: Backspace at a row's start, or the body's, goes
        // to the end of the row above
        (ab, c(9), "{bs}", "---\na: 1|\nb: 2\n---"),
        ("---\na: 1\n---\n\nBody", c(13), "{bs}", "---\na: 1|\n---\n\nBody"),
        // at a row's start Return opens a row above and Backspace takes an
        // empty one there; the first row has none
        ("---\n\nstatus\n---\nBody", c(5), "{ret}", "---\n\n\n|status\n---\nBody"),
        ("---\n\nstatus\n---\nBody", c(5), "{bs}", "---\n|status\n---\nBody"),
        ("---\na: 1\nb: 2\n---\nBody", c(9), "{ret}", "---\na: 1\n\n|b: 2\n---\nBody"),
        ("---\na: 1\nb: 2\n---\nBody", c(4), "{bs}", "---\n|a: 1\nb: 2\n---\nBody"),
        // ⌘⌫ empties the cell, then the row, and ⌫ takes the row
        (rows, c(15), "{cmd-bs}", "---\na: 1\nb: |\nc: 3\n---\nBody"),
        (rows, c(15), "{cmd-bs}{cmd-bs}{bs}", "---\na: 1|\nc: 3\n---\nBody"),
        ("---\nk: [a, b]\n---\n", c(12), "{cmd-bs}", "---\nk: [a, |]\n---\n"),
        // ⌥↑ and ⌥↓ move the row, the caret with it
        (rows, c(15), "{alt-ArrowUp}", "---\nb: two|\na: 1\nc: 3\n---\nBody"),
        (rows, c(15), "{alt-ArrowDown}", "---\na: 1\nc: 3\nb: two|\n---\nBody"),
        // a selection through the closing fence takes the block from its
        // first row, else stops at the fence; one over the opening fence,
        // or typing as the note opens, types into the first key
        (rows, c(4), "{S-ArrowDown}{S-ArrowDown}{S-ArrowDown}{bs}", "|Body"),
        (rows, c(9), "{S-ArrowDown}{S-ArrowDown}{S-ArrowDown}{bs}", "---\na: 1\n|\n---\nBody"),
        (ab, Some((0, 4)), "x", "---\nx|a: 1\nb: 2\n---"),
        (ab, None, "x", "---\nx|a: 1\nb: 2\n---"),
        // a value's first keys pick its kind: `[` a flow list, `- ` a block
        // list, an empty `[]` goes; a comment's `#` is quoted and quotes
        // pair and step over
        ("---\nk:\n---\n", c(6), "[a", "---\nk: [a|]\n---\n"),
        ("---\nk:\n---\n", c(6), " - a", "---\nk:\n  - a|\n---\n"),
        ("---\nk:\n---\n", c(6), "[{bs}", "---\nk: |\n---\n"),
        ("---\nk:\n---\n", c(6), "#1", "---\nk: \"#1|\"\n---\n"),
        ("---\nk:\n---\n", c(6), "\"a\"", "---\nk: \"a\"|\n---\n"),
        ("---\ntags: \n---\n", c(10), "#x", "---\ntags: \"#x|\"\n---\n"),
        // Space flips a checkbox
        ("---\nk: true\n---\n", c(4), "{Tab} ", "---\nk: false|\n---\n"),
        // the list and task shortcuts change a row's kind, a list made text
        // joining its items as written; the inline ones would write
        // markdown into the YAML, so do nothing
        ("---\nk: a\n---\n", c(8), "{cmd-S-8}", "---\nk:\n  - a|\n---\n"),
        ("---\nk: a\n---\n", c(8), "{cmd-S-8}{cmd-S-8}", "---\nk: a|\n---\n"),
        ("---\nk:\n  - x\n  - \"y z\"\n---\n", c(4), "{cmd-S-8}", "---\nk: x, \"y z\"|\n---\n"),
        ("---\nk:\n---\n", c(4), "{cmd-S-8}", "---\nk:\n  - |\n---\n"),
        ("---\nk: a\n---\n", c(8), "{cmd-S-9}", "---\nk: false|\n---\n"),
        ("---\nk: a\n---\n", c(8), "{cmd-B}", "---\nk: a|\n---\n"),
        // ⌘; adds a property: a block when the note has none, else a row
        // after the caret's, else after the last
        ("# Title\n\nBody", c(0), "{cmd-;}", "---\n|\n---\n# Title\n\nBody"),
        ("# Title\n\nBody", c(0), "{cmd-;}tags", "---\ntags|\n---\n# Title\n\nBody"),
        ("---\na: 1\n---\n\nBody", c(14), "{cmd-;}", "---\na: 1\n|\n---\n\nBody"),
        ("---\na: 1\nb: 2\n---\n", c(4), "{cmd-;}", "---\na: 1\n|\nb: 2\n---\n"),
    ] {
        assert_eq!(typing_over(md, select, script), want, "{script} at {select:?} in {md:?}");
    }
}

/// Rule: in source the rows' Tab and Return are the grid's, the syntax is
/// text the caret goes through and Backspace takes, and a dash list goes
/// on as a markdown list does.
#[test]
fn keys_in_source_edit_the_yaml() {
    let hi = "---\ntitle: Hi\nstatus: done\n---\n\nBody";
    let flag = "---\ntitle: Hi\nflag: true\n---\n\nBody";
    let tags = "---\ntags:\n  - a\n---\n\nBody\n";
    let quoted = "---\ntags:\n  - \"a b\"\n---\n\nBody\n";
    for (md, at_, script, want) in [
        (hi, 13, "{Tab}", "---\ntitle: Hi\nstatus|: done\n---\n\nBody"),
        (hi, 13, "{Tab}{Tab}", "---\ntitle: Hi\nstatus: done|\n---\n\nBody"),
        (hi, 13, "{ret}", "---\ntitle: Hi\n|\nstatus: done\n---\n\nBody"),
        ("---\nkey\n---\n\nBody", 7, "{ret}", "---\nkey: |\n---\n\nBody"),
        (flag, 11, "{ArrowLeft}", "---\ntitle:| Hi\nflag: true\n---\n\nBody"),
        (flag, 11, "{bs}", "---\ntitle:|Hi\nflag: true\n---\n\nBody"),
        (flag, 13, "{cmd-ArrowLeft}", "---\n|title: Hi\nflag: true\n---\n\nBody"),
        // no box to flip
        (flag, 24, " ", "---\ntitle: Hi\nflag: true |\n---\n\nBody"),
        ("---\nkey\n---\n\nBody", 7, ": [a, b]", "---\nkey: [a, b]|\n---\n\nBody"),
        (tags, 15, "{ret}b{ret}", "---\ntags:\n  - a\n  - b\n  - |\n---\n\nBody\n"),
        (tags, 15, "{ret}{ret}", "---\ntags:\n  - a\n|\n---\n\nBody\n"),
        (tags, 15, "{ret}{bs}", "---\ntags:\n  - a|\n---\n\nBody\n"),
        (quoted, 19, "{ret}", "---\ntags:\n  - \"a b\"\n  - |\n---\n\nBody\n"),
    ] {
        assert_eq!(typing_source(md, at_, script), want, "{script} at {at_} in {md:?}");
    }
}

/// Rule: completions open on typing, or on a click, Tab, or new row
/// landing on an empty key or value, never on the arrows or a command, so
/// those keep moving; nothing is highlighted until one is picked or what is
/// typed is no candidate, so Return and Tab move on; the character that
/// ends a key or item takes the candidate it finishes.
#[test]
fn completions_open_on_typing_with_nothing_highlighted() {
    #[derive(Debug, PartialEq)]
    enum Popup {
        Shut,
        Open,
        Highlighted,
    }
    let popup = |t: &TestEditor| {
        let pc = &t.editor.edit.property_completions;
        match (pc.active && !pc.results.is_empty(), pc.highlighted()) {
            (false, _) => Popup::Shut,
            (true, None) => Popup::Open,
            (true, Some(_)) => Popup::Highlighted,
        }
    };
    let hi = "---\ntitle: Hi\n---\n\nBody";
    let status = "---\nstatus: \n---\n";
    for (md, at_, script, want) in [
        (hi, 13, "{ret}", Popup::Open),
        (hi, 13, "{ret}st", Popup::Highlighted),
        (hi, 13, "{ret}{ArrowDown}", Popup::Highlighted),
        (status, 4, "{Tab}", Popup::Open),
        (status, 10, "{ArrowRight}", Popup::Shut),
        ("---\nstatus: done\n---\n", 10, "{Tab}", Popup::Shut),
        ("---\na: 1\n---\n", 4, "{cmd-;}", Popup::Shut),
    ] {
        let mut t = in_vault(md);
        t.push(at(at_));
        t.enter_frame();
        keys(&mut t, script);
        assert_eq!(popup(&t), want, "{script} at {at_} in {md:?}: {}", shown(&t));
    }
    // a click on an empty value, as the app sends one: where it landed
    let md = "---\ntitle: Hi\nstatus: \n---\n";
    let mut t = in_vault(md);
    t.enter_frame();
    let [top, bottom] = t.editor.edit.cursor_line(Grapheme(22)).unwrap();
    let pos = egui::pos2(top.x + 1.0, (top.y + bottom.y) / 2.0);
    t.push(Event::Select { region: Region::Location(Location::Pos(pos)) });
    t.enter_frame();
    t.enter_frame();
    assert_eq!((shown(&t).as_str(), popup(&t)), ("---\ntitle: Hi\nstatus: |\n---\n", Popup::Open));
    // other notes' values, nearest note first, then count, then name
    let mut t = in_vault(status);
    t.push(at(4));
    t.enter_frame();
    keys(&mut t, "{Tab}");
    let results: Vec<&str> = t
        .editor
        .edit
        .property_completions
        .results
        .iter()
        .map(|c| c.text.as_str())
        .collect();
    assert_eq!(results, ["doing", "done", "far"]);

    let tags = "---\ntags: [home]\n---\n";
    for (md, at_, script, want) in [
        // nothing highlighted: Return and Tab move on
        (hi, 13, "{ret}{ret}", "---\ntitle: Hi\n---\n\n|Body"),
        (status, 4, "{Tab}{Tab}", "---\nstatus: \n|\n---\n"),
        ("---\nstatus:\n---\n", 11, " done{ret}", "---\nstatus: done\n|\n---\n"),
        // picked with the arrows or Ctrl+N
        ("---\nstatus:\n---\n", 11, " {ctrl-N}{ctrl-N}{ret}", "---\nstatus: done|\n---\n"),
        (hi, 13, "{ret}{ArrowDown}{ret}", "---\ntitle: Hi\nstatus: |\n---\n\nBody"),
        // the ending character takes the candidate it finishes; Return ends
        // a key too; in a list Return takes the candidate and a comma ends
        // the item as typed
        ("", 0, "{cmd-;}sta:", "---\nstatus: |\n---\n"),
        ("", 0, "{cmd-;}stage:", "---\nstage: |\n---\n"),
        ("", 0, "{cmd-;}status{ret}", "---\nstatus: |\n---\n"),
        ("", 0, "{cmd-;}sta{Tab}d{Tab}", "---\nstatus: doing|\n---\n"),
        (tags, 15, ",ar{ret},ar,", "---\ntags: [home, art, ar, |]\n---\n"),
        (
            "---\ntags:\n  - work\n---\n",
            18,
            ",ho{ret}{ret}",
            "---\ntags:\n  - work\n  - home\n|\n---\n",
        ),
    ] {
        let mut t = in_vault(md);
        t.push(at(at_));
        t.enter_frame();
        keys(&mut t, script);
        assert_eq!(shown(&t), want, "{script} at {at_} in {md:?}");
    }
    // the note's own index, saved as it is typed, is no candidate
    let mut t = in_vault("---\nstatus: do\n---\n");
    let own = crate::links::read_note("---\nstatus: don\n---\n");
    let id = t.editor.edit.file_id;
    t.editor
        .edit
        .renderer
        .files
        .write()
        .unwrap()
        .notes
        .insert(id, own);
    t.push(at(14));
    t.enter_frame();
    keys(&mut t, "n{Tab}");
    assert_eq!(shown(&t), "---\nstatus: done|\n---\n");
}

/// Rule: the shortcut and the toggle collapse the block to pills, moving a
/// caret in it to the body, every offset keeping a caret position; ↑ or a
/// pill opens it at a value's end, it stays open, and the body below moves
/// back.
#[test]
fn the_block_collapses_and_the_caret_opens_it() {
    let md = "---\ntitle: Hello\nstatus: done\ntags: [a, b]\n---\nA long body line here";
    let node_end = md.find("\n---\n").unwrap() + 4;
    let toggle = || Event::FrontMatter(FrontMatterAction::ToggleCollapsed);
    let body_top = |t: &TestEditor| {
        let body = Grapheme(md.len() - 1);
        t.editor
            .edit
            .renderer
            .fragment_at_offset(body)
            .unwrap()
            .rect
            .top()
    };
    // from a caret or a selection in the block
    for (shortcut, (a, b)) in [(true, (8, 8)), (false, (8, 8)), (false, (8, 13))] {
        let mut t = TestEditor::new(md);
        t.push(Event::Select { region: (Grapheme(a), Grapheme(b)).into() });
        t.enter_frame();
        let open = body_top(&t);
        if shortcut {
            keys(&mut t, "{cmd-S-D}");
        } else {
            t.push(toggle());
            t.enter_frame();
            t.enter_frame();
        }
        let r = &t.editor.edit.renderer;
        assert!(r.front_matter_collapsed);
        assert_eq!(r.buffer.current.selection.1, Grapheme(node_end + 1), "to the body");
        for o in 0..=node_end {
            assert!(r.fragment_at_offset(Grapheme(o)).is_some(), "no caret position at {o}");
        }
        assert!(body_top(&t) < open);
        t.push(at(md.len()));
        t.enter_frame();
        keys(&mut t, "{ArrowUp}");
        assert_eq!(shown(&t), md.replace("b]", "b|]"), "↑ lands at the last value's end");
        t.push(at(md.len()));
        t.enter_frame();
        t.enter_frame();
        let r = &t.editor.edit.renderer;
        assert!(!r.front_matter_collapsed, "it stays open");
        assert_eq!(body_top(&t), open);
    }
    // a pill opens its row at the value's end
    let mut t = TestEditor::new(md);
    t.push(at(md.len()));
    t.enter_frame();
    t.push(toggle());
    t.enter_frame();
    t.enter_frame();
    let status = Grapheme(md.find("status").unwrap());
    let pill = t
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .find(|f| {
            f.style_stack
                .last()
                .is_some_and(|s| s.chip && s.source_range.0 <= status && status < s.source_range.1)
        })
        .unwrap()
        .rect
        .center();
    for pressed in [true, false] {
        let button = egui::PointerButton::Primary;
        let modifiers = egui::Modifiers::NONE;
        t.enter_frame_with_input(vec![
            egui::Event::PointerMoved(pill),
            egui::Event::PointerButton { pos: pill, button, pressed, modifiers },
        ]);
    }
    t.enter_frame();
    t.enter_frame();
    assert!(!t.editor.edit.renderer.front_matter_collapsed);
    assert_eq!(shown(&t), md.replace("done", "done|"));
}

/// Rule: other notes' properties come from the note index, which reads the
/// vault only once something asks (#5267).
#[test]
fn typing_a_key_asks_for_the_note_index() {
    use crate::test_utils::files::{at as path_at, cache, tree};
    let files = tree("alice", &["/a/one.md", "/a/two.md"]);
    let one = path_at(&files, "/a/one.md");
    let md = "---\ntitle: Hi\n---\n";
    let mut t = TestEditor::in_files(cache(files), one, md);
    t.push(at(md.find("Hi").unwrap() + 2));
    t.enter_frame();
    keys(&mut t, "{ret}st");
    let files = t.editor.edit.renderer.files.read().unwrap();
    assert!(files.notes.is_wanted());
}

/// `one.md` open on `md`, beside notes whose front matter feeds
/// completions: two in its folder, one a folder away.
pub(super) fn in_vault(md: &str) -> TestEditor {
    in_vault_with(
        md,
        &[
            ("/a/two.md", "---\nstatus: done\ntags: [work, home]\nreviewed: true\n---\n"),
            ("/a/four.md", "---\nstatus: doing\ntags:\n  - work\n  - writing\n---\n"),
            ("/b/three.md", "---\nstatus: far\ntags: [art, workshop]\n---\n"),
        ],
    )
}

/// `md` open as `/a/one.md` beside `notes`.
pub(super) fn in_vault_with(md: &str, notes: &[(&str, &str)]) -> TestEditor {
    use crate::links::read_note;
    use crate::test_utils::files::{at as path_at, cache, tree};
    let mut paths = vec!["/a/one.md"];
    paths.extend(notes.iter().map(|(p, _)| *p));
    let files = tree("alice", &paths);
    let one = path_at(&files, "/a/one.md");
    let mut c = cache(files.clone());
    for (path, text) in notes {
        c.notes.insert(path_at(&files, path), read_note(text));
    }
    TestEditor::in_files(c, one, md)
}

/// Keystrokes as the desktop app delivers them, a frame each: characters
/// type, and `{tab}`-style names press keys, with `cmd-`, `alt-`, `ctrl-`,
/// and `S-` for modifiers.
pub(super) fn keys(t: &mut TestEditor, script: &str) {
    use egui::{Event as E, Key, Modifiers as M};
    let mut chars = script.chars();
    while let Some(c) = chars.next() {
        let events = if c == '{' {
            let name: String = chars.by_ref().take_while(|c| *c != '}').collect();
            let mut m = M::NONE;
            let mut key = name.as_str();
            for (prefix, modifier) in [
                ("cmd-", M::COMMAND | M::MAC_CMD),
                ("alt-", M::ALT),
                ("ctrl-", M::CTRL),
                ("S-", M::SHIFT),
            ] {
                if let Some(rest) = key.strip_prefix(prefix) {
                    (m, key) = (m | modifier, rest);
                }
            }
            let key = match key {
                ";" => Key::Semicolon,
                "8" => Key::Num8,
                "9" => Key::Num9,
                "bs" => Key::Backspace,
                "ret" => Key::Enter,
                other => Key::from_name(other).unwrap_or_else(|| panic!("no key {other}")),
            };
            let mut events = vec![E::ModifiersChanged(m)];
            events.extend(super::harness::key_press(key, m));
            events.push(E::ModifiersChanged(M::NONE));
            events
        } else {
            vec![E::Text(c.to_string())]
        };
        t.enter_frame_with_input(events);
        t.enter_frame_with_input(vec![]);
    }
}

/// The text with `|` at the caret.
pub(super) fn shown(t: &TestEditor) -> String {
    let r = &t.editor.edit.renderer;
    let at = r.buffer.current.selection.1.0;
    let byte = r.buffer.current.segs.offset_to_byte(Grapheme(at)).0;
    let text = &r.buffer.current.text;
    format!("{}|{}", &text[..byte], &text[byte..])
}

/// `keys` typed into `md`, shown as source, from offset `at_`.
fn typing_source(md: &str, at_: usize, script: &str) -> String {
    let mut t = in_vault(md);
    t.editor.persistence.set_source(true);
    t.push(at(at_));
    t.enter_frame();
    keys(&mut t, script);
    shown(&t)
}
