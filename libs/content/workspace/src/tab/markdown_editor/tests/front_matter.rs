//! The front matter grid's product rules, one table each: keys in the
//! grid, keys in source, when completions open, and how the block
//! collapses. The properties in `edit_props` carry typing into cells,
//! list items, row actions, and line endings.

use super::super::input::Event;
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
        (ab, Some((0, 4)), "x", "---\nx|a: 1\nb: 2\n---"),
        (ab, None, "x", "---\nx|a: 1\nb: 2\n---"),
        ("---\nk:\n---\n", c(6), "[{bs}", "---\nk: |\n---\n"),
        ("---\nk:\n---\n", c(6), "\"a\"", "---\nk: \"a\"|\n---\n"),
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
