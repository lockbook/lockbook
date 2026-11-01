//! Code in fences and code files.

use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;

/// `doc` with its `|` as the caret (or two `|`s as a selection), as a note
/// or, given an extension, a file.
fn editor(doc: &str, ext: Option<&str>) -> TestEditor {
    let start = doc.find('|').unwrap();
    let end = doc.rfind('|').unwrap();
    let end = if end == start { start } else { end - 1 };
    let text = doc.replace('|', "");
    let mut ws = match ext {
        Some(ext) => TestEditor::file(&text, ext),
        None => TestEditor::new(&text),
    };
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(start)),
            end: Location::Grapheme(Grapheme(end)),
        },
    });
    ws.enter_frame();
    ws
}

/// The text with `|` where the caret is.
fn shown(ws: &TestEditor) -> String {
    let (start, end) = ws.editor.edit.renderer.buffer.current.selection;
    let mut text = ws.get_text().to_string();
    if start != end {
        text.insert(end.0, '|');
    }
    text.insert(start.0, '|');
    text
}

fn run(doc: &str, ext: Option<&str>, events: Vec<Event>) -> String {
    let mut ws = editor(doc, ext);
    for event in events {
        ws.push(event);
        ws.enter_frame();
    }
    shown(&ws)
}

fn enter() -> Event {
    Event::Newline { shift: false }
}

fn tab(deindent: bool) -> Event {
    Event::Indent { deindent }
}

fn typed(text: &str) -> Event {
    Event::Replace { region: Region::Selection, text: text.into(), advance_cursor: true }
}

#[test]
fn enter_keeps_indent() {
    let doc = "```rust\nfn f() {\n    let x = 1;|\n}\n```";
    let want = "```rust\nfn f() {\n    let x = 1;\n    |\n}\n```";
    assert_eq!(run(doc, None, vec![enter()]), want);
}

#[test]
fn enter_after_open_bracket_indents() {
    let doc = "```js\nif (x) {|\n```";
    let want = "```js\nif (x) {\n    |\n```";
    assert_eq!(run(doc, None, vec![enter()]), want);
}

#[test]
fn enter_between_brackets_puts_close_on_its_own_line() {
    let doc = "```rust\nfn f() {|}\n```";
    let want = "```rust\nfn f() {\n    |\n}\n```";
    assert_eq!(run(doc, None, vec![enter()]), want);
}

#[test]
fn enter_in_quoted_fence_keeps_the_quote() {
    let doc = "> ```js\n> f({|\n> ```";
    let want = "> ```js\n> f({\n>     |\n> ```";
    assert_eq!(run(doc, None, vec![enter()]), want);
}

#[test]
fn bracket_in_a_string_does_not_indent() {
    let doc = "```js\nlet s = \"{\";|\n```";
    let want = "```js\nlet s = \"{\";\n|\n```";
    assert_eq!(run(doc, None, vec![enter()]), want);
}

#[test]
fn enter_in_code_file() {
    let doc = "fn f() {|}";
    let want = "fn f() {\n    |\n}";
    assert_eq!(run(doc, Some("rs"), vec![enter()]), want);
}

#[test]
fn tab_indents_by_the_code_unit() {
    let doc = "```yaml\na:\n  b: 1\n|\n```";
    let want = "```yaml\na:\n  b: 1\n  |\n```";
    assert_eq!(run(doc, None, vec![tab(false)]), want);
}

#[test]
fn tab_and_shift_tab_move_selected_lines() {
    let doc = "```py\n|a = 1\n\nb = 2|\n```";
    let indented = "```py\n    |a = 1\n\n    b = 2|\n```";
    assert_eq!(run(doc, None, vec![tab(false)]), indented);
    assert_eq!(run(doc, None, vec![tab(false), tab(true)]), doc);
}

#[test]
fn shift_tab_undoes_tab_on_the_only_indented_lines() {
    let doc = "```rust\nfn f() {\n    |a();\n    b();|\n}\n```";
    assert_eq!(run(doc, None, vec![tab(false), tab(true)]), doc);
}

#[test]
fn close_bracket_outdents_to_its_match() {
    let doc = "```rust\nfn f() {\n    if x {\n        y();\n        |\n```";
    let want = "```rust\nfn f() {\n    if x {\n        y();\n    }|\n```";
    assert_eq!(run(doc, None, vec![typed("}")]), want);
}

#[test]
fn enter_with_a_selection_leaving_the_block() {
    let doc = "```rust\nfn a() {|\n}\n```\n\npara|";
    run(doc, None, vec![enter()]);
}

#[test]
fn highlight_edge_inside_a_grapheme() {
    let doc = "```python\nprint(1\u{fe0f}\u{20e3})\n```\n|";
    run(doc, None, vec![]);
}

#[test]
fn close_bracket_in_a_string_is_typed_as_is() {
    let doc = "```python\nf(\"\"\"\n    |\n\"\"\")\n```";
    let want = "```python\nf(\"\"\"\n    )|\n\"\"\")\n```";
    assert_eq!(run(doc, None, vec![typed(")")]), want);
}
