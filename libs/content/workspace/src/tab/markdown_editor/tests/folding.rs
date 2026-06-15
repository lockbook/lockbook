//! Exact-behavior tests for section folding: the `···` chip, fold tags
//! never rendering as source, newline-after-fold, navigation treating the
//! folded region as cursored-past, and edits against it unfolding it.

use lb_rs::model::text::offset_types::Grapheme;

use super::super::fold::FOLD_TAG;
use super::super::input::{Advance, Bound, Event, Increment, Location, Region};
use super::harness::TestEditor;
use super::render_props::{render_frame, test_renderer};

fn select_at(offset: usize) -> Event {
    Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(offset)),
            end: Location::Grapheme(Grapheme(offset)),
        },
    }
}

fn char_arrow(backwards: bool) -> Event {
    Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Char),
            backwards,
            extend_selection: false,
        },
    }
}

/// `(doc, tag_start, tag_end)` for a folded H1 with a one-line body.
fn folded_heading_doc() -> (String, usize, usize) {
    let doc = format!("# a{FOLD_TAG}\nbody");
    let tag_start = doc.find("<!--").unwrap();
    let tag_end = tag_start + FOLD_TAG.len();
    (doc, tag_start, tag_end)
}

/// The fold tag renders as an atomic chip — never as its source text —
/// even when the cursor reveals the heading line's syntax.
#[test]
fn fold_tag_never_renders_as_source() {
    let (doc, tag_start, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    for offset in [0, 3, tag_end] {
        ws.push(select_at(offset));
        ws.enter_frame();

        let fragments = &ws.editor.edit.renderer.fragments;
        let chip = fragments
            .iter()
            .find(|f| f.source_range == (Grapheme(tag_start), Grapheme(tag_end)))
            .unwrap_or_else(|| panic!("no chip fragment with cursor at {offset}"));
        assert!(chip.atomic, "chip not atomic with cursor at {offset}");
        assert!(chip.interaction.is_some(), "chip not clickable with cursor at {offset}");
        assert!(chip.rect.width() > 0.0, "chip has no visible extent with cursor at {offset}");
        for f in fragments {
            let (s, e) = f.source_range;
            let strictly_inside = s.0 >= tag_start && e.0 <= tag_end && e > s;
            if strictly_inside && !f.atomic {
                panic!("tag rendered as source text with cursor at {offset}: {:?}", f.source_range);
            }
        }
    }
    assert!(ws.get_text().contains(FOLD_TAG), "cursor on the heading line must not unfold");
}

/// Folded contents produce no fragments; a find match within them
/// reveals them without removing the fold tag.
#[test]
fn find_match_reveals_folded_contents_without_unfolding() {
    let (doc, _, _) = folded_heading_doc();
    let body = doc.find("body").unwrap();
    let covers_body = |r: &crate::tab::markdown_editor::MdRender| {
        r.fragments.iter().any(|f| {
            f.source_range.0.0 >= body
                && f.source_range.1.0 <= body + 4
                && f.source_range.1 > f.source_range.0
        })
    };

    let mut r = test_renderer(&doc);
    assert!(!render_frame(&mut r, 800.0, None, |r| covers_body(r)), "folded body rendered");

    let mut r = test_renderer(&doc);
    r.find_current_match = Some((Grapheme(body), Grapheme(body + 4)));
    assert!(render_frame(&mut r, 800.0, None, |r| covers_body(r)), "find match didn't reveal");
    assert!(r.buffer.current.text.contains(FOLD_TAG), "find reveal must not unfold");
}

/// Enter with the cursor right of the chip creates a new heading of the
/// same level *after* the folded contents, with the cursor on it.
#[test]
fn newline_right_of_chip_creates_heading_after_folded_content() {
    let doc = format!("## a{FOLD_TAG}\nbody one\nbody two\n## b");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();

    let expected = format!("## a{FOLD_TAG}\nbody one\nbody two\n## \n## b");
    assert_eq!(ws.get_text(), expected);
    let cursor = expected.find("## \n").unwrap() + 3;
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(cursor), Grapheme(cursor)),
    );
    assert!(ws.get_text().contains(FOLD_TAG), "newline-after must not unfold");
}

/// Enter with the cursor right of a folded item's chip creates a new
/// item *after* the folded subtree.
#[test]
fn newline_right_of_chip_creates_item_after_folded_subtree() {
    let doc = format!("* a{FOLD_TAG}\n  * b\n* c");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();

    let expected = format!("* a{FOLD_TAG}\n  * b\n* \n* c");
    assert_eq!(ws.get_text(), expected);
    let cursor = expected.find("* \n").unwrap() + 2;
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(cursor), Grapheme(cursor)),
    );
}

/// The chip stands for the folded text: right-arrow at the end of the
/// heading text first skips the tag atom, then steps *past* the hidden
/// contents to the next visible position — never into them.
#[test]
fn arrow_right_skips_chip_then_skips_section() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let tag_start = doc.find("<!--").unwrap();
    let tag_end = tag_start + FOLD_TAG.len();
    let next_section = doc.find("\n# b").unwrap() + 1;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_start));
    ws.enter_frame();
    ws.push(char_arrow(false));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
        "right-arrow should skip the chip atom"
    );

    ws.push(char_arrow(false));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(next_section), Grapheme(next_section)),
        "right-arrow right of the chip should step past the folded contents"
    );
    assert!(ws.get_text().contains(FOLD_TAG), "arrowing past must not unfold");
}

/// A section folded through the end of the doc has no visible position
/// after it; right-arrow right of the chip stays put.
#[test]
fn arrow_right_at_doc_end_section_stays_put() {
    let (doc, _, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(char_arrow(false));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
    );
    assert!(ws.get_text().contains(FOLD_TAG));
}

/// Left-arrow mirrors the skips: from the line after the section it lands
/// right of the chip, and from there left of the chip.
#[test]
fn arrow_left_skips_section_then_chip() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let tag_start = doc.find("<!--").unwrap();
    let tag_end = tag_start + FOLD_TAG.len();
    let next_section = doc.find("\n# b").unwrap() + 1;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(next_section));
    ws.enter_frame();
    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
        "left-arrow should land right of the chip, past the hidden contents"
    );

    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_start), Grapheme(tag_start)),
    );
    assert!(ws.get_text().contains(FOLD_TAG), "left-arrow must not unfold");
}

/// Blank lines between a folded section and the next block are boundary,
/// not contents: they render as visible spacing rows, so arrows rest on
/// them — in both directions — and a cursor there leaves the fold alone.
#[test]
fn blank_line_after_folded_section_is_walkable() {
    let doc = format!("# h{FOLD_TAG}\np\n\n#");
    let tag_end = doc.find("-->").unwrap() + 3;
    let blank = doc.rfind("\n\n").unwrap() + 1;
    let next_section = blank + 1;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(next_section));
    ws.enter_frame();
    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(blank), Grapheme(blank)),
        "left-arrow should rest on the blank line, not skip to the chip"
    );
    assert!(
        ws.editor.edit.cursor_line(Grapheme(blank)).is_some(),
        "blank line should render a row for the cursor"
    );
    assert!(ws.get_text().contains(FOLD_TAG), "cursor on the blank line must not unfold");

    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
        "second left-arrow should skip the hidden contents to the chip"
    );

    ws.push(char_arrow(false));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(blank), Grapheme(blank)),
        "right-arrow should step past the hidden contents to the blank line"
    );
    assert!(ws.get_text().contains(FOLD_TAG));
}

/// Same boundary rule for items: the blank line after a folded item's
/// subtree is walkable.
#[test]
fn blank_line_after_folded_item_is_walkable() {
    let doc = format!("* a{FOLD_TAG}\n  * b\n\n* c");
    let tag_end = doc.find("-->").unwrap() + 3;
    let blank = doc.rfind("\n\n").unwrap() + 1;
    let next_item = blank + 1;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(next_item));
    ws.enter_frame();
    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(blank), Grapheme(blank)),
        "left-arrow should rest on the blank line, not skip to the chip"
    );

    ws.push(char_arrow(true));
    ws.enter_frame();
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
        "second left-arrow should skip the hidden subtree to the chip"
    );
    assert!(ws.get_text().contains(FOLD_TAG));
}

/// A trailing blank line after a section folded through doc end stays
/// reachable: right-arrow from the chip lands on it.
#[test]
fn trailing_blank_line_after_doc_end_fold_is_reachable() {
    let doc = format!("# a{FOLD_TAG}\nbody\n");
    let tag_end = doc.find("-->").unwrap() + 3;
    let last = doc.chars().count();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(char_arrow(false));
    ws.enter_frame();
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (Grapheme(last), Grapheme(last)),);
    assert!(ws.get_text().contains(FOLD_TAG));
}

/// Backspace at the first position after a folded section would join
/// visible text into hidden text; it unfolds instead and deletes nothing.
#[test]
fn backspace_after_folded_section_unfolds_instead_of_joining() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let next_section = doc.find("\n# b").unwrap() + 1;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(next_section));
    ws.enter_frame();
    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
    ws.enter_frame();

    assert_eq!(ws.get_text(), "# a\nbody\n# b");
    let cursor = Grapheme(next_section - FOLD_TAG.len());
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (cursor, cursor));
}

/// A selection that contains capsules stands for their hidden contents:
/// deleting it deletes the folded subtrees too — nothing left behind to
/// pop into view.
#[test]
fn deleting_selection_of_folded_items_deletes_their_contents() {
    let doc = format!("* a{FOLD_TAG}\n  * a1\n* b{FOLD_TAG}\n  * b1\n* c");
    let tag_b_end = doc.rfind("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(tag_b_end)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
    ws.enter_frame();

    assert_eq!(ws.get_text(), "\n* c");
    assert_eq!(ws.editor.edit.renderer.buffer.current.selection, (Grapheme(0), Grapheme(0)),);
}

/// Cut carries a capsule's hidden contents to the clipboard with it, so
/// cut+paste moves the folded section intact.
#[test]
fn cut_selection_with_capsule_takes_contents() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(tag_end)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Cut);
    ws.enter_frame();

    assert_eq!(ws.get_text(), "\n# b");
}

/// Typing over a selection that contains a capsule replaces the hidden
/// contents along with it.
#[test]
fn typing_over_selection_replaces_folded_contents() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(tag_end)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Replace { region: Region::Selection, text: "x".into(), advance_cursor: true });
    ws.enter_frame();

    assert_eq!(ws.get_text(), "x\n# b");
}

/// Shift+right from right of the chip selects past the hidden contents;
/// deleting that selection removes the tag along with the contents it
/// hid, instead of orphaning an inert tag.
#[test]
fn deleting_selection_covering_contents_takes_tag_too() {
    let doc = format!("# a{FOLD_TAG}\nbody\n# b");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Char),
            backwards: false,
            extend_selection: true,
        },
    });
    ws.enter_frame();
    let next_section = doc.find("\n# b").unwrap() + 1;
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(next_section)),
        "extending right should select past the folded contents"
    );

    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "# a# b");
}

/// Backspace on an *empty* line after a folded section has nothing to
/// join into hidden text: it deletes the line and rests the cursor right
/// of the chip — no unfold.
#[test]
fn backspace_empty_line_after_folded_section_deletes_line() {
    let doc = format!("* a{FOLD_TAG}\n  * b\n");
    let tag_end = doc.find("-->").unwrap() + 3;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let doc_end = doc.chars().count();
    ws.push(select_at(doc_end));
    ws.enter_frame();
    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
    ws.enter_frame();

    assert_eq!(ws.get_text(), format!("* a{FOLD_TAG}\n  * b"), "newline deleted, fold kept");
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_end), Grapheme(tag_end)),
        "cursor should rest right of the chip"
    );
}

/// Forward-delete right of the chip would erase hidden text invisibly;
/// it unfolds instead and deletes nothing.
#[test]
fn forward_delete_right_of_chip_unfolds_instead_of_deleting() {
    let (doc, _, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: false,
        },
    });
    ws.enter_frame();

    assert_eq!(ws.get_text(), "# a\nbody");
}

/// Backspace right of the chip deletes the whole tag (unfolding), never
/// leaving partial tag text behind.
#[test]
fn backspace_right_of_chip_removes_whole_tag() {
    let (doc, tag_start, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    ws.push(Event::Delete {
        region: Region::SelectionOrAdvance {
            advance: Advance::By(Increment::Char),
            backwards: true,
        },
    });
    ws.enter_frame();

    assert_eq!(ws.get_text(), "# a\nbody");
    assert_eq!(
        ws.editor.edit.renderer.buffer.current.selection,
        (Grapheme(tag_start), Grapheme(tag_start)),
    );
}

/// Clicking the chip expands the section (removes the fold tag).
#[test]
fn click_chip_unfolds() {
    let (doc, tag_start, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let chip_rect = ws
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .find(|f| {
            f.source_range == (Grapheme(tag_start), Grapheme(tag_end)) && f.interaction.is_some()
        })
        .expect("chip fragment")
        .rect;
    let pos = chip_rect.center();
    ws.enter_frame_with_input(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]);
    ws.enter_frame(); // apply the queued unfold
    ws.enter_frame();

    assert_eq!(ws.get_text(), "# a\nbody");
}

/// Selecting a region into the folded contents (e.g. shift+arrows past
/// the chip) unfolds; selecting *across* the entire fold (select-all)
/// keeps it folded.
#[test]
fn selection_into_contents_unfolds_but_select_all_does_not() {
    let (doc, _, _) = folded_heading_doc();

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    ws.push(Event::Select { region: Region::Bound { bound: Bound::Doc, backwards: false } });
    ws.enter_frame();
    assert!(ws.get_text().contains(FOLD_TAG), "select-all must keep the fold");

    let body_mid = doc.find("body").unwrap() + 2;
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    ws.push(select_at(body_mid));
    ws.enter_frame();
    assert_eq!(ws.get_text(), "# a\nbody", "selection endpoint in contents must unfold");
}

/// Nested folds: a cursor landing in contents hidden by both an outer
/// and an inner fold unfolds both — every cursor position is visible.
#[test]
fn cursor_into_nested_folds_unfolds_all() {
    let doc = format!("# a{FOLD_TAG}\n## b{FOLD_TAG}\nbody\n");
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let body_mid = doc.find("body").unwrap() + 2;
    ws.push(select_at(body_mid));
    ws.enter_frame();
    assert_eq!(ws.get_text(), "# a\n## b\nbody\n");
}

/// The caret beside the chip renders beside the capsule — outside its
/// side padding — not against the icon glyph like inline-code editing.
#[test]
fn cursor_beside_chip_renders_outside_capsule() {
    let (doc, tag_start, tag_end) = folded_heading_doc();
    let chip_rect = |ws: &TestEditor| {
        ws.editor
            .edit
            .renderer
            .fragments
            .iter()
            .find(|f| {
                f.source_range == (Grapheme(tag_start), Grapheme(tag_end))
                    && f.interaction.is_some()
            })
            .expect("chip fragment")
            .rect
    };
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(select_at(tag_end));
    ws.enter_frame();
    let glyph_right = chip_rect(&ws).max.x;
    let caret_x = ws
        .editor
        .edit
        .cursor_line(Grapheme(tag_end))
        .expect("caret")[0]
        .x;
    assert!(
        caret_x > glyph_right + 1.0,
        "caret right of chip at x={caret_x} should clear the capsule padding (glyph right edge {glyph_right})"
    );

    ws.push(select_at(tag_start));
    ws.enter_frame();
    let glyph_left = chip_rect(&ws).min.x;
    let caret_x = ws
        .editor
        .edit
        .cursor_line(Grapheme(tag_start))
        .expect("caret")[0]
        .x;
    assert!(
        caret_x < glyph_left - 1.0,
        "caret left of chip at x={caret_x} should clear the capsule padding (glyph left edge {glyph_left})"
    );
}

/// ToggleFold with the cursor on the heading folds and unfolds without
/// the cursor's own position immediately undoing the fold.
#[test]
fn toggle_fold_round_trip() {
    let doc = "# a\nbody";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();

    ws.push(select_at(1));
    ws.enter_frame();
    ws.push(Event::ToggleFold);
    ws.enter_frame();
    ws.enter_frame(); // apply_fold's events apply next frame
    assert_eq!(ws.get_text(), format!("# a{FOLD_TAG}\nbody"));

    ws.push(Event::ToggleFold);
    ws.enter_frame();
    ws.enter_frame();
    assert_eq!(ws.get_text(), "# a\nbody");
}

/// Folding while the selection reaches into the contents clips the
/// selection to the visible region so the section folds and stays folded.
#[test]
fn fold_with_selection_into_contents_stays_folded() {
    let doc = "# a\nbody";
    let body_mid = doc.find("body").unwrap() + 2;
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(1)),
            end: Location::Grapheme(Grapheme(body_mid)),
        },
    });
    ws.enter_frame();
    ws.push(Event::ToggleFold);
    ws.enter_frame();
    ws.enter_frame();
    ws.enter_frame(); // settle: fold replace + clipped selection
    assert_eq!(ws.get_text(), format!("# a{FOLD_TAG}\nbody"));
}

/// A word selection that lands inside the tag (e.g. a double-click near
/// the chip) grows to cover the whole capsule — partial tag text can
/// never survive as source — and an edit over it replaces the contents
/// the capsule stands for.
#[test]
fn word_select_inside_tag_snaps_to_whole_tag() {
    let (doc, tag_start, tag_end) = folded_heading_doc();
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    ws.push(Event::Select {
        region: Region::BoundAt {
            bound: Bound::Word,
            location: Location::Grapheme(Grapheme(tag_end - 1)),
            backwards: true,
        },
    });
    ws.enter_frame();
    let selection = ws.editor.edit.renderer.buffer.current.selection;
    assert_eq!(
        (selection.0.0.min(selection.1.0), selection.0.0.max(selection.1.0)),
        (tag_start, tag_end),
        "in-tag word selection should cover the whole tag"
    );

    ws.push(Event::Replace { region: Region::Selection, text: "x".into(), advance_cursor: true });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "# ax");
}
