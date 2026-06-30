//! Tests for the link-preview card trigger ([`MdRender::link_renders_as_card`]).
//! A link becomes a block card only when it's a bare autolink that is the sole
//! content of a top-level paragraph (not inside a container block). The source
//! stays clean, portable markdown — the trigger is positional, not a syntax
//! suffix — so these pin exactly which links qualify.

use std::sync::{Arc, Mutex};

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};

use super::harness::TestEditor;
use crate::tab::markdown_editor::widget::inline::link_meta::{LinkMeta, LinkMetaState};
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
fn url_in_a_list_item_is_not_a_card() {
    assert!(!first_link_is_card("- https://example.com\n"));
}

#[test]
fn url_in_a_blockquote_is_not_a_card() {
    assert!(!first_link_is_card("> https://example.com\n"));
}

#[test]
fn url_in_a_task_item_is_not_a_card() {
    assert!(!first_link_is_card("- [ ] https://example.com\n"));
}

#[test]
fn two_urls_on_one_line_are_not_cards() {
    // Neither is the *sole* content of the paragraph.
    assert!(!first_link_is_card("https://a.example https://b.example\n"));
}

#[test]
fn nested_list_url_is_not_a_card() {
    assert!(!first_link_is_card("- outer\n  - https://example.com\n"));
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
fn previews_off_uncached_renders_no_card() {
    // Default off + nothing cached → a bare URL stays a normal inline link.
    let mut ws = TestEditor::new("https://example.com\n");
    ws.enter_frame();
    assert!(!has_card_fragment(&ws));
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
