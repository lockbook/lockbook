use lb_rs::model::text::offset_types::Grapheme;

use super::harness::TestEditor;
use crate::spelling::{SpellCheck, Spelling, install};
use crate::tab::markdown_editor::input::{Event, Location, Region};

/// Knows one misspelling.
struct Teh;

impl SpellCheck for Teh {
    fn check(&self, word: &str) -> Option<bool> {
        Some(word != "teh")
    }
    fn suggest(&self, _word: &str) -> Vec<String> {
        vec!["the".into()]
    }
    fn learn(&self, _word: &str) {}
    fn learned(&self, _word: &str) -> bool {
        false
    }
    fn unlearn(&self, _word: &str) {}
}

/// The words marked after a frame with the caret at `caret`.
fn marked(md: &str, caret: usize) -> Vec<(usize, String)> {
    marked_in(md, caret, false)
}

/// The words marked after a frame with the caret at `caret`, in source view
/// if `source`.
fn marked_in(md: &str, caret: usize, source: bool) -> Vec<(usize, String)> {
    let mut ws = TestEditor::new(md);
    install(&ws.editor.edit.renderer.ctx, Some(Spelling::new(Box::new(Teh))));
    ws.editor.persistence.set_source(source);
    let at = Location::Grapheme(Grapheme(caret));
    ws.push(Event::Select { region: Region::BetweenLocations { start: at, end: at } });
    ws.enter_frame();
    ws.enter_frame();
    let r = &ws.editor.edit.renderer;
    r.misspelled
        .iter()
        .map(|&w| (w.0.0, r.buffer[w].to_string()))
        .collect()
}

#[test]
fn marks_prose_only() {
    let md = "teh `teh` [teh](https://teh.com) #teh @teh #to-teh #a/teh @b-teh x-teh https://teh.example\n\n```\nteh\n```\n";
    assert_eq!(
        marked(md, md.len()),
        vec![(0, "teh".into()), (11, "teh".into()), (67, "teh".into())]
    );
}

#[test]
fn source_marks_what_rendered_marks() {
    let md =
        "# teh\n\n- **teh** `teh` [teh](https://teh.com) https://teh.example\n\n```\nteh\n```\n";
    let words = vec![(2, "teh".into()), (11, "teh".into()), (24, "teh".into())];
    assert_eq!(marked_in(md, md.len(), false), words, "rendered");
    assert_eq!(marked_in(md, md.len(), true), words, "source");
}

#[test]
fn typed_word_waits() {
    let mut ws = TestEditor::new("te");
    install(&ws.editor.edit.renderer.ctx, Some(Spelling::new(Box::new(Teh))));
    let at = |g| Location::Grapheme(Grapheme(g));
    ws.push(Event::Select { region: Region::BetweenLocations { start: at(2), end: at(2) } });
    ws.enter_frame();
    ws.push(Event::Replace { region: Region::Selection, text: "h".into(), advance_cursor: true });
    ws.enter_frame();
    assert!(ws.editor.edit.renderer.misspelled.is_empty(), "typing");
    ws.push(Event::Select { region: Region::BetweenLocations { start: at(1), end: at(1) } });
    ws.enter_frame();
    assert_eq!(ws.editor.edit.renderer.misspelled.len(), 1, "a caret placed in it");
}

#[test]
fn recheck_asks_again_and_keeps_ignored() {
    let spelling = Spelling::host();
    assert_eq!(spelling.check("teh"), None);
    assert_eq!(spelling.take_requests(), ["teh"]);
    spelling.answer("teh", true);
    spelling.ignore("blorpt");
    spelling.recheck();
    assert_eq!(spelling.check("teh"), Some(true), "keeps its answer while asked");
    assert_eq!(spelling.check("blorpt"), Some(true));
    assert_eq!(spelling.take_requests(), ["teh"]);
    spelling.answer("teh", false);
    assert_eq!(spelling.check("teh"), Some(false));
    assert!(spelling.take_requests().is_empty());
}

#[test]
fn unlearn_asks_again() {
    let spelling = Spelling::host();
    spelling.check("snorkid");
    spelling.take_requests();
    spelling.answer("snorkid", true);
    spelling.unlearn("snorkid");
    assert_eq!(spelling.check("snorkid"), None);
    assert_eq!(spelling.take_requests(), ["snorkid"]);
}

#[test]
fn placed_caret_composing_stays_marked() {
    let mut ws = TestEditor::new("teh cat");
    install(&ws.editor.edit.renderer.ctx, Some(Spelling::new(Box::new(Teh))));
    let at = Location::Grapheme(Grapheme(1));
    ws.push(Event::Select { region: Region::BetweenLocations { start: at, end: at } });
    ws.editor.edit.renderer.platform_marked = Some((Grapheme(0), Grapheme(3)));
    ws.enter_frame();
    ws.enter_frame();
    assert_eq!(ws.editor.edit.renderer.misspelled, vec![(Grapheme(0), Grapheme(3))]);
}
