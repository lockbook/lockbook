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

/// Tab and Return in a table move between cells, add a row, and leave the
/// table from a blank one in source as in the rendered note.
#[test]
fn source_note_tables_take_tab_and_return() {
    let doc = "| a | b |\n| - | - |\n| c | d |\n";
    let mut ws = TestEditor::new(doc);
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    let c = doc.find('c').unwrap();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(c)),
            end: Location::Grapheme(Grapheme(c)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();
    let d = doc.find('d').unwrap();
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (Grapheme(d), Grapheme(d + 1)));

    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "| a | b |\n| - | - |\n| c | d |\n|  |  |\n");

    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "| a | b |\n| - | - |\n| c | d |\n\n");

    // before a row's first cell, Return pushes the table down as text
    ws.push(Event::Select { region: (Grapheme(0), Grapheme(0)).into() });
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "\n| a | b |\n| - | - |\n| c | d |\n\n");
}

/// A hover card's note held to its rows switches to source and back.
#[test]
fn source_switch_in_a_card() {
    let mut ws = TestEditor::new("# Reading list\n\none\n\ntwo\n");
    ws.editor.embedded = true;
    ws.editor.in_card = true;
    ws.enter_frame();
    for source in [true, false] {
        ws.editor.persistence.set_source(source);
        ws.enter_frame();
        assert_eq!(ws.editor.shows_source(), source);
    }
}

/// A table comes from the keyboard alone, in source as rendered: ⌥⌘T
/// inserts one with the caret in its first cell, Return after a header row
/// makes that row a table's header, and Tab past a blank last row leaves.
#[test]
fn source_note_inserts_tables_from_the_keyboard() {
    let mut ws = TestEditor::new("Notes\n");
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(5)),
            end: Location::Grapheme(Grapheme(5)),
        },
    });
    ws.enter_frame();
    let alt_cmd = egui::Modifiers { alt: true, command: true, mac_cmd: true, ..Default::default() };
    ws.enter_frame_with_input(super::harness::key_press(egui::Key::T, alt_cmd));
    ws.enter_frame();
    assert_eq!(ws.get_text(), "Notes\n\n|  |  |\n| --- | --- |\n|  |  |\n");
    let first_cell = Grapheme("Notes\n\n| ".len());
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (first_cell, first_cell));

    let mut ws = TestEditor::new("| Name | Count |\n");
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(16)),
            end: Location::Grapheme(Grapheme(16)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "| Name | Count |\n| --- | --- |\n|  |  |\n");
    let first_cell = Grapheme("| Name | Count |\n| --- | --- |\n| ".len());
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (first_cell, first_cell));

    // Tab across the blank row and past its end leaves the table, as Return does
    for _ in 0..2 {
        ws.push(Event::Indent { deindent: false });
        ws.enter_frame();
    }
    assert_eq!(ws.get_text(), "| Name | Count |\n| --- | --- |\n\n");

    // a blank line ends the table only where the paragraph would run on into it
    for (md, after) in [
        ("| a |\nmore\n", "| a |\n| --- |\n|  |\n\nmore\n"),
        ("- | a |\n- next\n", "- | a |\n  | --- |\n  |  |\n- next\n"),
    ] {
        let mut ws = TestEditor::new(md);
        ws.editor.persistence.set_source(true);
        ws.enter_frame();
        let end = Grapheme(md.find('\n').unwrap());
        ws.push(Event::Select { region: (end, end).into() });
        ws.enter_frame();
        ws.push(Event::Newline { shift: false });
        ws.enter_frame();
        assert_eq!(ws.get_text(), after);
    }
}

/// Source shows a diagram as its fence, with no picture to select or delete.
#[test]
fn source_note_has_no_diagrams() {
    let mut ws = TestEditor::new("a\n\n```mermaid\nflowchart TD\n    A --> B\n```\n");
    ws.enter_frame();
    assert!(!ws.editor.edit.renderer.bounds.diagrams.is_empty());
    ws.editor.persistence.set_source(true);
    ws.enter_frame();
    assert!(ws.editor.edit.renderer.bounds.diagrams.is_empty());
}
