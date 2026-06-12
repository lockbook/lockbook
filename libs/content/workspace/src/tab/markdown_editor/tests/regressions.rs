//! Example-based regression and exact-behavior tests for the markdown
//! editor. Unlike the property audit tables in [`super::render_props`] /
//! [`super::edit_props`], each test here pins one specific input to one
//! specific outcome (often a shipped bug fix). Render-side tests reuse the
//! property `_check` helpers exposed by [`super::render_props`].

use comrak::Arena;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt};

use super::super::input::{Advance, Bound, Event, Location, Region};
use super::super::widget::utils::wrap_layout::FragmentContent;
use super::harness::TestEditor;
use super::render_props::{
    assert_bg_rows_meet_within_inline_code, fragment_rect_disjoint_check_md,
    galley_rect_in_render_area, glyph_in_render_area_check_md, render_frame, test_renderer,
};

// ── renderer ──

#[test]
fn image_link_no_break_opportunities_wraps() {
    // Real-world content from a user file (cb650r.md): a line
    // that begins with `^^` and then immediately contains an
    // image link, with no whitespace between, and with identifier-
    // shaped (underscore/hyphen-heavy) alt text and URL. None of
    // `_`, `-`, alphanumerics are UAX#14 break opportunities, so
    // cosmic-text can produce a `ShapeWord` covering most of the
    // line — walker must produce break points so the row wraps
    // within the column instead of running off the right edge.
    let md =
        "^^![pasted_image_2025-07-28_00-51-47.png](imports/pasted_image_2025-07-28_00-51-47.png)\n";
    for w in [200.0, 300.0, 400.0, 500.0, 600.0] {
        // Reproduce without cursor on the line (reveal=false): the
        // image renders just its alt text, which is itself a long
        // identifier-heavy token. The user reports this case also
        // fails in real usage.
        glyph_in_render_area_check_md(md, w).unwrap_or_else(|e| panic!("width={}: {}", w, e));
    }
}

#[test]
fn long_link_stays_inside_render_area() {
    // Long links should wrap at UAX#14 break opportunities (`/`,
    // `.`) inserted via the walker's zero-Glue between adjacent
    // non-blank words; if there are no break opportunities the
    // per-cluster glyph wrap should kick in.
    let cases = [
        // Auto-link with `/` break opportunities
        "Click <https://example.com/very/long/path/to/resource> please.\n",
        // Inline link with long display text and href
        "[link text that is quite long](https://example.com/x)\n",
        // Long display text mirrors URL (common auto-link form)
        "[https://example.com/very/long/path](https://example.com/very/long/path)\n",
        // Display text is one long un-broken token (needs per-cluster wrap)
        "[verylongtokenwithnobreakers](https://example.com)\n",
    ];
    for md in cases {
        for w in [200.0, 250.0, 300.0, 400.0] {
            galley_rect_in_render_area(md, w)
                .unwrap_or_else(|e| panic!("'{}' at width={}: {}", md.trim(), w, e));
        }
    }
}

/// Each captured block marker is selectable: its `line_own_prefix` is
/// covered by exactly one `Spacer` fragment that hit-testing lands on, so
/// a drag/double-click selects the whole marker (Part A).
#[test]
fn block_markers_are_selectable() {
    use comrak::nodes::NodeValue;

    type Pred = fn(&NodeValue) -> bool;
    let cases: &[(&str, Pred)] = &[
        ("- a\n", |v| matches!(v, NodeValue::Item(_))),
        ("1. a\n", |v| matches!(v, NodeValue::Item(_))),
        ("> a\n", |v| matches!(v, NodeValue::BlockQuote)),
        ("- [ ] a\n", |v| matches!(v, NodeValue::TaskItem(_))),
    ];
    for (md, pred) in cases {
        let mut r = test_renderer(md);
        render_frame(&mut r, 800.0, None, |_| {});

        let arena = Arena::new();
        let root = r.reparse(&arena);
        let node = root
            .descendants()
            .find(|n| pred(&n.data.borrow().value))
            .unwrap_or_else(|| panic!("no matching container in {md:?}"));
        let line = r.node_first_line(node);
        let own = r.line_own_prefix(node, line);
        assert!(!own.is_empty(), "{md:?}: marker own_prefix is empty");

        let frag = r
            .fragments
            .iter()
            .find(|f| f.source_range == own)
            .unwrap_or_else(|| panic!("{md:?}: no fragment maps the marker {own:?}"));
        assert!(
            matches!(frag.content, FragmentContent::Spacer),
            "{md:?}: marker fragment is not a Spacer"
        );

        // Hit-testing the marker's gutter lands on its fragment, so a
        // drag / double-click selects the whole marker.
        let idx = r.closest_fragment_at_pos(frag.rect.center()).unwrap();
        assert_eq!(
            r.fragments[idx].source_range, own,
            "{md:?}: hit-test on marker did not resolve to the marker fragment"
        );
    }
}

/// Nested-list indentation is selectable per level: the gutter columns of a
/// deeply-nested item tile contiguously from the block's left edge up to
/// the marker, with no empty column behind the bullet. Each level owns its
/// own column (its `line_own_prefix`), so the columns tile without gaps.
#[test]
fn nested_indent_columns_are_selectable() {
    use comrak::nodes::NodeValue;

    let md = "a\n* a\n  * a\n    * a\n";
    let mut r = test_renderer(md);
    render_frame(&mut r, 800.0, None, |_| {});

    let arena = Arena::new();
    let root = r.reparse(&arena);
    let deepest = root
        .descendants()
        .filter(|n| matches!(n.data.borrow().value, NodeValue::Item(_)))
        .max_by_key(|n| n.ancestors().count())
        .expect("an item");
    let marker = r.line_own_prefix(deepest, r.node_first_line(deepest));
    let marker_rect = r
        .fragments
        .iter()
        .find(|f| f.source_range == marker)
        .expect("deepest marker fragment")
        .rect;

    // Gutter (Spacer) columns on the marker's row, left of the marker.
    // Filter on `Spacer` content, not `atomic` — indentation columns are
    // non-atomic (a click there places the cursor rather than selecting
    // the column); only marker columns are atomic.
    let mut row: Vec<_> = r
        .fragments
        .iter()
        .filter(|f| {
            matches!(f.content, FragmentContent::Spacer)
                && !f.source_range.is_empty()
                && (f.rect.min.y - marker_rect.min.y).abs() < 0.5
                && f.rect.max.x <= marker_rect.max.x + 0.5
        })
        .map(|f| f.rect)
        .collect();
    row.sort_by(|a, b| a.min.x.partial_cmp(&b.min.x).unwrap());

    assert!(row.len() >= 3, "expected 3 nested gutter columns, got {}", row.len());
    // They must tile contiguously up to the marker — no gap (the reported
    // empty column right behind the bullet).
    for pair in row.windows(2) {
        assert!(
            (pair[0].max.x - pair[1].min.x).abs() < 0.5,
            "gap between gutter columns: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
    assert!(
        (row.last().unwrap().max.x - marker_rect.max.x).abs() < 0.5,
        "gutter columns don't reach the marker"
    );
}

#[test]
fn reveal_toggle_preserves_content_coverage() {
    // Move the cursor through every grapheme position of a doc with
    // multiple inlines (each revealing in turn) and check that
    // *every* non-whitespace text-bearing node manifests at every
    // cursor position. Catches reveal-toggle render bugs where some
    // inline disappears when the cursor enters/leaves it.
    let md = ":smile: foo `code` bar *emph* baz\n";
    let mut r = test_renderer(md);
    let last = r.buffer.current.segs.last_cursor_position().0;
    for cursor in 0..=last {
        r.buffer.current.selection = (Grapheme(cursor), Grapheme(cursor));
        r.reveal_selection = Some(r.buffer.current.selection);
        r.reveal_seq = cursor as u64;
        // Pick a few text nodes by hand and assert they manifest. The
        // literal source positions (grapheme indices — every char is 1
        // grapheme in this ASCII doc):
        //   :smile: → (0,7)  foo → (8, 11)  code → (13, 17)
        //   bar → (19, 22)   emph → (24, 28)  baz → (30, 33)
        let covered = render_frame(&mut r, 400.0, None, |r| {
            [(0, 7), (8, 11), (13, 17), (19, 22), (24, 28), (30, 33)]
                .iter()
                .all(|&(lo, hi)| {
                    r.fragments
                        .iter()
                        .any(|f| f.source_range.0.0 < hi && f.source_range.1.0 > lo)
                })
        });
        assert!(covered, "content uncovered at cursor offset {cursor}");
    }
}

#[test]
fn inline_code_wrap_bgs_meet_at_row_seam() {
    // Long inline code that's wider than the column → wraps. Each
    // visual row gets its own bg fragment; they should meet exactly
    // at the row seam (bottom of upper = top of lower) because
    // `row_spacing = 2 × inline_padding` is sized so the inline_pad
    // each side extends fills the spacing exactly.
    let md = "`one two three four five six seven eight nine ten`\n";
    // Narrow width forces wrap.
    assert_bg_rows_meet_within_inline_code(md, 200.0).unwrap();
}

#[test]
fn heading_autolink_with_fold_stays_in_render_area() {
    let md = "# **foo foo <https://x.test>** <!-- {\"fold\":true} -->\n";
    for i in 0..40u32 {
        let w = 250.0 + (i as f32) * 25.0;
        glyph_in_render_area_check_md(md, w).unwrap_or_else(|e| panic!("width={w}: {e}"));
    }
}

#[test]
fn adjacent_inline_code_backgrounds_dont_overlap() {
    // Two inline-code spans separated by a single space — both have
    // a background. With `Fragment::rect` as the visual extent (bg
    // padding included), the property collapses to "no two fragment
    // rects overlap." The walker reserves the bg breathing room via
    // `Pad` items emitted inside each backgrounded scope.
    let cases = [
        "`foo` `bar`\n",
        "text `foo` text `bar` text\n",
        // Same with `Highlight` (`==…==`) which is also backgrounded.
        "==foo== ==bar==\n",
        // Inline code immediately followed by `Highlight`.
        "`foo`==bar==\n",
        // Nested distinct colors: highlight wrapping inline code.
        "==before `foo` after==\n",
        // List item with inline code — regression for the user-
        // observed "inline code in list item has no decoration".
        "- contains `Code` between text\n",
    ];
    for md in &cases {
        for w in [200.0, 400.0, 800.0] {
            fragment_rect_disjoint_check_md(md, w)
                .unwrap_or_else(|e| panic!("md={md:?} width={w}: {e}"));
        }
    }
}

#[test]
fn icon_glyphs_skip_emoji_font() {
    use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, shape_as_emoji};
    use crate::theme::icons::Icon;

    // The touch-mode open-link affordance is a Nerd Font icon in the
    // supplementary PUA, which overlaps the emoji codepoint range. It must not
    // route to the colorless emoji font, or it renders in the default fg
    // instead of blue (#4653).
    assert!(!shape_as_emoji(&FontFamily::Icons, Icon::OPEN_IN_NEW.icon));

    // Emoji in regular text still route to the emoji font — the Icons guard
    // must not regress this. `:warning:` carries VS-16, which the editor needs
    // so it renders in color rather than as a monochrome SF-Pro outline.
    assert!(shape_as_emoji(&FontFamily::Sans, "😀"));
    assert!(shape_as_emoji(&FontFamily::Sans, "\u{26A0}\u{FE0F}")); // ⚠️
    assert!(shape_as_emoji(&FontFamily::Mono, "🚀"));
    assert!(!shape_as_emoji(&FontFamily::Sans, "a"));
}

// ── editor ──

/// Regression for #4662: arrowing up off a folded heading must walk the
/// cursor to the lines above it, not freeze.
///
/// Originally the fold tag revealed to its ~22-char source when the cursor
/// landed on the heading line; on a narrow viewport that expansion wrapped
/// the heading across rows and up-arrowing stuck inside the wrapped tag.
/// The tag renders as a compact chip now, but the narrow width keeps the
/// heading wrapping across rows, preserving the walk this test guards.
/// (The property suite runs at the fixed 800px `SCREEN_SIZE`, where the
/// heading never wraps and the original bug stayed hidden.)
#[test]
fn up_arrow_through_fold_tag() {
    use super::super::input::Increment;
    use egui::Vec2;
    let up = || Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Lines(1)),
            backwards: true,
            extend_selection: false,
        },
    };
    let place = |g: usize| Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(g)),
            end: Location::Grapheme(Grapheme(g)),
        },
    };

    // Narrow width so the revealed tag wraps the heading line.
    let width = Vec2::new(320., 600.);
    let doc = "above one\nabove two\n# H1 heading text here <!-- {\"fold\":true} -->\nbody";
    // Start at the end of the fold tag (deepest into the wrapped rows).
    let start = doc.find("-->").unwrap() + 3;

    let mut ws = TestEditor::new(doc);
    ws.enter_frame_at(width);
    ws.push(place(start));
    ws.enter_frame_at(width);

    // The revealed heading wraps into many rows here, so press enough to
    // walk all of them; each press must make progress (no fixpoint).
    let mut trajectory = vec![ws.editor.edit.renderer.buffer.current.selection.1.0];
    for _ in 0..30 {
        ws.push(up());
        ws.enter_frame_at(width);
        trajectory.push(ws.editor.edit.renderer.buffer.current.selection.1.0);
    }

    // "above one" occupies offsets 0..=9; reaching it proves the cursor
    // walked past the wrapped fold tag and the lines above it.
    let landed = *trajectory.last().unwrap();
    assert!(landed <= 9, "up-arrow got stuck before the first line; trajectory: {trajectory:?}",);
}

/// Regression for #4665: arrowing up through a list of links must walk
/// the cursor to the lines above, not freeze.
///
/// Same shape as [`up_arrow_through_fold_tag`]: a link renders as just
/// its display text until the cursor lands on its line, then reveals to
/// its full `[text](url)` source. On a narrow viewport that expansion
/// wraps the link across visual rows, and up-arrowing then sticks on a
/// soft-wrap boundary inside the link instead of reaching the row above.
#[test]
fn up_arrow_through_link_list() {
    use super::super::input::Increment;
    use egui::Vec2;
    let up = || Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Lines(1)),
            backwards: true,
            extend_selection: false,
        },
    };
    let place = |g: usize| Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(g)),
            end: Location::Grapheme(Grapheme(g)),
        },
    };

    // Width where a revealed link source wraps across rows.
    let width = Vec2::new(480., 600.);
    let doc = "above one\nabove two\n[link one](https://example.com/some/fairly/long/path/one)\n[link two](https://example.com/some/fairly/long/path/two)\n[link three](https://example.com/some/fairly/long/path/three)";
    let start = doc.len();

    let mut ws = TestEditor::new(doc);
    ws.enter_frame_at(width);
    ws.push(place(start));
    ws.enter_frame_at(width);

    let mut trajectory = vec![ws.editor.edit.renderer.buffer.current.selection.1.0];
    for _ in 0..12 {
        ws.push(up());
        ws.enter_frame_at(width);
        trajectory.push(ws.editor.edit.renderer.buffer.current.selection.1.0);
    }

    // The two "above" lines occupy offsets 0..=18; reaching them proves
    // the cursor walked up past all three wrapped link lines.
    let landed = *trajectory.last().unwrap();
    assert!(landed <= 18, "up-arrow got stuck in the link list; trajectory: {trajectory:?}",);
}

/// Selection-driven scroll-to-cursor must arm before `post_render` so
/// reveal-driven height changes are followed by the scroll fix-up on
/// the same frame. The buggy version armed it after the draw phase, so
/// frame N+1 painted with shifted heights against the old offset and
/// frame N+2's scroll catch-up flashed the user's view.
///
/// Sequence: scroll a tall doc to the bottom (anchor is well above the
/// fenced code block), then move the cursor into the code block. Reveal
/// expands the block by many rows, and the catch-up scroll lands the
/// new offset on the same frame as the height change.
#[test]
fn scroll_stable_after_reveal_arrow() {
    use super::super::scroll_content::DocScrollContent;
    use crate::widgets::affine_scroll::Action;
    let mut doc = String::new();
    for _ in 0..40 {
        doc.push_str("paragraph paragraph paragraph paragraph paragraph\n\n");
    }
    doc.push_str("```\n");
    for _ in 0..30 {
        doc.push_str("code line code line code line\n");
    }
    doc.push_str("```\n\n");
    for _ in 0..30 {
        doc.push_str("trailing paragraph trailing paragraph\n\n");
    }
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    {
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
    ws.enter_frame();

    let cb_inside = doc.find("code line").unwrap() + 5;
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(cb_inside)),
            end: Location::Grapheme(Grapheme(cb_inside)),
        },
    });
    ws.enter_frame();
    let after_event = ws.editor.edit.scroll_area.stored_offset();
    ws.enter_frame();
    let after_idle = ws.editor.edit.scroll_area.stored_offset();

    assert_eq!(
        after_event, after_idle,
        "scroll offset shifted on the idle frame after reveal-driven height change",
    );
}

#[test]
fn harness_smoke() {
    let mut ws = TestEditor::new("hello world");
    ws.enter_frame();
    assert_eq!(ws.get_text(), "hello world");
}

/// Regression: a link-title fetch completing must invalidate cached
/// heights. Empty-text links display the resolved title (or the raw URL
/// if the title's still loading); a long URL → short title flips wrap
/// rows, so the cached height becomes wrong if `link_seq` isn't stamped.
#[test]
fn layout_cache_consistent_under_link_title() {
    use std::sync::{Arc, Mutex};
    const EPS: f32 = 0.5;
    // Long URL ensures URL display wraps to multiple rows; short title
    // collapses to one row, making the height delta visible.
    let url = "https://x.test/very/long/path/that/should/wrap/across/multiple/rows/in/the/editor/viewport";
    let doc = format!("[]({url})\n");
    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    {
        let mut titles = ws
            .editor
            .edit
            .renderer
            .layout_cache
            .link_titles
            .borrow_mut();
        titles.insert(
            url.to_string(),
            Arc::new(Mutex::new(super::super::widget::block::TitleState::Loaded("ok".into()))),
        );
    }
    ws.editor.edit.renderer.layout_cache.link_seq.store(
        ws.editor
            .edit
            .renderer
            .ws_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::sync::atomic::Ordering::Relaxed,
    );
    ws.enter_frame();

    let arena = comrak::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let nodes: Vec<_> = root.descendants().collect();
    let snapshot: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            ws.editor
                .edit
                .renderer
                .get_cached_node_height(n)
                .map(|h| (i, h))
        })
        .collect();
    ws.editor
        .edit
        .renderer
        .layout_cache
        .height
        .borrow_mut()
        .clear();
    for (i, cached) in snapshot {
        let fresh = ws.editor.edit.renderer.height(nodes[i]);
        assert!(
            (cached - fresh).abs() <= EPS,
            "cached {} != fresh {} for node[{}]",
            cached,
            fresh,
            i,
        );
    }
}

/// Regression: completing an embed load must invalidate cached image
/// heights. With `embeds_seq` correctly stamped, the height cache picks
/// up the new image dims on the next read; without it, layout reuses
/// the placeholder height.
#[test]
fn layout_cache_consistent_under_embed() {
    const EPS: f32 = 0.5;
    let lb = super::harness::build_lb();
    let (mut ws, embeds) =
        TestEditor::with_test_embeds(lb, "![alt](https://x.test/i.png)\n\nafter\n");
    ws.enter_frame();

    embeds.complete("https://x.test/i.png", egui::Vec2::new(300.0, 500.0));
    ws.enter_frame();

    let arena = comrak::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let nodes: Vec<_> = root.descendants().collect();
    let snapshot: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            ws.editor
                .edit
                .renderer
                .get_cached_node_height(n)
                .map(|h| (i, h))
        })
        .collect();
    ws.editor
        .edit
        .renderer
        .layout_cache
        .height
        .borrow_mut()
        .clear();
    for (i, cached) in snapshot {
        let fresh = ws.editor.edit.renderer.height(nodes[i]);
        assert!(
            (cached - fresh).abs() <= EPS,
            "cached {} != fresh {} for node[{}]",
            cached,
            fresh,
            i,
        );
    }
}

/// Regression: cursor entering a fenced code block flips its reveal
/// state, growing its height. Cached height must reflect the new state
/// rather than returning the pre-reveal value via a stale stamp.
#[test]
fn layout_cache_consistent_under_reveal() {
    const EPS: f32 = 0.5;
    let doc = "para\n\n```\nline\nline\nline\n```\n\npara\n";
    let mut ws = TestEditor::new(doc);

    // Cursor outside the code block.
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(0)),
            end: Location::Grapheme(Grapheme(0)),
        },
    });
    ws.enter_frame();

    // Cursor inside the code block — flips its reveal.
    let cb_inside = doc.find("line").unwrap() + 2;
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(cb_inside)),
            end: Location::Grapheme(Grapheme(cb_inside)),
        },
    });
    ws.enter_frame();

    let arena = comrak::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let nodes: Vec<_> = root.descendants().collect();
    let snapshot: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            ws.editor
                .edit
                .renderer
                .get_cached_node_height(n)
                .map(|h| (i, h))
        })
        .collect();
    ws.editor
        .edit
        .renderer
        .layout_cache
        .height
        .borrow_mut()
        .clear();
    for (i, cached) in snapshot {
        let fresh = ws.editor.edit.renderer.height(nodes[i]);
        assert!(
            (cached - fresh).abs() <= EPS,
            "cached height {} != fresh {} for node[{}]",
            cached,
            fresh,
            i,
        );
    }
}

/// Regression: tapping the unfold button on a deeply nested list with
/// the cursor offscreen below the fold used to "scroll to the bottom"
/// of the doc. Mechanism: removing the fold tag (a Replace) OT-shifts
/// the cursor's numeric offset; the editor mistook that for a user
/// cursor move and triggered scroll-to-cursor, pulling the viewport
/// toward the (offscreen) cursor near the doc end. Reproducer is the
/// real-world doc shape that surfaced the bug.
#[test]
fn fold_toggle_does_not_jump_scroll_regression() {
    use super::super::scroll_content::DocScrollContent;
    use crate::widgets::affine_scroll::Action;
    let doc = "* asdf\n  * asdf\n  * asdf\n  * asdf\n    * asdf\n      * asdf\n        * asdf\n      * asdf\n        * asdf\n          * asdf\n          * asdf\n        * asdf\n          * asdf\n            * asdf\n          * asdf\n* asdf\n    * asdf\n      * asdf\n          * asdf\n        * asdf\n      * asdf\n        * asdf\n        * asdf\n    * asdf<!-- {\"fold\":true} -->\n      * asdf\n        * asdf\n      * asdf\n    * asdf<!-- {\"fold\":true} -->\n      * asdf\n          * asdf\n            * asdf\n            * asdf\n        * asdf\n      * asdf\n      * asdf\n  * asdf<!-- {\"fold\":true} -->\n    * asdf\n      * asdf\n        * asdf\n      * asdf\n        * asdf\n          * asdf\n          * asdf\n        * asdf\n          * asdf\n            * asdf\n          * asdf\n    * asdf\n    * asdf\n      * asdf\n          * asdf\n        * asdf\n      * asdf\n        * asdf\n        * asdf\n    * asdf\n      * asdf\n        * asdf\n      * asdf\n    * asdf\n      * asdf\n          * asdf\n            * asdf\n            * asdf\n        * asdf\n      * asdf\n      * asdf\n      * asdf\n";
    let fold_tag = "<!-- {\"fold\":true} -->";
    let first_tag = doc.find(fold_tag).unwrap();
    let target =
        first_tag + fold_tag.len() + doc[first_tag + fold_tag.len()..].find(fold_tag).unwrap();

    let mut ws = TestEditor::new(doc);
    ws.editor.edit.renderer.touch_mode = true;
    ws.enter_frame();

    let cursor_pos = doc.len() - 5;
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(cursor_pos)),
            end: Location::Grapheme(Grapheme(cursor_pos)),
        },
    });
    ws.enter_frame();
    ws.enter_frame();

    // Scroll back toward the middle so the cursor is offscreen below.
    {
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
            .handle(&content, Action::ScrollByPixels(-800.0));
    }
    ws.enter_frame();
    let before = ws.editor.edit.scroll_area.stored_offset();

    // Unfold by removing the fold tag — same shape as `apply_fold`'s
    // unfold path (no cursor adjustment).
    ws.push(Event::Replace {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(target)),
            end: Location::Grapheme(Grapheme(target + fold_tag.len())),
        },
        text: "".to_string(),
        advance_cursor: false,
    });
    ws.enter_frame();
    ws.enter_frame();
    let after = ws.editor.edit.scroll_area.stored_offset();

    assert_eq!(before, after, "unfold tap moved scroll: before={:?}, after={:?}", before, after,);
}

/// Every cursor offset renders, even in a deeply-nested list whose
/// continuation line's indentation is redistributed across gutter columns.
/// A column interior is reachable only by keyboard and reveals the node
/// that *owns* that source (the outermost item, whose raw render covers the
/// offset) — not the inner level whose visual column it is.
#[test]
fn cursor_renders_deep_nested_continuation() {
    let md = "a\n* a\n  * a\n    * a\n      b\n";
    let mut ws = TestEditor::new(md);
    ws.enter_frame();
    let last = ws
        .editor
        .edit
        .renderer
        .buffer
        .current
        .segs
        .last_cursor_position();
    let mut invisible = Vec::new();
    for o in 0..=last.0 {
        let g = Grapheme(o);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(g),
                end: Location::Grapheme(g),
            },
        });
        ws.enter_frame();
        if ws.editor.edit.cursor_line(g).is_none() {
            invisible.push(o);
        }
    }
    assert!(invisible.is_empty(), "invisible cursor offsets: {invisible:?}");
}

/// A trailing whitespace-only line is reachable. Such a line gets absorbed
/// into the outer list item's sourcepos (past all its child blocks) and
/// renders via post-spacing, which strips it as the item's indentation
/// prefix. Without a gutter fragment for that prefix the cursor there has
/// no on-screen representation.
#[test]
fn cursor_renders_trailing_whitespace_line() {
    for md in ["a\n ", "a\n* a\n  * a\n    * a\n      b\n "] {
        let mut ws = TestEditor::new(md);
        ws.enter_frame();
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        let mut invisible = Vec::new();
        for o in 0..=last.0 {
            let g = Grapheme(o);
            ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(g),
                    end: Location::Grapheme(g),
                },
            });
            ws.enter_frame();
            if ws.editor.edit.cursor_line(g).is_none() {
                invisible.push(o);
            }
        }
        assert!(invisible.is_empty(), "md={md:?}: invisible cursor offsets: {invisible:?}");
    }
}

/// cmd+left/right treat the syntax|text boundary (text start) as an interior
/// stop on a prefixed line. cmd+left walks content start → text start → source
/// line start; cmd+right walks source line start → text start → row end. The
/// outer endpoints are idempotent.
#[test]
fn cmd_line_jump_gutter() {
    // `  * abc` → source line start 4, text start 8, row end 11;
    //            indent col (4,6), marker col (6,8), content (8,11).
    let mut ws = TestEditor::new("* a\n  * abc\n");
    ws.enter_frame();

    let jump = |ws: &mut TestEditor, from: usize, backwards: bool| -> usize {
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(Grapheme(from)),
                end: Location::Grapheme(Grapheme(from)),
            },
        });
        ws.enter_frame();
        ws.push(Event::Select {
            region: Region::ToAdvance {
                advance: Advance::To(Bound::Line),
                backwards,
                extend_selection: false,
            },
        });
        ws.enter_frame();
        ws.editor.edit.renderer.buffer.current.selection.1.0
    };

    // cmd+left: content → text start → source line start
    assert_eq!(jump(&mut ws, 9, true), 8, "content cmd+left → text start");
    assert_eq!(jump(&mut ws, 8, true), 4, "text start cmd+left → source line start");
    assert_eq!(jump(&mut ws, 4, true), 4, "cmd+left idempotent at source line start");

    // cmd+right: source line start → text start → row end
    assert_eq!(jump(&mut ws, 4, false), 8, "source line start cmd+right → text start");
    assert_eq!(jump(&mut ws, 8, false), 11, "text start cmd+right → wrap row end");
    assert_eq!(jump(&mut ws, 11, false), 11, "cmd+right idempotent at wrap row end");

    // a cursor mid-gutter resolves to the same two stops
    assert_eq!(jump(&mut ws, 6, true), 4, "gutter cmd+left → source line start");
    assert_eq!(jump(&mut ws, 6, false), 8, "gutter cmd+right → text start");
}

/// Gutter prefix units (markers, per-level indentation) are words, so word
/// navigation and double-click treat each as a unit rather than splitting a
/// marker into `*` + ` ` or dropping the indentation.
#[test]
fn gutter_columns_are_words() {
    let mut ws = TestEditor::new("* a\n");
    ws.enter_frame();
    assert!(
        ws.editor
            .edit
            .renderer
            .bounds
            .words
            .contains(&(Grapheme(0), Grapheme(2))),
        "marker should be a word: {:?}",
        ws.editor.edit.renderer.bounds.words
    );

    // Nested: `  * a` → indentation column (6,8) and marker column (8,10).
    let mut ws = TestEditor::new("a\n* a\n  * a\n");
    ws.enter_frame();
    let words = &ws.editor.edit.renderer.bounds.words;
    assert!(words.contains(&(Grapheme(6), Grapheme(8))), "indent column not a word: {words:?}");
    assert!(words.contains(&(Grapheme(8), Grapheme(10))), "marker column not a word: {words:?}");
}

/// Dragging across a bullet marker selects the whole marker and the
/// selection is visibly highlighted. Two regressions guarded here:
/// (1) the moving end resolved the gutter fragment to its range start,
/// collapsing a marker-only drag to an empty selection; (2) `fragment_x`
/// mis-snapped a marker whose range doesn't start at offset 0, so its
/// highlight rect had zero width. The marker sits on the second line so
/// its range starts past 0 (the case (2) missed).
#[test]
fn drag_across_marker_selects_it() {
    let mut ws = TestEditor::new("a\n* a\n");
    ws.enter_frame();
    let edit = &mut ws.editor.edit;

    // `* ` on line 2 spans graphemes 2..4 (after `a\n`).
    let marker_range = (Grapheme(2), Grapheme(4));
    let marker = edit
        .renderer
        .fragments
        .iter()
        .find(|f| f.source_range == marker_range)
        .expect("marker gutter fragment")
        .rect;

    // Drag from the gutter's left edge to its right edge.
    let y = marker.center().y;
    let sel = edit.region_to_range(Region::BetweenLocations {
        start: Location::Pos(egui::Pos2::new(marker.min.x + 1.0, y)),
        end: Location::Pos(egui::Pos2::new(marker.max.x - 1.0, y)),
    });
    assert_eq!(sel, marker_range, "drag should select the whole marker");

    let rects = edit.range_rects(sel);
    assert!(!rects.is_empty(), "marker selection must be visible");
    let widest = rects.iter().map(|r| r.width()).fold(0.0_f32, f32::max);
    assert!(widest > marker.width() * 0.5, "marker highlight too narrow: {widest}");
}

/// Revealing a container's prefix swaps its gutter decoration for the raw
/// marker in the same column; the content stays formatted. So the block's
/// height — and the y of everything below it — must be identical revealed
/// or not. A heading inside the quote makes any height regression large
/// and obvious, and would also expose a stale cached height across the
/// reveal flip.
#[test]
fn reveal_change_layout_consistent() {
    let doc = "> # heading inside block quote\n\nfollowing paragraph.\n";
    let mut ws = TestEditor::new(doc);

    let following = doc.find("following").unwrap();

    // (heading content top y, trailing paragraph top y) with the cursor at
    // `cursor`. Source offset 4 starts the heading text (after `> # `) and 32
    // starts "following paragraph"; both render the same whether or not the
    // `>` prefix is revealed, so both y's must be reveal-invariant.
    let layout = |ws: &mut TestEditor, cursor: usize| -> (f32, f32) {
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(Grapheme(cursor)),
                end: Location::Grapheme(Grapheme(cursor)),
            },
        });
        ws.enter_frame();
        let fragments = &ws.editor.edit.renderer.fragments;
        let top_of = |offset: usize| {
            fragments
                .iter()
                .find(|f| f.source_range.0.0 == offset)
                .unwrap_or_else(|| panic!("no fragment starting at offset {offset}"))
                .rect
                .min
                .y
        };
        (top_of(4), top_of(32))
    };

    // Cursor outside the quote reveals nothing. Offset 1 sits strictly
    // inside the `> ` marker (between `>` and the space) — the interior
    // position that reveals the blockquote syntax.
    let (heading_plain, following_plain) = layout(&mut ws, following);
    let (heading_revealed, following_revealed) = layout(&mut ws, 1);

    assert!(
        (heading_revealed - heading_plain).abs() < 0.5,
        "revealing the prefix moved the heading: {heading_plain:.1} -> {heading_revealed:.1}"
    );
    assert!(
        (following_revealed - following_plain).abs() < 0.5,
        "revealing the prefix moved the trailing paragraph: \
         {following_plain:.1} -> {following_revealed:.1}"
    );
}

/// A revealed marker is right-aligned to its column's content edge by the
/// laid-out marker width. Using the layout's available width (≫ the marker)
/// instead pushes it far off the left edge, so the marker vanishes even
/// though its fragments still exist. Asserts the marker renders on-screen,
/// just left of the content.
#[test]
fn revealed_marker_on_screen() {
    let mut ws = TestEditor::new("12. item\n");
    ws.enter_frame();

    // Offset 2 sits strictly inside the `12. ` marker, revealing it.
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(2)),
            end: Location::Grapheme(Grapheme(2)),
        },
    });
    ws.enter_frame();

    let fragments = &ws.editor.edit.renderer.fragments;
    let marker = fragments
        .iter()
        .find(|f| f.source_range.0.0 == 0)
        .expect("revealed marker fragment");
    let content = fragments
        .iter()
        .find(|f| f.source_range.0.0 == 4)
        .expect("content fragment");

    assert!(marker.rect.left() >= 0.0, "marker off the left edge: left={}", marker.rect.left());
    assert!(
        marker.rect.right() <= content.rect.left() + 0.5,
        "marker overlaps content: marker right={} content left={}",
        marker.rect.right(),
        content.rect.left(),
    );
}

/// A cursor in a container's leading indentation must not reveal its
/// marker — only a cursor in the marker itself reveals. Here the inner
/// item's line begins with the outer item's continuation indentation; a
/// cursor there must not reveal the outer marker (on the line above).
#[test]
fn indentation_cursor_does_not_reveal_marker() {
    let mut ws = TestEditor::new("- a\n  - b\n");
    ws.enter_frame();

    // Offset 5 is interior to the two-space indentation on line 2 (the
    // outer item's continuation prefix), not any marker.
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(5)),
            end: Location::Grapheme(Grapheme(5)),
        },
    });
    ws.enter_frame();

    // The outer marker (offset 0) stays a decoration spacer, not raw glyphs.
    let marker = ws
        .editor
        .edit
        .renderer
        .fragments
        .iter()
        .find(|f| f.source_range.0.0 == 0)
        .expect("outer marker fragment");
    assert!(
        matches!(marker.content, FragmentContent::Spacer),
        "cursor in indentation revealed the marker"
    );
}

/// Async image loads must update `size()` atomically with their `seq`
/// bump. The bug pattern: `seq` bumps when the worker completes, but
/// `size()` reads from a `dims` map that's only populated by `show()`
/// running with the loaded state. So the frame after the bump sees
/// the cache cleared but `size()` still returns the placeholder,
/// populates the height cache from placeholder dims, and the next
/// frames are stuck with that stale value while `show()` paints with
/// the real dims.
///
/// Asserts cached `height(image_paragraph)` matches a freshly-recomputed
/// height after the editor has had two frames to settle past load
/// completion.
#[test]
fn image_load_layout_consistent() {
    use std::sync::{Arc, RwLock};

    use egui::{Color32, ColorImage, Context, ImageData, TextureOptions};
    use lb_rs::Uuid;

    use crate::file_cache::FileCache;
    use crate::resolvers::image_embed::ImageEmbedResolver;
    use crate::widgets::image_cache::ImageCache;
    use crate::workspace::WsPersistentStore;

    let lb = super::harness::build_lb();
    let ctx = Context::default();
    let client = super::super::HttpClient::default();
    let files = Arc::new(RwLock::new(FileCache::empty()));
    let persistence = WsPersistentStore::new(false, format!("/tmp/{}", Uuid::new_v4()).into());

    let image_cache =
        ImageCache::new(ctx.clone(), client, lb.clone(), files.clone(), persistence.clone());
    let image_cache_handle = image_cache.clone();

    let file_id = Uuid::new_v4();
    let embed = Box::new(ImageEmbedResolver::new(image_cache, file_id));

    // Wide image: 1600×100 → very short rendered height when fitted to
    // the editor's content width. Placeholder is 200×200 → much taller.
    // The delta makes a stale cache obviously wrong.
    let url = "test://wide.png";
    let editor = super::super::Editor::new(
        &format!("![image]({url})\n\nfollowing\n"),
        file_id,
        None,
        super::super::MdResources {
            ctx: ctx.clone(),
            core: lb,
            persistence,
            link_resolver: Box::new(()),
            embeds: embed,
            files,
        },
        super::super::MdConfig { readonly: false, ext: "md".to_string(), tablet_or_desktop: true },
    );
    let mut ws = TestEditor::from_editor(editor);

    // Allocate a fake texture and seed the cache with a `Loaded`
    // pointing at it — equivalent to a worker thread completing.
    let texture_id = {
        let pixels = vec![Color32::WHITE; 1600 * 100];
        let image = ImageData::Color(Arc::new(ColorImage::new([1600, 100], pixels)));
        let tex_mgr = ctx.tex_manager();
        let id = tex_mgr
            .write()
            .alloc("test_image".into(), image, TextureOptions::default());
        id
    };
    image_cache_handle.complete_load(url, Ok(texture_id));

    // Two frames: first triggers `embeds_updated` and clears the cache;
    // the second renders against the (potentially stale) populated cache.
    ws.enter_frame();
    ws.enter_frame();

    let arena = comrak::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let image_paragraph = root.children().next().expect("image paragraph");
    let cached_height = ws.editor.edit.renderer.height(image_paragraph);
    ws.editor
        .edit
        .renderer
        .layout_cache
        .height
        .borrow_mut()
        .clear();
    let fresh_height = ws.editor.edit.renderer.height(image_paragraph);

    assert!(
        (cached_height - fresh_height).abs() < 1.0,
        "image paragraph height stale after load: cached={cached_height} fresh={fresh_height}"
    );
}

#[test]
fn shift_tab_innermost_nested_list_one_level() {
    let doc = "- a\n  - b\n    - c\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();

    // Cursor on the innermost item's content ("c" is at offset 16).
    let cursor = Grapheme(16);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();

    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();

    let after = ws.get_text().to_string();
    assert_eq!(after, "- a\n  - b\n  - c\n", "expected one level out (`  - c`); got:\n{after:?}",);
}

/// Regression tests for the user-reported "tab on item N causes item
/// N+1 to look de-indented" scenarios. Both fail under the old
/// grapheme-based `has_prefix_on_first_selected_line` check; both
/// must be no-ops with the structural ancestor check.
#[test]
fn indent_no_op_when_already_inside_prior_container() {
    let cases: &[(&str, &str, usize)] = &[
        // 4-deep nested list, cursor on the 3rd row. Indenting it
        // would make the 4th row appear to deindent (was nested in
        // 3rd, now sibling).
        ("4-deep, cursor on 3rd row", "* a\n  * b\n    * c\n      * d\n", 12),
        // Two siblings at over-indented level (col 6) inside a level-1
        // item. The first sibling has no previous sibling at its
        // level, so tab should be a no-op.
        ("two siblings at col 6, cursor on first", "* a\n  * b\n      * c\n      * d\n", 12),
    ];
    for (desc, input, cursor_offset) in cases {
        let mut ws = TestEditor::new(input);
        ws.enter_frame();
        let cursor = Grapheme(*cursor_offset);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(cursor),
                end: Location::Grapheme(cursor),
            },
        });
        ws.enter_frame();
        ws.push(Event::Indent { deindent: false });
        ws.enter_frame();
        let after = ws.get_text().to_string();
        assert_eq!(
            &after, input,
            "case {desc:?}: expected no-op but got mutation:\n  before: {input:?}\n  after:  {after:?}",
        );
    }
}

/// Regression: toggling list style on an empty list-item line (e.g.
/// `* ` then Cmd+Shift+9) used to insert a second marker on top of
/// the existing one, producing `* * [ ] `. The fallback in
/// `toggle_style` now strips an existing item marker first when the
/// target style is a list.
#[test]
fn toggle_list_style_on_empty_item_switches() {
    use crate::tab::markdown_editor::input::Bound;
    use comrak::nodes::{ListType, NodeList, NodeValue};

    let to_task = || Event::ToggleStyle {
        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
        style: NodeValue::List(NodeList {
            list_type: ListType::Bullet,
            is_task_list: true,
            ..Default::default()
        }),
    };
    let to_ordered = || Event::ToggleStyle {
        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
        style: NodeValue::List(NodeList { list_type: ListType::Ordered, ..Default::default() }),
    };

    // (description, input, cursor, event, expected_text, expected_cursor)
    let cases: &[(&str, &str, usize, Event, &str, usize)] = &[
        ("empty bullet → task", "* ", 2, to_task(), "* [ ] ", 6),
        ("empty bullet → ordered", "* ", 2, to_ordered(), "1. ", 3),
        ("non-empty bullet → task", "* a", 3, to_task(), "* [ ] a", 7),
        ("empty ordered → task", "1. ", 3, to_task(), "* [ ] ", 6),
    ];
    for (desc, input, cursor, ev, expected_text, expected_cursor) in cases {
        let mut ws = TestEditor::new(input);
        ws.enter_frame();
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(Grapheme(*cursor)),
                end: Location::Grapheme(Grapheme(*cursor)),
            },
        });
        ws.enter_frame();
        ws.push(ev.clone());
        ws.enter_frame();
        let got_text = ws.get_text().to_string();
        let got_sel = ws.editor.edit.renderer.buffer.current.selection;
        assert_eq!(
            got_text, *expected_text,
            "case {desc:?}: text input={input:?} got={got_text:?} expected={expected_text:?}",
        );
        assert_eq!(
            got_sel,
            (Grapheme(*expected_cursor), Grapheme(*expected_cursor)),
            "case {desc:?}: cursor expected={expected_cursor} got={got_sel:?}",
        );
    }
}

/// Regression: changing the list type of a *nested* item used to read
/// the wrong list. `unapply_block` walked the whole ancestor chain, so a
/// nested task item under a bullet list matched the outer bullet list and
/// the toggle removed the inner item's marker instead of converting it.
/// It must stop at the nearest enclosing list (the inner task list) and,
/// since that's a different type, perform a type change.
#[test]
fn toggle_bullet_on_nested_task_item_changes_type() {
    use comrak::nodes::{ListType, NodeList, NodeValue};

    let to_bullet = Event::ToggleStyle {
        region: Region::Selection,
        style: NodeValue::List(NodeList {
            list_type: ListType::Bullet,
            is_task_list: false,
            ..Default::default()
        }),
    };

    let doc = "* x\n  * [x] y";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();
    // cursor inside "y"
    let cursor = Grapheme(13);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(to_bullet);
    ws.enter_frame();
    assert_eq!(ws.get_text(), "* x\n  * y");
}

/// Regression for the user-reported "shift-tab then tab needs many
/// undos with cursor-only steps". With per-frame undo units, the two
/// commands are independent: one undo restores the post-shift-tab
/// state; another restores the original.
#[test]
fn shift_tab_then_tab_undoes_in_two_steps() {
    let doc = "* a\n  * b\n    * c\n    * d\n      * e\n";
    let mut ws = TestEditor::new(doc);
    ws.enter_frame();
    let cursor = Grapheme(8);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();
    let post_shift_tab = ws.get_text().to_string();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();
    // Tab no longer reverses shift-tab (deindent drags children,
    // indent doesn't); the point here is undo *granularity*.
    ws.push(Event::Undo);
    ws.enter_frame();
    assert_eq!(ws.get_text(), post_shift_tab);
    ws.push(Event::Undo);
    ws.enter_frame();
    assert_eq!(ws.get_text(), doc);
}

/// Repeated shift-tab on a parent item must not orphan its
/// descendants — the user-reported "deindent the parent twice and
/// the deepest line stops being a list item" bug. After each
/// deindent, every line that was a list item should still be one.
/// Repeated shift-tab on a parent item drags its descendants along —
/// the reported "deindent twice and the deepest line becomes orphaned
/// at over-indent" bug. Without the cascade, line 3 would stay at
/// col 6 after line 2 reaches col 0.
#[test]
fn shift_tab_repeated_preserves_descendants() {
    let mut ws = TestEditor::new("* a\n  * b\n    * c\n      * d\n");
    ws.enter_frame();
    let cursor = Grapheme(10); // line 2 ("    * c")
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    for _ in 0..2 {
        ws.push(Event::Indent { deindent: true });
        ws.enter_frame();
    }
    assert_eq!(ws.get_text(), "* a\n  * b\n* c\n  * d\n");
}

/// Shift-tab on a top-level container (no outer container of the
/// same kind to escape into) is a no-op — for bq/alert just as for
/// items. The bq case used to demote `> quote` to a paragraph.
#[test]
fn shift_tab_top_level_container_is_noop() {
    let cases: &[(&str, &str, usize)] = &[
        // (description, input, cursor_offset)
        ("top-level item", "* a\n", 2),
        ("top-level bq", "> quote\n", 4),
        ("top-level bq, no space marker", ">quote\n", 3),
        // Multi-line top-level bq, cursor on continuation — also
        // a no-op since the bq has no outer bq to escape to.
        ("multi-line top-level bq", "> foo\n> bar\n", 9),
    ];
    for (desc, input, cursor_offset) in cases {
        let mut ws = TestEditor::new(input);
        ws.enter_frame();
        let cursor = Grapheme(*cursor_offset);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(cursor),
                end: Location::Grapheme(cursor),
            },
        });
        ws.enter_frame();
        ws.push(Event::Indent { deindent: true });
        ws.enter_frame();
        let after = ws.get_text().to_string();
        assert_eq!(
            &after, input,
            "case {desc:?}: expected no-op but got mutation:\n  before: {input:?}\n  after:  {after:?}",
        );
    }
}

/// Tab on a parent indents only that line. Its tight child is not
/// dragged along — it keeps its absolute indentation and re-parents
/// to a sibling (`target parent` becomes a child of `anchor`;
/// `slack child` becomes its sibling under `anchor`). This is the
/// promote-don't-cascade behavior; the deindent direction still
/// cascades (see `shift_tab_repeated_preserves_descendants`).
#[test]
fn tab_promotes_target_child_becomes_sibling() {
    let mut ws = TestEditor::new("- anchor\n- target parent\n  - slack child\n");
    ws.enter_frame();
    let cursor = Grapheme(11); // line 1 ("- target parent")
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "- anchor\n  - target parent\n  - slack child\n");
}

/// `      * c` is 4+ cols past `* b`'s content column, so comrak
/// parses it as an indented code block, not a child list item. With
/// no list child to align to, indent falls back to one unit and the
/// code-block line is untouched.
#[test]
fn tab_indent_no_list_child_uses_one_unit() {
    let mut ws = TestEditor::new("* a\n* b\n      * c\n");
    ws.enter_frame();
    let cursor = Grapheme(5); // line 1 ("* b")
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "* a\n  * b\n      * c\n");
}

/// User-reported: indenting `child four` (which has an over-indented
/// child `asdf` at col 4) must align it to col 4, not +2 — otherwise
/// `asdf` stays its child and is promoted to level 3. After the fix
/// `child four` and `asdf` are both level-2 siblings under `bullet
/// parent`; `asdf`'s level is unchanged.
#[test]
fn tab_indents_parent_to_child_keeps_child_level() {
    let mut ws = TestEditor::new("- bullet parent\n- child four\n    - asdf\n");
    ws.enter_frame();
    let cursor = Grapheme(18); // line 1 ("- child four")
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "- bullet parent\n    - child four\n    - asdf\n");
}

/// Newline at the end of a nested list item must start the sibling at
/// the prior item's actual source indentation, not a shallower depth
/// reconstructed from ancestor marker widths.
#[test]
fn newline_preserves_nested_item_indentation() {
    let mut ws = TestEditor::new("- a\n    - b\n");
    ws.enter_frame();
    let cursor = Grapheme(11); // end of "    - b"
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Newline { shift: false });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "- a\n    - b\n    - \n");
}

/// Shift-tab on an over-indented nested item pops it fully out one
/// level (to its parent's column), not just by the marker width. The
/// user-reported bug: `child indented four` (col 4 under a col-0
/// parent whose content starts at col 2) only dropped to col 2,
/// staying nested and re-parsing the following sibling as its child.
/// It must reach col 0. (`asdf` then nests under the outdented item —
/// standard markdown for a following line indented past it — but is
/// no longer pushed a level deeper.)
#[test]
fn shift_tab_pops_overindented_item_full_level() {
    let mut ws = TestEditor::new("- bullet parent\n    - child indented four\n    - asdf\n");
    ws.enter_frame();
    let cursor = Grapheme(22); // line 1 ("    - child indented four")
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "- bullet parent\n- child indented four\n    - asdf\n");
}

/// Multi-line shift-tab: selecting a range that spans a parent and
/// some of its descendants should still produce a coherent result.
/// The cascade dedup makes already-shifted descendants no-op so they
/// don't double-deindent.
#[test]
fn multi_line_shift_tab_preserves_structure() {
    // Select lines 1-2 ("  * b" through "    * c") and shift-tab.
    // Each gets one level off; descendant cascade applies once per
    // unique line.
    let mut ws = TestEditor::new("* a\n  * b\n    * c\n");
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(4)),
            end: Location::Grapheme(Grapheme(15)),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();
    assert_eq!(ws.get_text(), "* a\n* b\n  * c\n");
}

/// Hand-crafted shift-tab cases for container types whose nesting
/// semantics don't fit the generator's "uniform 2-col-per-level"
/// shape: task items (whose `[ ]` is content not marker per comrak,
/// and whose `extension_own_prefix` subtracts 4), blockquotes (whose
/// marker is non-whitespace), and alerts (blockquote-shaped).
#[test]
fn shift_tab_mixed_container_cases() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // (description, input, cursor_offset, expected_output)
        //
        // Each shift-tab removes ONE level of nesting: items/task
        // items drop to the escaped parent's own column (one padding
        // level when minimally indented; the full over-indent when
        // not), blockquotes/alerts drop one marker. Tab indent gets
        // expanded to spaces as a side effect of the col-precise
        // rewrite.
        //
        // ─── plain items + task items ──────────────────────────────
        ("item containing task item", "- a\n  - [ ] b\n", 13, "- a\n- [ ] b\n"),
        ("task item containing item", "- [ ] a\n  - b\n", 13, "- [ ] a\n- b\n"),
        // Tab-indented task item nested in plain item — the tab is
        // one level of nesting under `a`; shift-tab pops it fully out
        // to `a`'s column (col 0), same as the 2-space case above.
        ("tab-indented task item under item", "- a\n\t- [ ] b\n", 11, "- a\n- [ ] b\n"),
        //
        // ─── ordered lists (3-col padding for `1. `) ───────────────
        ("ordered nested in plain", "- a\n  1. b\n", 9, "- a\n1. b\n"),
        ("plain nested in ordered", "1. a\n   - b\n", 10, "1. a\n- b\n"),
        ("two-deep ordered", "1. a\n   1. b\n", 11, "1. a\n1. b\n"),
        //
        // ─── numbered task items (`1. [ ]`) ────────────────────────
        ("plain nested in numbered task", "1. [ ] a\n   - b\n", 14, "1. [ ] a\n- b\n"),
        //
        // ─── blockquotes / alerts ──────────────────────────────────
        // Nested bq/alert deindent strips one marker level (the
        // innermost `>`/`> `). Top-level bq is no-op — covered in
        // `shift_tab_top_level_container_is_noop`.
        ("nested blockquote", "> > foo\n", 5, "> foo\n"),
        ("triple blockquote", "> > > foo\n", 7, "> > foo\n"),
        // Bq inside an item: the bq has no outer bq, the item is
        // top-level, so shift-tab is a no-op.
        ("item containing blockquote", "- > foo\n", 5, "- > foo\n"),
        // Mixed-spacing nested bq — outer bq's marker is `>` (no
        // following space), inner bq's marker is `> ` (with space).
        // Removing the inner marker leaves outer intact.
        ("nested bq mixed spacing", ">> foo\n", 4, ">foo\n"),
        // Both bq markers use `>` (no space).
        ("nested bq both no space", ">>foo\n", 3, ">foo\n"),
        // Top-level alert: body line shift-tab is a no-op (alert
        // has no outer bq/alert to escape into).
        ("alert body", "> [!NOTE]\n> body\n", 14, "> [!NOTE]\n> body\n"),
        //
        // ─── no-ops ────────────────────────────────────────────────
        ("top-level item", "- a\n", 2, "- a\n"),
    ];
    for (desc, input, cursor_offset, expected) in cases {
        let mut ws = TestEditor::new(input);
        ws.enter_frame();
        let cursor = Grapheme(*cursor_offset);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(cursor),
                end: Location::Grapheme(cursor),
            },
        });
        ws.enter_frame();
        ws.push(Event::Indent { deindent: true });
        ws.enter_frame();
        let after = ws.get_text().to_string();
        assert_eq!(
            &after, expected,
            "case {desc:?}: input={input:?} cursor={cursor_offset} got={after:?} expected={expected:?}",
        );
    }
}

#[test]
fn find_esc_closes_widget() {
    use super::harness::key_press;

    let mut ws = TestEditor::new("alpha beta alpha gamma");
    let find_id = ws.editor.find.id;

    // open find like the toolbar does, then settle a couple frames so the
    // search field actually holds focus with its steady-state lock filter.
    ws.editor.find.open_requested = true;
    ws.enter_frame();
    ws.editor.find.term = Some("alpha".to_string());
    ws.enter_frame_with_input(vec![]);
    ws.enter_frame_with_input(vec![]);

    assert!(ws.editor.find.term.is_some(), "find open before esc");
    let focused = ws.has_focus(find_id);

    // Reproduce the real-app state: the search field's GlyphonTextEdit leaves
    // its focus-lock filter at `escape: false`, so egui's begin_pass surrenders
    // focus on the Esc press before `Find::show` runs (#4646).
    ws.editor.edit.renderer.ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            find_id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: false,
                escape: false,
            },
        );
    });

    ws.enter_frame_with_input(key_press(egui::Key::Escape, egui::Modifiers::NONE));

    assert!(
        ws.editor.find.term.is_none(),
        "esc should close find (search field focused before esc = {focused})",
    );
}
