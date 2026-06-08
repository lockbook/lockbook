//! Property tests for `MdRender`, the markdown renderer shared by `MdLabel`
//! and `MdEdit`. The audit table below lists every property with its corpus
//! and seed count; each `_check` body documents the invariant it guards.
//!
//! Docs come from `super::doc_gen::gen_doc` (structured GFM) or `fuzz_text`
//! (random UTF-8, used by the no-panic property). On failure the byte buffer
//! is delta-debugged and the shrunken input printed.

use std::cell::OnceCell;
use std::sync::{Arc, Mutex};

use comrak::Arena;
use egui::{Pos2, RawInput, Rect, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt, RangeExt};

use super::super::MdRender;
use super::doc_gen::{Features, InlineFeatures, gen_doc};
use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
use crate::test_utils::byte_source::ByteSource;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt};

/// Runs `check` over `seeds` docs generated from `features` (the corpus is
/// the visible argument). The same `features` drives the failure
/// reproducer, so the printed markdown matches the corpus that failed.
fn run<C, E>(check: C, features: Features, seeds: u64)
where
    C: Fn(&[u8], &Features) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(
        seeds,
        128,
        |b| check(b, &features),
        |b| {
            let mut src = ByteSource::new(b);
            let md = gen_doc(&mut src, &features);
            format!(
                "markdown ({} chars, {} bytes, debug-escaped):\n{md:?}\nmarkdown (literal):\n{md}",
                md.chars().count(),
                md.len(),
            )
        },
    );
}

/// Runs `check` over random-UTF-8 fuzz input — no `Features` corpus.
fn run_fuzz<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 128, check, |b| {
        let mut src = ByteSource::new(b);
        let md = fuzz_text(&mut src);
        format!("fuzz text ({} bytes, debug-escaped):\n{md:?}", md.len())
    });
}

// ════════════════════════ property audit table ════════════════════════
// property → corpus → seeds. Corpus is `Features::all()` unless a carve-out
// is shown; `run_fuzz` is the random-UTF-8 corpus. Bodies are below, under
// the implementations banner.

// no panic
#[test]
fn no_panic() {
    run_fuzz(no_panic_check, 1000);
}
#[test]
fn no_panic_structured() {
    run(no_panic_structured_check, Features::all(), 1000);
}

// layout geometry
#[test]
fn width_monotonic() {
    run(width_monotonic_check, Features::all(), 1000);
}
#[test]
fn fragment_rect_disjoint() {
    run(fragment_rect_disjoint_check, Features::all(), 1000);
}
#[test]
fn fragment_content_inset_matches_bg() {
    run(fragment_content_inset_matches_bg_check, Features::all(), 1000);
}
#[test]
fn render_deterministic() {
    run(render_deterministic_check, Features::all(), 1000);
}
#[test]
fn resize_idempotence() {
    run(resize_idempotence_check, Features::all(), 1000);
}
#[test]
fn drag_never_reveals() {
    run(drag_never_reveals_check, Features::all(), 1000);
}

// cursor hit-testing
#[test]
fn cursor_x_roundtrip() {
    run(cursor_x_roundtrip_check, Features::all(), 1000);
}
#[test]
fn click_in_bg_padding_zone() {
    run(click_in_bg_padding_zone_check, Features::all(), 1000);
}

// content fidelity
#[test]
fn content_coverage() {
    run(content_coverage_check, Features::all(), 1000);
}
#[test]
fn parse_equivalence() {
    run(parse_equivalence_check, Features::all(), 1000);
}
#[test]
fn tab_space_equivalence() {
    run(tab_space_equivalence_check, Features::all(), 1000);
}

// glyph painting — tier_b excludes complex scripts / long tokens (font-
// fallback px drift, by design) and nested containers (columns shift under
// reveal); a failure on tier_b is a real walker bug.
#[test]
fn glyph_in_fragment() {
    run(glyph_in_fragment_check, Features::tier_b(), 1000);
}
#[test]
fn glyph_in_render_area() {
    // tier_b plus inline math off: heading-scaled `$…$` overshoots at narrow
    // widths (seed 1214: `### $**`foo` foo* foo …**$`).
    let b = Features::tier_b();
    let f = Features { inlines: InlineFeatures { math: false, ..b.inlines }, ..b };
    run(glyph_in_render_area_check, f, 1000);
}

// ═══════════════════════════ implementations ═══════════════════════════

thread_local! {
    /// Per-thread cached font system. Loading fontdb + constructing
    /// `glyphon::FontSystem` is tens of ms — amortise across every
    /// `test_renderer` call. Per-thread (not process-global) so parallel
    /// test threads don't serialise on the inner `Mutex<FontSystem>`
    /// during cosmic-text shaping.
    static FONT_SYSTEM: OnceCell<Arc<Mutex<glyphon::FontSystem>>> = const { OnceCell::new() };
}

fn cached_font_system() -> Arc<Mutex<glyphon::FontSystem>> {
    FONT_SYSTEM.with(|cell| {
        cell.get_or_init(|| {
            let mut db = glyphon::fontdb::Database::new();
            crate::font::load(&mut db);
            Arc::new(Mutex::new(glyphon::FontSystem::new_with_locale_and_db("en-US".into(), db)))
        })
        .clone()
    })
}

/// Builds a minimally-configured `MdRender` suitable for test calls. Mirrors
/// the setup `MdLabel` expects from its caller (theme + font system).
pub(super) fn test_renderer(md: &str) -> MdRender {
    let r = MdRender::test(md);
    r.ctx.set_lb_theme(Theme::default(Mode::Dark));
    r.ctx
        .data_mut(|d| d.insert_temp(egui::Id::NULL, cached_font_system()));
    let glyphon_cache = Arc::new(Mutex::new(crate::widgets::glyphon_cache::GlyphonCache::new()));
    r.ctx
        .data_mut(|d| d.insert_temp(egui::Id::NULL, glyphon_cache));
    r
}

/// Returns the rendered height of `md` at `width`, using a fresh renderer.
fn render_height(md: &str, width: f32) -> f32 {
    let mut r = test_renderer(md);
    r.set_width(width);
    let arena = Arena::new();
    let root = r.reparse(&arena);
    r.height(root)
}

/// Counts each comrak `NodeValue` discriminant in the parse of `md`. A
/// render-independent fingerprint of the parse structure: two docs
/// producing the same counts have the same shape (same number of lists,
/// paragraphs, headings, etc.). Useful for property tests that assert
/// transformations preserve structure (e.g. tab → space normalization).
fn ast_node_counts(md: &str) -> std::collections::BTreeMap<&'static str, usize> {
    let mut r = test_renderer(md);
    let arena = Arena::new();
    let root = r.reparse(&arena);
    let mut counts = std::collections::BTreeMap::new();
    for node in root.descendants() {
        let name = node_value_name(&node.data.borrow().value);
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
}

fn node_value_name(v: &comrak::nodes::NodeValue) -> &'static str {
    use comrak::nodes::NodeValue::*;
    match v {
        Document => "Document",
        FrontMatter(_) => "FrontMatter",
        BlockQuote => "BlockQuote",
        List(_) => "List",
        Item(_) => "Item",
        DescriptionList => "DescriptionList",
        DescriptionItem(_) => "DescriptionItem",
        DescriptionTerm => "DescriptionTerm",
        DescriptionDetails => "DescriptionDetails",
        CodeBlock(_) => "CodeBlock",
        HtmlBlock(_) => "HtmlBlock",
        Paragraph => "Paragraph",
        Heading(_) => "Heading",
        ThematicBreak => "ThematicBreak",
        FootnoteDefinition(_) => "FootnoteDefinition",
        Table(_) => "Table",
        TableRow(_) => "TableRow",
        TableCell => "TableCell",
        Text(_) => "Text",
        TaskItem(_) => "TaskItem",
        SoftBreak => "SoftBreak",
        LineBreak => "LineBreak",
        Code(_) => "Code",
        HtmlInline(_) => "HtmlInline",
        Raw(_) => "Raw",
        Emph => "Emph",
        Strong => "Strong",
        Strikethrough => "Strikethrough",
        Superscript => "Superscript",
        Subscript => "Subscript",
        Link(_) => "Link",
        Image(_) => "Image",
        FootnoteReference(_) => "FootnoteReference",
        ShortCode(_) => "ShortCode",
        MultilineBlockQuote(_) => "MultilineBlockQuote",
        Escaped => "Escaped",
        Math(_) => "Math",
        WikiLink(_) => "WikiLink",
        Underline => "Underline",
        SpoileredText => "SpoileredText",
        EscapedTag(_) => "EscapedTag",
        Highlight => "Highlight",
        Alert(_) => "Alert",
        Subtext => "Subtext",
    }
}

struct GalleySnapshot {
    rect: Rect,
    /// True when the fragment paints text glyphs; false for `Spacer`s
    /// (gutter prefix columns, bg pads, anchors), which carry no text.
    is_glyphs: bool,
}

/// Runs one headless render frame for `md` at `width` (optionally with a
/// selection set) and returns whatever `f` extracts from the settled
/// renderer. Centralizes the `ctx.run` → `CentralPanel` → reparse → clear
/// → `show_block` ritual every render property shares; `f` runs after
/// `show_block`, with `r.fragments` / `r.text_areas` populated. `r` keeps
/// that state after the call, so callers needing a post-frame AST walk can
/// reparse and inspect it.
pub(super) fn render_frame<T>(
    r: &mut MdRender, width: f32, selection: Option<(Grapheme, Grapheme)>,
    f: impl FnOnce(&mut MdRender) -> T,
) -> T {
    let ctx = r.ctx.clone();
    // `ctx.run`'s closure is `FnMut`, so `f` (an `FnOnce`) is parked in an
    // `Option` and taken on the single frame the body runs.
    let mut f = Some(f);
    let mut out = None;
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            r.dark_mode = ui.style().visuals.dark_mode;
            r.set_width(width);
            if let Some(sel) = selection {
                r.buffer.current.selection = sel;
            }
            let arena = Arena::new();
            let root = r.reparse(&arena);
            let height = r.height(root);
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            r.fragments.clear();
            r.bounds.wrap_lines.clear();
            r.text_areas.clear();
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                r.show_block(ui, root, Pos2::ZERO);
            });
            out = Some((f.take().expect("render_frame body runs once"))(r));
        });
    });
    out.expect("CentralPanel body runs synchronously")
}

/// Paints `md` at `width` with an optional selection and snapshots the
/// resulting galleys.
fn render_galleys(
    md: &str, width: f32, selection: Option<(Grapheme, Grapheme)>,
) -> Vec<GalleySnapshot> {
    let mut r = test_renderer(md);
    render_frame(&mut r, width, selection, |r| {
        r.fragments
            .iter()
            .map(|f| GalleySnapshot {
                rect: f.rect,
                is_glyphs: matches!(f.content, FragmentContent::Glyphs { .. }),
            })
            .collect()
    })
}

/// Generates a short UTF-8 string from arbitrary bytes. Used to fuzz the
/// renderer for panics on weird input.
fn fuzz_text(src: &mut ByteSource) -> String {
    let len = src.bias(&[1, 2, 3, 3, 2, 1]) * 16;
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(src.draw(256) as u8);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Per CommonMark/GFM §2.2: "Tabs in lines are not expanded to spaces.
/// However, in contexts where whitespace helps to define block structure,
/// tabs behave as if they were replaced by spaces with a tab stop of 4
/// characters." Inline (mid-line, post-content) tabs are kept literal —
/// expanding them would change the rendered text. Only tabs in the leading
/// whitespace of a line are structural; those we expand.
///
/// Tab stops sit at columns 0, 4, 8, … so a tab advances to the next stop.
/// The number of spaces a tab produces depends on its column position.
fn tab_to_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut col = 0usize;
    let mut at_line_start = true;
    for c in s.chars() {
        match c {
            '\n' | '\r' => {
                out.push(c);
                col = 0;
                at_line_start = true;
            }
            '\t' if at_line_start => {
                let next_stop = (col / 4 + 1) * 4;
                for _ in col..next_stop {
                    out.push(' ');
                }
                col = next_stop;
            }
            ' ' => {
                out.push(c);
                col += 1;
            }
            _ => {
                out.push(c);
                col += 1;
                at_line_start = false;
            }
        }
    }
    out
}

/// Property: every text-bearing leaf node with non-whitespace content
/// manifests in the rendering — at least one galley exists whose source
/// range overlaps the node's range. Whitespace-only text nodes are
/// exempt: when one falls at a wrap point, it's "shown" as the line
/// break itself rather than as a galley, and that's expected.
///
/// "Text-bearing" = `Text`, `Code`, `CodeBlock`, `HtmlBlock`,
/// `HtmlInline`, `Math`, `Raw`: leaves whose source bytes ARE the user's
/// content. Containers, structural separators (`ThematicBreak`,
/// `SoftBreak`, `LineBreak`), and atomic decorations (`ShortCode` →
/// emoji) are excluded — they manifest as layout/decoration rather than
/// text galleys.
///
/// The editor takes liberties (hides syntax markers, paints decorations
/// rather than text), but a *non-whitespace* text node that produces no
/// galley is content lost. Catches the bug class behind "task item
/// containing an indented code block renders the code-block content as
/// nothing".
fn content_coverage_check_md(md: &str) -> Result<(), &'static str> {
    use comrak::nodes::NodeValue::*;
    // Render once via the editor's main show path and inspect
    // fragments to verify every text-bearing node manifests.
    let mut r = test_renderer(md);
    let fragments: Vec<(Grapheme, Grapheme)> =
        render_frame(&mut r, 800.0, None, |r| r.fragments.iter().map(|f| f.source_range).collect());
    let arena = Arena::new();
    let root = r.reparse(&arena);
    for node in root.descendants() {
        let bears_text = matches!(
            &node.data.borrow().value,
            Text(_) | Code(_) | CodeBlock(_) | HtmlBlock(_) | HtmlInline(_) | Math(_) | Raw(_)
        );
        if !bears_text {
            continue;
        }
        // Folded nodes (and their descendants) intentionally render
        // nothing — `<!-- {"fold":true} -->` after a heading hides
        // following siblings, so a Text inside a hidden paragraph
        // having no fragment is correct.
        if node
            .ancestors()
            .any(|a| a.parent().is_some() && r.hidden_by_fold(a))
        {
            continue;
        }
        let range = r.node_range(node);
        let text = &r.buffer[range];
        // Whitespace-only text nodes (e.g. a single-space `Text` between
        // two inlines like `… ![alt](url) `code` …`) often fall on a wrap
        // point — cosmic-text wraps the line *at* the space, so the
        // space "manifests" as the line break itself rather than as a
        // fragment. `char::is_whitespace` (not `is_ascii_whitespace`) so
        // NBSP and other Unicode whitespace are also exempted.
        if text.chars().all(char::is_whitespace) {
            continue;
        }
        let manifests = fragments.iter().any(|f| f.0 < range.1 && f.1 > range.0);
        if !manifests {
            return Err("non-whitespace text-bearing node has no rendered fragment");
        }
    }
    Ok(())
}

fn content_coverage_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    content_coverage_check_md(&md)
}

/// Property: a doc with tab-form indentation parses to the same AST shape
/// as the same doc with tabs normalized to spaces (per CommonMark §2.2).
/// Render-independent — compares only `comrak`'s parse result, so it's not
/// blocked by rendering bugs.
fn parse_equivalence_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    if !md.contains('\t') {
        return Ok(());
    }
    let md_spaces = tab_to_spaces(&md);
    if ast_node_counts(&md) != ast_node_counts(&md_spaces) {
        return Err("tab-form and space-form produce different AST node counts");
    }
    Ok(())
}

/// Property: a doc with tab-form indentation renders equivalently to the
/// same doc with tabs normalized to spaces (per CommonMark §2.2). Compared
/// at a wide width so wrap differences caused by tabs vs spaces having
/// different glyph widths don't muddy the signal.
///
/// - **Height** mismatch ⇒ the two parsed into structurally different
///   documents (extra/missing block).
/// - **Total non-override galley width** mismatch ⇒ leading whitespace
///   leaked into rendered content (the bug class behind "list marker
///   leaks into text" when a container's prefix-stripping doesn't account
///   for tabs).
fn tab_space_equivalence_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    if !md.contains('\t') {
        return Ok(()); // no tabs, no test
    }
    // Skip whitespace-only docs: comrak parses both tab and space forms
    // as empty (no Text/Code/etc nodes), so there's no markdown content
    // to compare. The renderer still draws the raw whitespace as a single
    // galley, but tab-glyph and space-glyph widths differ in cosmic-text
    // — a difference inherent to font shaping, not a markdown-parse bug.
    if md.trim().is_empty() {
        return Ok(());
    }
    let md_spaces = tab_to_spaces(&md);
    let width = 5000.0;
    if (render_height(&md, width) - render_height(&md_spaces, width)).abs() > 0.5 {
        return Err(
            "tab-form and space-form renders disagree on height (likely a tab-indent parse bug)",
        );
    }
    // Text galleys only: gutter prefix `Spacer`s legitimately differ in
    // source structure between tab and space forms (an atomic tab can't be
    // split across nested columns the way spaces can) yet are visually
    // identical — they're not the leaked-text this guards against.
    let sum_w = |s: &str| -> f32 {
        render_galleys(s, width, None)
            .iter()
            .filter(|g| g.is_glyphs)
            .map(|g| g.rect.width())
            .sum()
    };
    if (sum_w(&md) - sum_w(&md_spaces)).abs() > 1.0 {
        return Err(
            "tab-form and space-form renders disagree on total galley width (likely a prefix-stripping leak)",
        );
    }
    Ok(())
}

/// Property: widening the viewport cannot increase rendered height.
fn width_monotonic_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    // Minimum width chosen to keep `gen_doc`'s deepest container nesting
    // (`MAX_BLOCK_DEPTH = 3`, indent 26 each) from triggering the
    // too-narrow early-return in `widget/block/mod.rs`'s `width()` /
    // `height()`. At width 200 a `> > > | … |` table collapses to one
    // empty row — wider then renders normally, breaking monotonicity for
    // reasons unrelated to the wrap algorithm.
    let widths = [400.0, 800.0, 1600.0];

    let mut prev: Option<f32> = None;
    for w in widths {
        let h = render_height(&md, w);
        if let Some(ph) = prev {
            if h > ph {
                return Err("wider width produced greater height");
            }
        }
        prev = Some(h);
    }
    Ok(())
}

/// Property: no two non-override galleys cover overlapping source bytes.
/// Property: no two fragment rects overlap on screen. With
/// `Fragment::rect` as the visual extent (CSS-box reading; see
/// CLAUDE.md), this is the *complete* statement of fragment-level
/// non-overlap — no separate bg-expansion or same-color carve-out
/// needed. If two fragment rects intersect with positive area,
/// that's a layout bug.
pub(super) fn fragment_rect_disjoint_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    const EPS: f32 = 0.01;
    let mut r = test_renderer(md);
    let rects: Vec<Rect> = render_frame(&mut r, width, None, |r| {
        r.fragments
            .iter()
            .map(|f| f.rect)
            .filter(|r| r.width() > EPS && r.height() > EPS)
            .collect()
    });
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let inter = rects[i].intersect(rects[j]);
            if inter.width() > EPS && inter.height() > EPS {
                eprintln!("rects[{i}]={:?} rects[{j}]={:?}", rects[i], rects[j]);
                return Err("two fragment rects overlap on screen");
            }
        }
    }
    Ok(())
}

fn fragment_rect_disjoint_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    fragment_rect_disjoint_check_md(&md, width)
}

/// Property: a click-and-drag selection never reveals captured block
/// syntax. Captured markers + list indentation render as single atomic
/// fragments (`show_block_line_prefixes`), so a pointer hit-test
/// (`pos_to_range`) can only resolve to a fragment edge — never to a
/// position strictly interior to a prefix. The container `reveal`
/// predicate triggers only on such interior positions, so feeding any
/// hit-test result into it must leave every container captured.
///
/// This sweeps real screen points (sampled across every fragment rect,
/// which now includes the gutter columns) through the production
/// `closest_fragment_at_pos` → atomic-edge-snap → `reveal` path. A drag's
/// two endpoints are independent hit-tests, so it suffices to show no
/// single hit-test reveals.
fn drag_never_reveals_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    use comrak::nodes::NodeValue;

    use crate::tab::markdown_editor::widget::utils::NodeValueExt;

    let mut r = test_renderer(md);
    render_frame(&mut r, width, None, |_| {});

    // Sample screen points across every fragment rect (gutter columns
    // included) at their vertical midpoint.
    let mut points: Vec<Pos2> = Vec::new();
    for f in &r.fragments {
        let y = (f.rect.min.y + f.rect.max.y) / 2.0;
        let (l, w) = (f.rect.min.x, f.rect.width().max(1.0));
        for frac in [0.0, 0.25, 0.5, 0.75, 1.0] {
            points.push(Pos2::new(l + w * frac, y));
        }
    }
    if points.is_empty() {
        return Ok(());
    }

    // Resolve each point the way a click/drag does (`pos_to_range`):
    // atomic fragments snap to their whole source range (both edges),
    // anchors to their point, content fragments to a real offset.
    let resolve = |r: &MdRender, p: Pos2| -> Option<(Grapheme, Grapheme)> {
        let idx = r.closest_fragment_at_pos(p)?;
        let frag = &r.fragments[idx];
        Some(if frag.source_range.is_empty() {
            frag.source_range.start().into_range()
        } else if frag.atomic {
            frag.source_range
        } else {
            r.fragment_offset(frag, p.x).into_range()
        })
    };

    let arena = Arena::new();
    let root = r.reparse(&arena);
    let containers: Vec<_> = root
        .descendants()
        .filter(|n| n.parent().is_some() && n.data.borrow().value.is_container_block())
        .collect();
    if containers.is_empty() {
        return Ok(());
    }

    for p in points {
        let Some(rng) = resolve(&r, p) else { continue };
        r.reveal_selection = Some(rng);
        for node in &containers {
            if r.reveal(node) {
                // ignore unsupported / spec-edge containers handled raw
                if matches!(node.data.borrow().value, NodeValue::Table(_) | NodeValue::TableRow(_))
                {
                    continue;
                }
                eprintln!("drag hit-test {p:?} -> selection {rng:?} revealed a container");
                return Err("a drag-derived selection revealed captured block syntax");
            }
        }
    }
    Ok(())
}

fn drag_never_reveals_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    drag_never_reveals_check_md(&md, width)
}

/// Property: every fragment's `rect` is text-tight, so `content_inset`
/// is always zero. Inline backgrounds don't expand the rect; their
/// vertical padding is drawn outside the rect at paint time (fitting
/// into `row_spacing`).
fn fragment_content_inset_matches_bg_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    let mut r = test_renderer(md);
    let violation = render_frame(&mut r, width, None, |r| {
        r.fragments
            .iter()
            .any(|f| {
                f.content_inset.left != 0.0
                    || f.content_inset.right != 0.0
                    || f.content_inset.top != 0.0
                    || f.content_inset.bottom != 0.0
            })
            .then_some("fragment has non-zero content_inset")
    });
    violation.map(Err).unwrap_or(Ok(()))
}

fn fragment_content_inset_matches_bg_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    fragment_content_inset_matches_bg_check_md(&md, width)
}

/// Property: `fragment_offset` is a left-inverse of `fragment_x`.
/// For every offset inside a fragment's source range,
/// `fragment_offset(fragment, fragment_x(fragment, offset)) ==
/// offset`. Catches regressions in cluster_advances math, the
/// `content_inset` shift, and linear-interp fallbacks.
fn cursor_x_roundtrip_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    let mut r = test_renderer(md);
    let violation = render_frame(&mut r, width, None, |r| {
        for f in r.fragments.clone().iter() {
            let total = f
                .source_range
                .end()
                .0
                .saturating_sub(f.source_range.start().0);
            // Pick a few offsets across the fragment's range:
            // start, end, and a few interior points.
            let probes: Vec<usize> = (0..=total).step_by((total / 4).max(1)).collect();
            for into in probes {
                let offset = f.source_range.start() + into;
                let x = r.fragment_x(f, offset);
                let back = r.fragment_offset(f, x);
                // `fragment_x` is not injective when zero-width
                // clusters (soft hyphens, ZWJ sequences) sit
                // between offsets — several adjacent offsets
                // share the same x. Accept any offset that
                // round-trips to the same x as a successful
                // round-trip.
                if back.0 != offset.0 {
                    let back_x = r.fragment_x(f, back);
                    if (back_x - x).abs() > 0.5 {
                        return Some("cursor x round-trip diverged");
                    }
                }
            }
        }
        None
    });
    violation.map(Err).unwrap_or(Ok(()))
}

fn cursor_x_roundtrip_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    cursor_x_roundtrip_check_md(&md, width)
}

/// Property: every painted glyph sits inside its source fragment's
/// content area (`fragment.rect` shrunk by `content_inset` on each
/// side), within a small tolerance. Localized version of
/// `glyph_in_render_area` — a failure here pinpoints which
/// fragment's walker/paint shape diverged instead of just "some
/// glyph strays off-screen." The 2px tolerance matches CLAUDE.md's
/// stated "walker and paint shape independently, by design" — sub-
/// pixel-to-few-px divergence at script edges / font fallback is
/// accepted; larger divergences indicate a walker bug.
fn glyph_in_fragment_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    const EPS: f32 = 0.5;
    let mut r = test_renderer(md);
    let violation = render_frame(&mut r, width, None, |r| {
        let ppi = r.ctx.pixels_per_point();
        // Match each `TextBufferArea` to the fragment whose glyph
        // origin matches the area's paint origin. For each glyph
        // in the area, check its painted x against the fragment's
        // content area (rect minus inset on both sides).
        for area in &r.text_areas {
            let area_origin_pt = Pos2::new(area.rect.left() / ppi, area.rect.top() / ppi);
            let frag = r.fragments.iter().find(|f| {
                let has_text = matches!(&f.content, FragmentContent::Glyphs { .. });
                if !has_text {
                    return false;
                }
                let glyph_origin =
                    f.rect.min + Vec2::new(f.content_inset.left, f.content_inset.top);
                (glyph_origin.x - area_origin_pt.x).abs() < 0.5
                    && (glyph_origin.y - area_origin_pt.y).abs() < 0.5
            });
            let Some(frag) = frag else { continue };
            let content_right = frag.rect.right() - frag.content_inset.right;
            let buf = area.buffer.read().unwrap();
            for run in buf.layout_runs() {
                for g in run.glyphs.iter() {
                    let gx_pt = (g.x + g.w) / ppi;
                    let glyph_right = area_origin_pt.x + gx_pt;
                    if glyph_right > content_right + EPS {
                        return Some("glyph paints past fragment's content area");
                    }
                }
            }
        }
        None
    });
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: rendering is deterministic. Rendering the same doc at
/// the same selection twice produces identical fragments (same rect,
/// same source_range, same content_inset). Catches non-deterministic
/// caching bugs and stale-cache-returns-old-data bugs where the
/// second render diverges from the first.
fn render_deterministic_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    fn snapshot(md: &str, width: f32) -> Vec<(Rect, (Grapheme, Grapheme))> {
        let mut r = test_renderer(md);
        render_frame(&mut r, width, None, |r| {
            r.fragments
                .iter()
                .map(|f| (f.rect, f.source_range))
                .collect()
        })
    }
    let a = snapshot(md, width);
    let b = snapshot(md, width);
    if a.len() != b.len() {
        return Err("fragment count differs between identical renders");
    }
    for (i, ((ar, asr), (br, bsr))) in a.iter().zip(b.iter()).enumerate() {
        if asr != bsr {
            let _ = i;
            return Err("fragment source range differs between identical renders");
        }
        if (ar.min.x - br.min.x).abs() > 0.001
            || (ar.min.y - br.min.y).abs() > 0.001
            || (ar.max.x - br.max.x).abs() > 0.001
            || (ar.max.y - br.max.y).abs() > 0.001
        {
            return Err("fragment rect differs between identical renders");
        }
    }
    Ok(())
}

fn render_deterministic_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    render_deterministic_check_md(&md, width)
}

/// Property: clicks in a backgrounded fragment's padding zone map
/// to the nearest content edge — not to a phantom "negative" glyph.
/// `fragment_offset(rect.left)` returns the source range's start;
/// `fragment_offset(rect.right)` returns its end. Catches inset-
/// math regressions where the padding zone returns midway offsets
/// or panics on negative-local-x.
fn click_in_bg_padding_zone_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    let mut r = test_renderer(md);
    let violation = render_frame(&mut r, width, None, |r| {
        let frags = r.fragments.clone();
        for f in &frags {
            let has_bg = f
                .style_stack
                .last()
                .is_some_and(|s| s.format.background != egui::Color32::TRANSPARENT);
            if !has_bg {
                continue;
            }
            if f.content_inset.left == 0.0 && f.content_inset.right == 0.0 {
                continue;
            }
            // Click on the very left edge of the rect — squarely
            // inside the leading bg padding (when present).
            if f.content_inset.left > 0.0 {
                let click_left = f.rect.min.x + 0.5;
                if r.fragment_offset(f, click_left) != f.source_range.0 {
                    return Some("click in left bg padding didn't map to source_range start");
                }
            }
            if f.content_inset.right > 0.0 {
                let click_right = f.rect.max.x - 0.5;
                if r.fragment_offset(f, click_right) != f.source_range.1 {
                    return Some("click in right bg padding didn't map to source_range end");
                }
            }
        }
        None
    });
    violation.map(Err).unwrap_or(Ok(()))
}

fn click_in_bg_padding_zone_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    click_in_bg_padding_zone_check_md(&md, width)
}

fn glyph_in_fragment_check(buf: &[u8], features: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, features);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    glyph_in_fragment_check_md(&md, width)
}

/// Regression test: when an inline backgrounded scope wraps within a
/// single wrap unit (a long `\`inline code\`` that needs multiple
/// visual rows at the column width), the bg fragments on adjacent
/// rows meet edge-to-edge vertically — no visible gap and no
/// overlap. Encodes the editor's `row_spacing = 2 × inline_padding`
/// design choice.
///
/// Restricted to a focused test rather than a generic property
/// because cross-wrap-unit fragment pairs (different paragraph
/// lines, different blocks) legitimately use different inter-line
/// spacing (`block_spacing`) — and the flat fragment list doesn't
/// carry per-wrap-unit identity, so a general "meet" check can't
/// distinguish the two cases.
pub(super) fn assert_bg_rows_meet_within_inline_code(
    md: &str, width: f32,
) -> Result<(), &'static str> {
    const EPS: f32 = 0.5;
    let mut r = test_renderer(md);
    let inline_pad = r.layout.inline_padding;
    // Fragment rect is text-tight; the *painted* bg extends `inline_pad`
    // up and down. Expand here so the seam check matches the user-visible
    // pill geometry.
    let mut bg_rects: Vec<(Rect, egui::Color32)> = render_frame(&mut r, width, None, |r| {
        r.fragments
            .iter()
            .filter_map(|f| {
                let bg = f.style_stack.last()?.format.background;
                if bg == egui::Color32::TRANSPARENT {
                    return None;
                }
                Some((f.rect.expand2(Vec2::new(0.0, inline_pad)), bg))
            })
            .collect()
    });
    // We expect ≥ 2 bg fragments (the inline code wraps to multiple
    // rows). Under no-coalescing each row may contain many bg
    // fragments side-by-side; group by row (y.min) and compare
    // distinct rows' y-bounds.
    bg_rects.sort_by(|a, b| a.0.min.y.partial_cmp(&b.0.min.y).unwrap());
    if bg_rects.len() < 2 {
        return Err("expected ≥ 2 bg fragments from wrapping inline code");
    }
    let mut row_bounds: Vec<(f32, f32, egui::Color32)> = Vec::new();
    for (r, c) in &bg_rects {
        match row_bounds.last_mut() {
            Some(last) if (r.min.y - last.0).abs() <= EPS && last.2 == *c => {
                last.1 = last.1.max(r.max.y);
            }
            _ => row_bounds.push((r.min.y, r.max.y, *c)),
        }
    }
    for win in row_bounds.windows(2) {
        let (_top_i, bot_i, ci) = win[0];
        let (top_j, _bot_j, cj) = win[1];
        if ci != cj {
            continue;
        }
        let gap = top_j - bot_i;
        if gap.abs() > EPS {
            return Err("wrapping inline code's bg fragments don't meet edge-to-edge");
        }
    }
    Ok(())
}

pub(super) fn galley_rect_in_render_area(md: &str, width: f32) -> Result<(), &'static str> {
    const EPS: f32 = 0.5;
    if render_galleys(md, width, None)
        .iter()
        .any(|g| g.rect.right() > width + EPS)
    {
        return Err("galley rect extends past render width");
    }
    Ok(())
}

/// Property: every actually-painted glyph lies within the render area.
/// Checks the glyph positions cosmic-text emits, offset by the
/// `TextBufferArea`'s screen `left/top`, against the doc's reported width
/// and height. Catches painting bugs that don't manifest in galley rects —
/// e.g. an RTL run whose buffer right-aligns its glyphs to the buffer
/// width, so glyphs paint at `pos.x + (W - V) .. pos.x + W` while the
/// galley rect only spans `pos.x .. pos.x + V`.
pub(super) fn glyph_in_render_area_check_md(md: &str, width: f32) -> Result<(), &'static str> {
    const EPS: f32 = 0.5;
    let mut r = test_renderer(md);
    let violation = render_frame(&mut r, width, None, |r| {
        let ppi = r.ctx.pixels_per_point();
        for area in &r.text_areas {
            let left_pt = area.rect.left() / ppi;
            let buf = area.buffer.read().unwrap();
            for run in buf.layout_runs() {
                for g in run.glyphs.iter() {
                    let gx_pt = (g.x + g.w) / ppi;
                    let g_left_pt = g.x / ppi;
                    let screen_right = left_pt + gx_pt;
                    let screen_left = left_pt + g_left_pt;
                    if screen_right > width + EPS || screen_left < -EPS {
                        return Some("painted glyph strays outside render width");
                    }
                }
            }
        }
        None
    });
    violation.map(Err).unwrap_or(Ok(()))
}

fn glyph_in_render_area_check(buf: &[u8], features: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, features);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    glyph_in_render_area_check_md(&md, width)
}

/// Property: rendering arbitrary UTF-8 text does not panic.
fn no_panic_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = fuzz_text(&mut src);
    let width = 100.0 + (src.draw(10) as f32) * 100.0;
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_height(&md, width)));
    match result {
        Ok(_) => Ok(()),
        Err(_) => Err("render panicked"),
    }
}

/// Property: rendering a structured GFM document does not panic. Same shape
/// as `no_panic_check` but driven by the `doc_gen` generator so the
/// renderer sees realistic GFM with complex-script text mixed in.
fn no_panic_structured_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_height(&md, width)));
    match result {
        Ok(_) => Ok(()),
        Err(_) => Err("render panicked on structured input"),
    }
}

/// Property: rendering at width W2 produces the same output regardless
/// of whether the renderer was previously used at a different width.
///
/// Generates a doc, picks two widths W1 ≠ W2. Renders at W2 with a
/// fresh renderer (`snap_fresh`). Renders the same doc at W2 with a
/// renderer that just rendered at W1 (`snap_after_w1`). If the layout
/// cache invalidates correctly on width change, both snapshots match.
///
/// Catches the class where cache entries keyed only by node range
/// (not width) leak across a width change — the symptom is "extra
/// space between blocks" or shifted galleys for one frame after a
/// window resize.
fn resize_idempotence_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, f);
    let w1 = 250.0 + (src.draw(8) as f32) * 100.0;
    let w2 = 250.0 + (src.draw(8) as f32) * 100.0;
    if (w1 - w2).abs() < 1.0 {
        return Ok(());
    }

    let snapshot = |r: &mut MdRender, width: f32| -> Vec<((Grapheme, Grapheme), f32)> {
        render_frame(r, width, None, |r| {
            r.fragments
                .iter()
                .map(|f| (f.source_range, f.rect.min.y))
                .collect()
        })
    };

    let mut r_fresh = test_renderer(&md);
    let snap_fresh = snapshot(&mut r_fresh, w2);

    let mut r_polluted = test_renderer(&md);
    let _ = snapshot(&mut r_polluted, w1);
    let snap_after_w1 = snapshot(&mut r_polluted, w2);

    if snap_fresh.len() != snap_after_w1.len() {
        return Err("galley count at W2 changed by prior W1 render");
    }
    for (a, b) in snap_fresh.iter().zip(snap_after_w1.iter()) {
        if a.0 != b.0 {
            return Err("galley source range at W2 changed by prior W1 render");
        }
        if (a.1 - b.1).abs() > 0.5 {
            return Err("galley y position at W2 changed by prior W1 render");
        }
    }
    Ok(())
}
