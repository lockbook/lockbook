use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;

/// The switch to source and back is viewing: the text, its unsaved edit,
/// the undo history and the outline survive, and a rename to a type with no
/// rendered form shows the same note as source, without an outline.
#[test]
fn source_switch_keeps_text_and_undo() {
    let doc = "# Title\n\nSome *text* here.\n";
    let mut ws = TestEditor::new(doc);
    ws.push(Event::Replace {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(0)),
        },
        text: "x".into(),
        advance_cursor: true,
    });
    ws.enter_frame();
    let edited = ws.get_text().to_string();

    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    assert!(ws.editor.shows_source());
    assert!(ws.editor.outline_offered);
    assert_eq!(ws.get_text(), edited);

    ws.editor.persistence.set_source(false);
    ws.enter_frame();
    assert!(!ws.editor.shows_source());
    ws.push(Event::Undo);
    ws.enter_frame();
    assert_eq!(ws.get_text(), doc);

    ws.editor.set_ext("txt");
    ws.enter_frame();
    assert!(ws.editor.shows_source());
    assert!(!ws.editor.outline_offered);
    assert_eq!(ws.get_text(), doc);
}

/// Source edits as markdown: a style toggles its syntax on and off, and a
/// link's text cmd-clicks open like its rendered form.
#[test]
fn source_note_edits_as_markdown() {
    use comrak::nodes::NodeValue;

    let doc = "one two [site](https://x.test)\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.persistence.set_source(true);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(4)),
            end: Location::Grapheme(Grapheme(4 + "two".len())),
        },
    });
    ws.enter_frame();
    for want in ["one **two** [site](https://x.test)\n", doc] {
        ws.push(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Strong });
        ws.enter_frame();
        assert_eq!(ws.get_text(), want);
    }

    let site = doc.find("site").unwrap();
    let pos = ws
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .find(|f| f.source_range.0.0 <= site && site < f.source_range.1.0)
        .expect("the link's text is laid out")
        .rect
        .center();
    let click = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::COMMAND,
    };
    for _ in 0..3 {
        ws.enter_frame_with_input_held(
            vec![egui::Event::PointerMoved(pos)],
            egui::Modifiers::COMMAND,
        );
    }
    ws.enter_frame_with_input_held(vec![click(true), click(false)], egui::Modifiers::COMMAND);
    ws.enter_frame();
    assert_eq!(ws.opened, ["https://x.test"]);
    assert_eq!(ws.get_text(), doc);
}

/// The toolbar's table inserts its markdown in source as when rendered.
#[test]
fn source_note_inserts_table() {
    use super::super::table::TableAction;

    let mut ws = TestEditor::new("text\n");
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    ws.push(Event::Table(TableAction::Insert { columns: 2, rows: 1 }));
    ws.enter_frame();
    assert!(ws.get_text().contains("| --- | --- |"), "{:?}", ws.get_text());
}

/// Enter in source continues a list like the rendered note does.
#[test]
fn source_note_continues_list() {
    let doc = "- one\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(5)),
            end: Location::Grapheme(Grapheme(5)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "- one\n- \n");
}
