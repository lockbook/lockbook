//! Code in fences and code files.

use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;
use crate::tab::markdown_editor::Arena;

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

fn syntax(larger: bool) -> Event {
    Event::Select { region: Region::Syntax { larger } }
}

#[test]
fn expand_and_shrink_selection() {
    let doc = "```rust\nfn f() { g(1|, 2); }\n```";
    let args = "```rust\nfn f() { g|(1, 2)|; }\n```";
    let call = "```rust\nfn f() { |g(1, 2)|; }\n```";
    assert_eq!(run(doc, None, vec![syntax(true)]), "```rust\nfn f() { g(|1|, 2); }\n```");
    assert_eq!(run(doc, None, vec![syntax(true), syntax(true)]), args);
    assert_eq!(run(doc, None, vec![syntax(true), syntax(true), syntax(true)]), call);
    let back = vec![syntax(true), syntax(true), syntax(true), syntax(false)];
    assert_eq!(run(doc, None, back), args);
}

#[test]
fn shrink_forgets_a_moved_selection() {
    let doc = "```rust\nfn f() { g(1|, 2); }\n```";
    let moved = Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(8)),
            end: Location::Grapheme(Grapheme(8)),
        },
    };
    let events = vec![syntax(true), moved, syntax(false)];
    assert_eq!(run(doc, None, events), "```rust\n|fn f() { g(1, 2); }\n```");
}

#[test]
fn brackets_pair_past_strings() {
    let doc = "```js\nf(\")\", [1])|\n```";
    let mut ws = editor(doc, None);
    let arena = Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let pair = ws.editor.edit.bracket_pair(root).unwrap();
    let open = doc.find("f(").unwrap() + 1;
    let close = doc.find("])").unwrap() + 1;
    assert_eq!(
        pair,
        [(Grapheme(close), Grapheme(close + 1)), (Grapheme(open), Grapheme(open + 1))]
    );
}

#[test]
fn enter_with_a_selection_leaving_the_block() {
    let doc = "```rust\nfn a() {|\n}\n```\n\npara|";
    run(doc, None, vec![enter()]);
}

#[test]
fn expand_to_a_whole_crlf_file() {
    let doc = "fn a() {\r\n    |b();\r\n}\r\n";
    run(doc, Some("rs"), (0..8).map(|_| syntax(true)).collect());
}

#[test]
fn expand_from_a_crlf_comment() {
    let doc = "x = 1  # no|te\r\ny = 2\r\n";
    run(doc, Some("py"), (0..4).map(|_| syntax(true)).collect());
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

fn chord(doc: &str, key: egui::Key) -> String {
    let mut ws = editor(doc, None);
    ws.enter_frame_with_input(super::harness::key_press(
        key,
        egui::Modifiers::COMMAND | egui::Modifiers::ALT,
    ));
    shown(&ws)
}

#[test]
fn code_block_chord_fences_the_paragraph() {
    let doc = "a\n\nhel|lo\n\nb";
    let want = "a\n\n```\nhel|lo\n```\n\nb";
    assert_eq!(chord(doc, egui::Key::E), want);
}

#[test]
fn code_block_chord_unfences_a_code_block() {
    let doc = "```rust\nfn |f|() {}\n```\n\nb";
    let want = "fn |f|() {}\n\nb";
    assert_eq!(chord(doc, egui::Key::E), want);
}

#[test]
fn rule_chord_puts_a_rule_after_the_paragraph() {
    let doc = "hel|lo\n\nb";
    let want = "hel|lo\n\n---\n\nb";
    assert_eq!(chord(doc, egui::Key::R), want);
}

#[test]
fn rule_chord_puts_a_rule_after_the_code_block() {
    let doc = "```\nfn |f|()\n```\n\nb";
    let want = "```\nfn |f|()\n```\n---\n\nb";
    assert_eq!(chord(doc, egui::Key::R), want);
}

#[test]
fn code_block_chord_opens_an_empty_fence_on_a_blank_line() {
    let doc = "a\n\n|";
    let want = "a\n\n```\n|\n```";
    assert_eq!(chord(doc, egui::Key::E), want);
}

#[test]
fn rule_chord_puts_a_rule_right_under_a_heading() {
    assert_eq!(chord("# h|i\n\nb", egui::Key::R), "# h|i\n---\n\nb");
    assert_eq!(chord("# hi\n|", egui::Key::R), "# hi\n---|");
}

#[test]
fn rule_chord_on_a_blank_line_keeps_the_paragraph_above() {
    let doc = "a\n|";
    let want = "a\n\n---|";
    assert_eq!(chord(doc, egui::Key::R), want);
}

#[test]
fn rule_chord_after_a_selection_into_a_blank_line_leaves_the_text_whole() {
    let doc = "h|ello\n\n|";
    let want = "hello\n\n---|";
    assert_eq!(chord(doc, egui::Key::R), want);
}

#[test]
fn code_block_chord_fence_outlasts_a_fence_inside() {
    let doc = "x|y\n\n- b\n  ```\n  c\n  ```|";
    let want = "````\nx|y\n\n- b\n  ```\n  c\n  ```|\n````";
    assert_eq!(chord(doc, egui::Key::E), want);
}

#[test]
fn code_block_chord_on_a_blank_line_in_an_item_opens_a_fence_there() {
    let doc = "1. a\n\n   |\n2. b";
    let want = "1. a\n\n   ```\n   |\n   ```\n2. b";
    assert_eq!(chord(doc, egui::Key::E), want);
}

#[test]
fn code_block_chord_leaves_lines_outside_blocks_alone() {
    let doc = "---\ntitle: a|\n---\n\nbody";
    assert_eq!(chord(doc, egui::Key::E), doc);
}

#[test]
fn code_block_chord_fences_a_task_under_its_checkbox() {
    let doc = "- [ ] |task";
    let want = "- [ ]\n  ```\n  |task\n  ```";
    assert_eq!(chord(doc, egui::Key::E), want);
    let mut ws = editor(doc, None);
    for _ in 0..2 {
        ws.enter_frame_with_input(super::harness::key_press(
            egui::Key::E,
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        ));
    }
    assert_eq!(shown(&ws), doc);
}

#[test]
fn code_block_button_lights_inside_a_code_block() {
    let lit = |doc: &str| {
        let mut ws = editor(doc, None);
        let arena = Arena::new();
        let root = ws.editor.edit.renderer.reparse(&arena);
        let style = comrak::nodes::NodeValue::CodeBlock(Default::default());
        ws.editor.edit.unapply_block(root, &style)
    };
    assert!(lit("a\n\n```\nco|de\n```"));
    assert!(!lit("a|\n\n```\ncode\n```"));
}

#[test]
fn enter_in_a_task_fence_keeps_the_task_indent() {
    let mut ws = editor("- [ ]\n  ```\n  roller|\n  ```\n- [ ] trim", None);
    ws.enter_frame_with_input(super::harness::key_press(egui::Key::Enter, Default::default()));
    assert_eq!(shown(&ws), "- [ ]\n  ```\n  roller\n  |\n  ```\n- [ ] trim");
}

#[test]
fn backspace_selects_a_divider_then_deletes_it() {
    for (doc, selected) in
        [("a\n\n---|\n\nb", "a\n\n|---|\n\nb"), ("a\n\n---\n|b", "a\n\n|---|\nb")]
    {
        let mut ws = editor(doc, None);
        let backspace = || super::harness::key_press(egui::Key::Backspace, Default::default());
        ws.enter_frame_with_input(backspace());
        assert_eq!(shown(&ws), selected);
        ws.enter_frame_with_input(backspace());
        assert!(!ws.get_text().contains("---"), "{doc:?}");
    }
}

#[test]
fn backspace_at_a_fence_start_takes_the_fence_away() {
    let backspace = || super::harness::key_press(egui::Key::Backspace, Default::default());
    for (doc, want) in [
        ("```\n|code\n```", "|code"),
        ("- [ ]\n  ```\n  |roller\n  ```\n- [ ] trim", "- [ ] |roller\n- [ ] trim"),
    ] {
        let mut ws = editor(doc, None);
        ws.enter_frame_with_input(backspace());
        assert_eq!(shown(&ws), want);
    }
}
