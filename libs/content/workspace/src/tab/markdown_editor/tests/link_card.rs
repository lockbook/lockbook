//! Tests for the link-preview card trigger ([`MdRender::link_renders_as_card`]).
//! A link becomes a block card only when it's a bare autolink that is the sole
//! content of a top-level paragraph (not inside a container block). The source
//! stays clean, portable markdown — the trigger is positional, not a syntax
//! suffix — so these pin exactly which links qualify.

use std::sync::{Arc, Mutex};

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::RangeExt as _;

use super::super::input::Event;
use super::harness::TestEditor;
use crate::tab::markdown_editor::widget::inline::link::meta::{LinkMeta, LinkMetaState};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, FragmentContent};

/// True if a `LinkCard` embed fragment was painted this frame.
fn has_card_fragment(ws: &TestEditor) -> bool {
    ws.editor
        .edit
        .renderer
        .fragments
        .iter()
        .any(|f| matches!(f.content, FragmentContent::Embed { kind: EmbedKind::LinkCard, .. }))
}

/// Render `md` with `cache_url`'s metadata pre-seeded (no network), then report
/// whether a card was emitted.
fn renders_card_with_cached_meta(md: &str, cache_url: &str) -> bool {
    let mut ws = TestEditor::new(md);
    ws.editor
        .edit
        .renderer
        .layout_cache
        .link_meta
        .borrow_mut()
        .insert(
            cache_url.to_string(),
            Arc::new(Mutex::new(LinkMetaState::Loaded(LinkMeta {
                title: "Example Title".into(),
                ..Default::default()
            }))),
        );
    ws.enter_frame();
    has_card_fragment(&ws)
}

/// Whether the *first* link in `md` renders as a card.
fn first_link_is_card(md: &str) -> bool {
    let mut ws = TestEditor::new(md);
    let arena = Arena::new();
    let root: &AstNode = ws.editor.edit.renderer.reparse(&arena);
    let r = &ws.editor.edit.renderer;
    root.descendants()
        .find(|n| matches!(n.data.borrow().value, NodeValue::Link(_)))
        .map(|n| r.link_renders_as_card(n))
        .expect("a link node")
}

#[test]
fn bare_url_alone_on_a_line_is_a_card() {
    assert!(first_link_is_card("https://example.com\n"));
}

#[test]
fn bare_url_under_a_heading_is_a_card() {
    // Headings are leaf blocks, not containers — content under them is top-level.
    assert!(first_link_is_card("# Title\n\nhttps://example.com\n"));
}

#[test]
fn bare_url_in_a_sentence_is_not_a_card() {
    assert!(!first_link_is_card("see https://example.com for details\n"));
}

#[test]
fn labeled_link_is_not_a_card() {
    // The user opted into a label — it stays a normal link, even alone on a line.
    assert!(!first_link_is_card("[Example](https://example.com)\n"));
}

#[test]
fn angle_bracket_autolink_is_not_a_card() {
    // `<url>` is the suppression escape hatch (Slack/Discord convention).
    assert!(!first_link_is_card("<https://example.com>\n"));
}

#[test]
fn url_in_a_container_block_is_not_a_card() {
    // One representative container — the trigger excludes any container block
    // via the same ancestor walk, so list item stands in for quote/task/nested.
    assert!(!first_link_is_card("- https://example.com\n"));
}

#[test]
fn two_urls_on_one_line_are_not_cards() {
    // Neither is the *sole* content of the paragraph.
    assert!(!first_link_is_card("https://a.example https://b.example\n"));
}

// ── preview-fetch opt-in ──

#[test]
fn previews_off_does_not_fetch_autolink_titles() {
    // Default `contact_linked_sites = false`: a bare autolink must not contact
    // the site — no entry is inserted into the title cache (network-free test).
    let mut ws = TestEditor::new("https://example.com\n");
    ws.enter_frame();
    assert!(
        ws.editor
            .edit
            .renderer
            .layout_cache
            .link_meta
            .borrow()
            .is_empty(),
        "no title fetch should be initiated when previews are off"
    );
}

// ── card rendering (does a card fragment actually get emitted) ──

#[test]
fn cached_metadata_renders_a_card() {
    assert!(renders_card_with_cached_meta("https://example.com\n", "https://example.com"));
}

#[test]
fn url_in_list_renders_no_card_even_when_cached() {
    // The positional trigger excludes container blocks regardless of metadata.
    assert!(!renders_card_with_cached_meta("- https://example.com\n", "https://example.com"));
}

#[test]
fn setting_mirrors_onto_renderer_each_frame() {
    let mut ws = TestEditor::new("text\n");
    assert!(!ws.editor.edit.renderer.contact_linked_sites, "off by default");
    ws.editor.persistence.set_contact_linked_sites(true);
    ws.enter_frame();
    assert!(ws.editor.edit.renderer.contact_linked_sites, "mirrored from persistence");
}

/// A wrapped capsule re-opens its style scope on each row, so chip boundary
/// anchors exist per row at the same source positions. The edge carets must
/// resolve to the capsule's *true* edges: start on the first row's pill-left,
/// end on the last row's pill-right (regression: the end caret rendered at
/// the end of the first row's segment).
#[test]
fn wrapped_capsule_edge_carets() {
    use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
    use lb_rs::model::text::offset_types::IntoRangeExt as _;
    let url = "https://example.com";
    let mut ws = TestEditor::new("see https://example.com\nnext para\n");
    ws.editor
        .edit
        .renderer
        .layout_cache
        .link_meta
        .borrow_mut()
        .insert(
            url.to_string(),
            Arc::new(Mutex::new(LinkMetaState::Loaded(LinkMeta {
                title: "Example Title".into(),
                favicon_url: Some("https://example.com/favicon.ico".into()),
                ..Default::default()
            }))),
        );
    // narrow viewport: the capsule wraps across two rows
    ws.enter_frame_at(egui::Vec2::new(160.0, 600.0));

    let range = {
        let arena = Arena::new();
        let root: &AstNode = ws.editor.edit.renderer.reparse(&arena);
        let node = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::Link(_)))
            .expect("a link node");
        ws.editor.edit.renderer.node_range(node)
    };
    ws.push(Event::Select { region: range.end().into_range().into() });
    ws.enter_frame_at(egui::Vec2::new(160.0, 600.0));

    let anchors = |offset| {
        ws.editor
            .edit
            .renderer
            .fragments
            .iter()
            .filter(|f| {
                f.source_range == (offset, offset)
                    && matches!(f.content, FragmentContent::Spacer)
                    && f.style_stack.last().is_some_and(|s| s.chip)
            })
            .collect::<Vec<_>>()
    };
    let trailing = anchors(range.end());
    assert!(trailing.len() > 1, "capsule wrapped: an anchor per row, got {}", trailing.len());

    let start_line = ws
        .editor
        .edit
        .cursor_line(range.start())
        .expect("start caret");
    let end_line = ws.editor.edit.cursor_line(range.end()).expect("end caret");
    assert!(
        end_line[0].y > start_line[0].y,
        "end caret on a later row: start {start_line:?} end {end_line:?}"
    );
    let true_end = trailing.last().unwrap().rect.max.x;
    assert_eq!(end_line[0].x, true_end, "end caret at the last row's pill edge");
    let true_start = anchors(range.start()).first().unwrap().rect.min.x;
    assert_eq!(start_line[0].x, true_start, "start caret at the first row's pill edge");
}

/// A tap-selected capsule's selection rects hug the pill: one merged rect per
/// row, spanning the side pads, whose far edge is where iOS drops the end
/// selection handle. (Regression: every capsule fragment shares the atom's
/// full source range, and recomputing both rect edges per fragment mangled
/// the segments — the reading-order sort then put a mid-pill space rect last,
/// dropping the native end handle in the middle of the capsule.)
#[test]
fn capsule_selection_rects_hug_pill() {
    use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
    let url = "https://example.com";
    let mut ws = TestEditor::new("see https://example.com after\n");
    ws.editor
        .edit
        .renderer
        .layout_cache
        .link_meta
        .borrow_mut()
        .insert(
            url.to_string(),
            Arc::new(Mutex::new(LinkMetaState::Loaded(LinkMeta {
                title: "Example Title".into(),
                favicon_url: Some("https://example.com/favicon.ico".into()),
                ..Default::default()
            }))),
        );
    ws.enter_frame();

    let range = {
        let arena = Arena::new();
        let root: &AstNode = ws.editor.edit.renderer.reparse(&arena);
        let node = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::Link(_)))
            .expect("a link node");
        ws.editor.edit.renderer.node_range(node)
    };
    // tap-select the capsule
    ws.push(Event::Select { region: range.into() });
    ws.enter_frame();

    let pad_edge = |offset, leading: bool| {
        let pads: Vec<_> = ws
            .editor
            .edit
            .renderer
            .fragments
            .iter()
            .filter(|f| {
                f.source_range == (offset, offset)
                    && matches!(f.content, FragmentContent::Spacer)
                    && f.style_stack.last().is_some_and(|s| s.chip)
            })
            .collect();
        if leading { pads.first().unwrap().rect.min.x } else { pads.last().unwrap().rect.max.x }
    };
    let pill_left = pad_edge(range.start(), true);
    let pill_right = pad_edge(range.end(), false);

    let rects = ws.editor.edit.range_rects(range);
    assert_eq!(rects.len(), 1, "one merged rect for an unwrapped capsule: {rects:?}");
    assert_eq!(rects[0].min.x, pill_left, "highlight spans the leading pad");
    assert_eq!(rects[0].max.x, pill_right, "highlight spans the trailing pad");

    // the native end handle reads the last rect's far edge == the end caret
    let end_line = ws.editor.edit.cursor_line(range.end()).expect("end caret");
    assert_eq!(rects.last().unwrap().max.x, end_line[0].x, "end handle at the pill edge");
}

/// Mobile edit-menu "Edit" on a link preview: tap-select keeps the card
/// (selection alone never reveals — select-all mustn't burst every preview);
/// `Event::EnterAtom` force-reveals the source with the whole URL selected
/// (a bare autolink *is* its URL, so there's no interior to caret into);
/// moving the selection out restores the card.
#[test]
fn enter_atom_reveals_link_card_source() {
    let url = "https://example.com";
    let mut ws = TestEditor::new("https://example.com\n");
    ws.editor
        .edit
        .renderer
        .layout_cache
        .link_meta
        .borrow_mut()
        .insert(
            url.to_string(),
            Arc::new(Mutex::new(LinkMetaState::Loaded(LinkMeta {
                title: "Example Title".into(),
                ..Default::default()
            }))),
        );
    ws.enter_frame();
    assert!(has_card_fragment(&ws), "card renders with cached meta");

    let range = {
        let arena = Arena::new();
        let root: &AstNode = ws.editor.edit.renderer.reparse(&arena);
        let node = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::Link(_)))
            .expect("a link node");
        ws.editor.edit.renderer.node_range(node)
    };
    ws.push(Event::Select { region: range.into() });
    ws.enter_frame();
    assert!(has_card_fragment(&ws), "tap-select keeps the card");

    ws.push(Event::EnterAtom);
    ws.enter_frame();
    assert!(!has_card_fragment(&ws), "entered: raw source replaces the card");
    let sel = ws.editor.edit.renderer.buffer.current.selection;
    assert_eq!(&ws.editor.edit.renderer.buffer[sel], url, "whole url selected");

    // leaving the atom restores the card
    let after = (range.end() + 1, range.end() + 1);
    ws.push(Event::Select { region: after.into() });
    ws.enter_frame();
    assert!(has_card_fragment(&ws), "left: card restored");
}
