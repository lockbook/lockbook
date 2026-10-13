//! Geometry and unit answers the iOS host relies on, checked headlessly.

use egui::Pos2;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;

fn select(ws: &mut TestEditor, g: usize) {
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(g)),
            end: Location::Grapheme(Grapheme(g)),
        },
    });
    ws.enter_frame();
}

/// A selection ending right after a newline has no glyph on the next line,
/// but the platform draws the end handle in the last rect, so that rect
/// sits on the next line where the caret would.
#[test]
fn selection_rects_end_on_a_blank_line() {
    let mut ws = TestEditor::new("aaa\n\nbbb\n\n\nccc");
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let [blank_top, _] = edit.cursor_line(Grapheme(9)).unwrap();
    let rects = edit.selection_rects((Grapheme(5), Grapheme(9)));
    let last = rects.last().unwrap();
    assert!(
        last.y_range().contains(blank_top.y + 4.0),
        "last rect {last:?} not on the blank line at y {}",
        blank_top.y
    );
    assert!((last.min.x - blank_top.x).abs() < 1.0);
    // A range ending mid-row adds nothing.
    assert_eq!(edit.selection_rects((Grapheme(5), Grapheme(7))).len(), 1);
    // A range ending before the newline adds nothing either.
    assert_eq!(edit.selection_rects((Grapheme(5), Grapheme(8))).len(), 1);
}

/// A tap past the end of a line lands after its trailing space, where the
/// next sentence starts, so the keyboard capitalizes what follows.
#[test]
fn tap_past_line_end_lands_after_trailing_space() {
    let md = "First sentence. \nSecond line";
    let mut ws = TestEditor::new(md);
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let [top, bottom] = edit.cursor_line(Grapheme(5)).unwrap();
    let p = Pos2::new(700.0, (top.y + bottom.y) / 2.0);
    let offset = edit.pos_to_char_offset(p);
    assert_eq!(offset, Grapheme("First sentence. ".len()), "tap past the line end");
}

fn scroll_to_bottom(ws: &mut TestEditor) {
    use super::super::scroll_content::DocScrollContent;
    use crate::widgets::affine_scroll::Action;
    let arena = comrak::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let content = DocScrollContent::for_frame(
        &ws.editor.edit.renderer,
        root,
        ws.editor.edit.scroll_area.state.viewport_height,
    );
    ws.editor
        .edit
        .scroll_area
        .state
        .handle(&content, Action::ScrollToBottom);
}

fn select_range(ws: &mut TestEditor, start: usize, end: usize) {
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(start)),
            end: Location::Grapheme(Grapheme(end)),
        },
    });
    ws.enter_frame();
}

const TALL: usize = 150;

fn tall_doc() -> String {
    (0..TALL)
        .map(|i| format!("paragraph {i} with a few words\n\n"))
        .collect()
}

/// A selection start beyond the band of laid-out rows still has exact
/// geometry: its row alone is laid out, above the band, so its rect leads
/// the selection rects and its points map back to it. Points between it
/// and the band resolve to the band's edge, as if the row were not there.
#[test]
fn far_selection_start_is_laid_out() {
    let md = tall_doc();
    let last = md.trim_end().len();
    let mut ws = TestEditor::new(&md);
    select_range(&mut ws, 0, last);
    scroll_to_bottom(&mut ws);
    ws.enter_frame();

    let edit = &ws.editor.edit;
    let frags = &edit.renderer.fragments;
    let band_top = frags
        .iter()
        .filter(|f| !f.far)
        .map(|f| f.rect.top())
        .fold(f32::INFINITY, f32::min);
    let band_first = frags
        .iter()
        .filter(|f| !f.far)
        .map(|f| f.source_range.start())
        .min()
        .unwrap();
    assert!(band_first > Grapheme(0), "the start should be beyond the band");
    let first_paragraph = md.find("\n\n").unwrap();
    assert!(
        frags
            .iter()
            .filter(|f| f.far)
            .all(|f| f.source_range.end().0 <= first_paragraph),
        "only the start's row is laid out beyond the band"
    );

    let [top, bottom] = edit
        .cursor_line(Grapheme(0))
        .expect("far start row laid out");
    assert!(bottom.y < band_top, "far row {top:?}-{bottom:?} not above the band top {band_top}");
    let mid = Pos2::new(top.x, (top.y + bottom.y) / 2.0);
    let rects = edit.selection_rects((Grapheme(0), Grapheme(last)));
    assert!(rects.first().unwrap().y_range().contains(mid.y), "first rect {:?}", rects.first());
    assert_eq!(edit.pos_to_char_offset(mid), Grapheme(0));

    let between = Pos2::new(top.x, (bottom.y + band_top) / 2.0);
    assert!(edit.pos_to_char_offset(between) >= band_first, "a point in the gap hit the far row");

    let [end_top, end_bottom] = edit.cursor_line(Grapheme(last)).expect("visible end");
    let end_mid = (end_top.y + end_bottom.y) / 2.0;
    assert!(rects.last().unwrap().y_range().contains(end_mid), "last rect {:?}", rects.last());
}

/// The mirror: a selection end beyond the band is laid out below it, and
/// maps back from its rect's trailing point, where UIKit re-anchors a
/// handle drag. An end on a blank line, between blocks or after the last
/// one, has a row too: the block whose spacing lays the line out.
#[test]
fn far_selection_end_is_laid_out() {
    let md = tall_doc();
    let between = md.match_indices("\n\n").nth(120).unwrap().0 + 1;
    for end in [md.trim_end().len(), between, md.chars().count()] {
        let mut ws = TestEditor::new(&md);
        // The cursor (the range's second end) stays at the top, so the view does.
        select_range(&mut ws, end, 0);
        ws.enter_frame();

        let edit = &ws.editor.edit;
        let band_bottom = edit
            .renderer
            .fragments
            .iter()
            .filter(|f| !f.far)
            .map(|f| f.rect.bottom())
            .fold(f32::NEG_INFINITY, f32::max);
        let [top, _] = edit
            .cursor_line(Grapheme(end))
            .expect("far end row laid out");
        assert!(
            top.y > band_bottom,
            "end {end}: far row at {} not below the band {band_bottom}",
            top.y
        );
        let rects = edit.selection_rects((Grapheme(0), Grapheme(end)));
        let last = *rects.last().unwrap();
        let trailing = Pos2::new(last.max.x, last.center().y);
        assert_eq!(
            edit.pos_to_char_offset(trailing),
            Grapheme(end),
            "end {end}: last rect {last:?}"
        );
    }
}

/// A platform's edit is applied before its frame, so the unfold the frame
/// does for an edit inside folded contents runs on that path too: a caret
/// left in a section, then folded over, types the section open again.
#[test]
fn platform_typing_inside_a_fold_unfolds_it() {
    use super::super::fold::FOLD_TAG;
    let doc = format!("# Heading {FOLD_TAG}\n\ntext\n\n");
    let mut ws = TestEditor::new(&doc);
    let end = doc.chars().count();
    select(&mut ws, end);
    assert!(ws.get_text().contains(FOLD_TAG));
    ws.editor.edit.apply_platform_event(Event::Replace {
        region: Region::Selection,
        text: "x".into(),
        advance_cursor: true,
    });
    ws.enter_frame();
    assert!(!ws.get_text().contains(FOLD_TAG), "{}", ws.get_text());
}

/// A tap to the right of a heading's fold chip places the caret after the
/// fold tag, at the end of the heading line, not before the chip.
#[test]
fn tap_right_of_a_fold_chip_lands_after_it() {
    use super::super::fold::FOLD_TAG;
    let heading = format!("# Heading {FOLD_TAG}");
    let doc = format!("{heading}\n\nhidden\n");
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    let edit = &ws.editor.edit;
    let tag_end = Grapheme(heading.chars().count());
    let [top, bottom] = edit.cursor_line(tag_end).unwrap();
    let p = Pos2::new(top.x + 30.0, (top.y + bottom.y) / 2.0);
    assert_eq!(edit.pos_to_char_offset(p), tag_end, "caret x after chip = {}", top.x);
}
