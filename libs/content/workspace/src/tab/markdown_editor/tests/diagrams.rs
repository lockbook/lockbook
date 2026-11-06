//! Where a diagram's error points.

use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::Event;
use super::harness::TestEditor;

/// The text a revealed diagram's error marks.
fn marked(md: &str) -> Vec<String> {
    let mut ws = TestEditor::new(md);
    let caret = Grapheme(md.find("mermaid").unwrap());
    ws.push(Event::Select { region: (caret, caret).into() });
    ws.enter_frame();
    ws.enter_frame();
    let r = &ws.editor.edit.renderer;
    r.diagram_errors
        .iter()
        .map(|&e| r.buffer[e].to_string())
        .collect()
}

/// The span merman points at, read through a quote's prefixes.
#[test]
fn diagram_error_marks_its_span() {
    let md = "> ```mermaid\n> flowchart LR\n>     A --> B\n>     B -- > C\n> ```\n";
    assert_eq!(marked(md), ["> C"]);
}

/// Something missing at the end marks the last line with text.
#[test]
fn diagram_error_at_the_end_marks_the_last_line() {
    let md = "```mermaid\ngraph TD\n  A-->B\n  C-->\n\n```\n";
    assert_eq!(marked(md), ["C-->"]);
}
