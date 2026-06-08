//! Frame-time bench for the link-heavy scratch test doc. Run with:
//!   cargo test -p workspace --lib --release link_perf -- --ignored --nocapture

use std::time::Instant;

use comrak::Arena;
use egui::{Pos2, RawInput, Rect, UiBuilder, Vec2};
use rand::{Rng, SeedableRng, rngs::StdRng};

use super::harness::{TestEditor, build_lb};
use super::render_props::test_renderer;
use crate::test_utils::byte_source::ByteSource;

const LINK_DOC: &str = "This paragraph exists to exercise every flavor of link the markdown editor knows how to render and route, starting with bare autolink URLs like https://example.com and https://news.ycombinator.com which should fetch titles asynchronously and swap in once the fetch returns, then inline links with text like [example](https://example.com) and empty-text inline links whose URL bytes swap for the fetched title like [](https://en.wikipedia.org/wiki/Markdown), a [mailto link](mailto:test@example.com) and an in-document [anchor link](#top) that should both resolve as External, wikilinks across every shape including bare like [[scratch-test]] and with a disambiguating path like [[imports/pasted_image_2026-05-11_18-38-14]] and with an explicit `.md` like [[scratch-test.md]] and case-insensitive like [[SCRATCH-TEST]], an absolute path link [absolute](/☀️%20tests/scratch-test.svg) and a relative one [sibling svg](scratch-test.svg) and an ascending [up and over](../facts/profiles/mom.md) and a percent-encoded [encoded path](/%E2%98%80%EF%B8%8F%20tests/scratch-test.svg) that should resolve identically to its raw form [raw path](/☀️ tests/scratch-test.svg), then the negative cases — an excessive-`..` [too far](../../../../foo.md) that must NOT silently saturate at root, a [folder path](/☀️%20tests/) that must NOT resolve since folders aren't documents, a [missing file](/never/exists.md), and a synthetic [bad uuid](lb://00000000-0000-0000-0000-000000000000) — all of which should render red/Broken with a tooltip on cmd-hover, plus a cross-tree case to exercise the yellow Warning state by linking from a pending-share tree (paste a real `lb://<uuid>` from a not-yet-accepted share here: [pending share](lb://REPLACE-WITH-PENDING-SHARE-UUID)), and finally inline images covering each path shape: a sibling image ![sibling png](black-standard-schnauzer.png), an absolute reference ![absolute](/☀️%20tests/black-standard-schnauzer.png), a nested import ![nested](imports/pasted_image_2026-05-11_18-38-14.png), an external URL ![remote](https://placehold.co/120x60), an empty-alt image whose alt text should swap for any fetched title ![](https://placehold.co/100x40), and a broken image ![missing](/nope/missing.png) which should render with the broken-image affordance.";

const PLAIN_DOC: &str = "This paragraph exists to exercise every flavor of text the markdown editor knows how to render and route, starting with bare URLs like example.com and news ycombinator com which would fetch titles asynchronously and swap in once the fetch returns, then inline notes with text like example example com and empty-text inline notes whose URL bytes swap for the fetched title like en wikipedia org wiki Markdown, a mailto note test example com and an in-document anchor note top that should both resolve as External, wikinotes across every shape including bare like scratch-test and with a disambiguating path like imports pasted_image_2026-05-11_18-38-14 and with an explicit md like scratch-test md and case-insensitive like SCRATCH-TEST, an absolute path note tests scratch-test svg and a relative one scratch-test svg and an ascending facts profiles mom md and a percent-encoded path tests scratch-test svg that should resolve identically to its raw form tests scratch-test svg, then the negative cases — an excessive dot dot path foo md that must NOT silently saturate at root, a folder path tests that must NOT resolve since folders aren't documents, a missing file never exists md, and a synthetic bad uuid 00000000-0000-0000-0000-000000000000 — all of which should render red Broken with a tooltip on cmd-hover, plus a cross-tree case to exercise the yellow Warning state by noting from a pending-share tree paste a real uuid from a not-yet-accepted share here REPLACE-WITH-PENDING-SHARE-UUID, and finally inline images covering each path shape: a sibling image black-standard-schnauzer png, an absolute reference tests black-standard-schnauzer png, a nested import imports pasted_image_2026-05-11_18-38-14 png, an external URL placehold co 120x60, an empty-alt image whose alt text should swap for any fetched title placehold co 100x40, and a broken image nope missing png which should render with the broken-image affordance.";

fn bench(label: &str, md: &str, frames: usize) {
    let lb = build_lb();
    let mut h = TestEditor::with_lb(lb, md);
    for _ in 0..5 {
        h.enter_frame();
    }

    let start = Instant::now();
    for _ in 0..frames {
        h.enter_frame();
    }
    let elapsed = start.elapsed();

    let per = elapsed / frames as u32;
    let fps = 1.0 / per.as_secs_f64();
    println!(
        "{label:<14} {frames} frames  total {elapsed:>8.2?}  per-frame {per:>8.2?}  ({fps:>5.0} fps)"
    );
}

const TINY_DOC: &str = "hello world";

const LONG_DOC_PREFIX: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ";

/// Tight loop on the link-heavy doc only, for `samply` / `cargo flamegraph`.
/// Build + profile with:
///   cargo test --profile profiling -p workspace --lib --no-run link_profile
///   samply record <target/profiling/deps/workspace_rs-XXXX> link_profile --ignored --nocapture
#[test]
#[ignore]
fn link_profile() {
    let lb = build_lb();
    let mut h = TestEditor::with_lb(lb, LINK_DOC);
    for _ in 0..10 {
        h.enter_frame();
    }
    for _ in 0..2000 {
        h.enter_frame();
    }
}

#[test]
#[ignore]
fn link_perf() {
    const FRAMES: usize = 200;
    let long_doc = LONG_DOC_PREFIX.repeat(50);
    println!(
        "doc lengths: tiny={}  plain={}  link={}  long={} bytes",
        TINY_DOC.len(),
        PLAIN_DOC.len(),
        LINK_DOC.len(),
        long_doc.len()
    );
    // run twice to check variance
    for _ in 0..2 {
        bench("tiny", TINY_DOC, FRAMES);
        bench("plain-prose", PLAIN_DOC, FRAMES);
        bench("link-heavy", LINK_DOC, FRAMES);
        bench("long-prose", &long_doc, FRAMES);
    }
}

// ── renderer micro / integration benches ──

/// Micro-benchmark: cold-shape N text spans through `upsert_glyphon_buffer`,
/// busting the cache by varying width per iteration. Lets us measure the
/// per-shape cost in isolation from the rest of the renderer.
#[test]
#[ignore]
fn bench_shape() {
    use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};
    use std::time::Instant;
    let r = test_renderer("");
    let format = Format {
        family: FontFamily::Sans,
        bold: false,
        italic: false,
        color: egui::Color32::WHITE,
        underline: false,
        strikethrough: false,
        background: egui::Color32::TRANSPARENT,
        border: egui::Color32::TRANSPARENT,
        spoiler: false,
        superscript: false,
        subscript: false,
    };
    let inputs: &[(&str, &str)] = &[
        ("ascii_short", "foo bar baz"),
        ("ascii_long", "the quick brown fox jumps over the lazy dog forty seven times in a row"),
        ("with_emoji", "hello world 👋 this is a sample 🎉 paragraph"),
        ("arabic", "كيف حال شكرا مرحبا"),
        ("mixed_scripts", "hello नमस्ते 👋 كيف foo bar"),
    ];
    let n = 2000;
    // warm-up — first call pays font fallback init etc.
    for (_, text) in inputs {
        let _ = r.upsert_glyphon_buffer_unwrapped(text, 16.0, 16.0, 100.0, &format);
    }
    for (label, text) in inputs {
        let start = Instant::now();
        for i in 0..n {
            let width = 200.0 + (i as f32);
            let _ = r.upsert_glyphon_buffer_unwrapped(text, 16.0, 16.0, width, &format);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "{:<15} n={} total={:.3}s  per-shape={:.1}μs",
            label,
            n,
            elapsed.as_secs_f64(),
            elapsed.as_micros() as f64 / n as f64
        );
    }
}

/// Cold-start render of specific document shapes via raw `show_block`,
/// printing per-shape timing. Baseline for perf before/after comparisons.
#[test]
#[ignore]
fn bench_render_warm() {
    use std::time::Instant;

    let width = 800.0;
    let n_iters = 10;

    fn time_warm_render(md: &str, width: f32) -> f64 {
        let mut r = test_renderer(md);
        r.set_width(width);
        let ctx = r.ctx.clone();
        // First render: cold, populates caches. Untimed.
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let arena = comrak::Arena::new();
                let root = r.reparse(&arena);
                let height = r.height(root);
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    r.show_block(ui, root, Pos2::ZERO);
                });
            });
        });
        // Second render: warm (no input changes, all caches hit).
        let start = Instant::now();
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let arena = comrak::Arena::new();
                let root = r.reparse(&arena);
                let height = r.height(root);
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    r.show_block(ui, root, Pos2::ZERO);
                });
            });
        });
        start.elapsed().as_secs_f64() * 1000.0
    }

    let bench = |label: &str, md: &str| {
        let mut samples = Vec::new();
        for _ in 0..n_iters {
            samples.push(time_warm_render(md, width));
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        eprintln!(
            "  {:<40}  bytes={:>7}  mean={:>7.2}ms  range=[{:>6.2}, {:>6.2}]",
            label,
            md.len(),
            mean,
            min,
            max
        );
    };

    let para = "The quick brown fox jumps over the lazy dog. ".repeat(4);
    let many_short_paras_10k = (0..50).map(|_| format!("{para}\n\n")).collect::<String>();
    let many_short_paras_100k = (0..500).map(|_| format!("{para}\n\n")).collect::<String>();
    bench("50 short paragraphs", &many_short_paras_10k);
    bench("500 short paragraphs", &many_short_paras_100k);

    let long_para_100k = "The quick brown fox jumps over the lazy dog. ".repeat(2250);
    bench("one 100k paragraph", &long_para_100k);

    let body = "the_quick_brown_fox_jumps\n".repeat(3800);
    let long_code_100k = format!("```\n{body}```\n");
    bench("one 100k fenced code block", &long_code_100k);

    let bq_100k = (0..500)
        .map(|_| format!("> {para}\n>\n"))
        .collect::<String>();
    bench("blockquote, 500 inner paragraphs", &bq_100k);

    let mut rng = StdRng::seed_from_u64(0);
    let mut buf = vec![0u8; 32_768];
    rng.fill(&mut buf[..]);
    let mut src = ByteSource::new(&buf);
    let generated_100k = super::doc_gen::gen_doc_large(&mut src, 100_000);
    bench("gen_doc_large seed=0 ~100k", &generated_100k);
}

#[test]
#[ignore]
fn bench_render_cold() {
    use std::time::Instant;

    let width = 800.0;
    let n_iters = 5;

    fn time_render(md: &str, width: f32) -> f64 {
        let mut r = test_renderer(md);
        r.set_width(width);
        let ctx = r.ctx.clone();
        let start = Instant::now();
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let arena = comrak::Arena::new();
                let root = r.reparse(&arena);
                let height = r.height(root);
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    r.show_block(ui, root, Pos2::ZERO);
                });
            });
        });
        start.elapsed().as_secs_f64() * 1000.0
    }

    let bench = |label: &str, md: &str| {
        let mut samples = Vec::new();
        for _ in 0..n_iters {
            samples.push(time_render(md, width));
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        eprintln!(
            "  {:<40}  bytes={:>7}  mean={:>7.1}ms  range=[{:>6.1}, {:>6.1}]",
            label,
            md.len(),
            mean,
            min,
            max
        );
    };

    let para = "The quick brown fox jumps over the lazy dog. ".repeat(4);
    let many_short_paras_10k = (0..50).map(|_| format!("{para}\n\n")).collect::<String>();
    let many_short_paras_100k = (0..500).map(|_| format!("{para}\n\n")).collect::<String>();
    bench("50 short paragraphs", &many_short_paras_10k);
    bench("500 short paragraphs", &many_short_paras_100k);

    let long_para_100k = "The quick brown fox jumps over the lazy dog. ".repeat(2250);
    bench("one 100k paragraph", &long_para_100k);

    let body = "the_quick_brown_fox_jumps\n".repeat(3800);
    let long_code_100k = format!("```\n{body}```\n");
    bench("one 100k fenced code block", &long_code_100k);

    let bq_100k = (0..500)
        .map(|_| format!("> {para}\n>\n"))
        .collect::<String>();
    bench("blockquote, 500 inner paragraphs", &bq_100k);

    let mut rng = StdRng::seed_from_u64(0);
    let mut buf = vec![0u8; 32_768];
    rng.fill(&mut buf[..]);
    let mut src = ByteSource::new(&buf);
    let generated_100k = super::doc_gen::gen_doc_large(&mut src, 100_000);
    bench("gen_doc_large seed=0 ~100k", &generated_100k);
}

/// Cold-open the editor at an arbitrary offset and render — the
/// search-preview hot path. We restore a persisted anchor (a top-level
/// block index), drive the affine scroll area through one full frame
/// (set_offset + visible + paint), and time the whole thing.
///
/// Three offsets — top, mid, end — sweep the spectrum of "how much
/// content sits below the anchor". With a viewport-bounded scroll
/// area this should be roughly flat across all three; if a code path
/// walks `signed_distance(offset, end)` (as the current `clamp_to_max`
/// does) the top-offset case will be O(doc_size).
/// Same as [`bench_scroll_cold_at_offsets`] but reuses the renderer
/// across iterations after a single cold open — measures repeated
/// scroll/render at a stable offset with hot caches.
#[test]
#[ignore]
fn bench_scroll_warm_at_offsets() {
    use crate::tab::markdown_editor::scroll_content::{DocRowId, DocScrollContent, paint_row};
    use crate::widgets::affine_scroll::{AffineScrollArea, Offset};
    use std::time::Instant;

    let width = 800.0;
    let viewport_height = 600.0;
    let n_iters = 10;

    fn run_one(
        r: &mut crate::tab::markdown_editor::MdRender, scroll: &mut AffineScrollArea<DocRowId>,
        width: f32, vh: f32, block_idx: usize,
    ) {
        let ctx = r.ctx.clone();
        let canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, vh));
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let arena = Arena::new();
                let root = r.reparse(&arena);
                ui.scope_builder(UiBuilder::new().max_rect(canvas_rect), |ui| {
                    ui.set_clip_rect(canvas_rect);
                    let visible = {
                        let content = DocScrollContent::new(r, root, vh / 2.0);
                        let off = Offset::new(DocRowId::Block(block_idx), 0.0);
                        scroll.set_offset(&content, off);
                        scroll.show(ui, &content).visible
                    };
                    let blocks: Vec<_> = root.children().collect();
                    for vrow in &visible {
                        let top_left = Pos2::new(canvas_rect.min.x, canvas_rect.min.y + vrow.top);
                        paint_row(ui, r, root, &blocks, &vrow.id, top_left, 0.0);
                    }
                });
            });
        });
    }

    let bench = |label: &str, md: &str, n_blocks: usize| {
        for (loc_label, idx) in [
            ("top  ", 0usize),
            ("mid  ", n_blocks.saturating_sub(1) / 2),
            ("end  ", n_blocks.saturating_sub(1)),
        ] {
            let mut r = test_renderer(md);
            r.set_width(width);
            let mut scroll = AffineScrollArea::<DocRowId>::new("bench_scroll_warm");
            // Untimed cold pass to populate caches.
            run_one(&mut r, &mut scroll, width, viewport_height, idx);

            let mut samples = Vec::new();
            for _ in 0..n_iters {
                let start = Instant::now();
                run_one(&mut r, &mut scroll, width, viewport_height, idx);
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            eprintln!(
                "  {:<35} {} block_idx={:>4}/{:<4} mean={:>7.2}ms  range=[{:>6.2}, {:>6.2}]",
                label, loc_label, idx, n_blocks, mean, min, max,
            );
        }
    };

    let count_blocks = |md: &str| -> usize {
        let mut r = test_renderer(md);
        let arena = Arena::new();
        r.reparse(&arena).children().count()
    };

    let para = "The quick brown fox jumps over the lazy dog. ".repeat(4);
    let many_short_paras = (0..500).map(|_| format!("{para}\n\n")).collect::<String>();
    bench("500 short paragraphs", &many_short_paras, count_blocks(&many_short_paras));

    let bq_500 = (0..500)
        .map(|_| format!("> {para}\n>\n"))
        .collect::<String>();
    bench("blockquote, 500 inner paras", &bq_500, count_blocks(&bq_500));

    let mut rng = StdRng::seed_from_u64(0);
    let mut buf = vec![0u8; 32_768];
    rng.fill(&mut buf[..]);
    let mut src = ByteSource::new(&buf);
    let generated_100k = super::doc_gen::gen_doc_large(&mut src, 100_000);
    bench("gen_doc_large seed=0 ~100k", &generated_100k, count_blocks(&generated_100k));
}

#[test]
#[ignore]
fn bench_scroll_cold_at_offsets() {
    use crate::tab::markdown_editor::scroll_content::{DocRowId, DocScrollContent, paint_row};
    use crate::widgets::affine_scroll::{AffineScrollArea, Offset};
    use std::time::Instant;

    let width = 800.0;
    let viewport_height = 600.0;
    let n_iters = 5;

    fn time_cold_open(md: &str, width: f32, vh: f32, block_idx: usize) -> f64 {
        let mut r = test_renderer(md);
        r.set_width(width);
        let ctx = r.ctx.clone();
        let mut scroll = AffineScrollArea::<DocRowId>::new("bench_scroll");
        let canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, vh));

        let start = Instant::now();
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let arena = Arena::new();
                let root = r.reparse(&arena);
                ui.scope_builder(UiBuilder::new().max_rect(canvas_rect), |ui| {
                    ui.set_clip_rect(canvas_rect);

                    // Phase 1: scroll math + restore persisted offset
                    // with an immutable renderer borrow.
                    let visible = {
                        let content = DocScrollContent::new(&r, root, vh / 2.0);
                        let off = Offset::new(DocRowId::Block(block_idx), 0.0);
                        scroll.set_offset(&content, off);
                        scroll.show(ui, &content).visible
                    };

                    // Phase 2: paint visible rows with a mutable
                    // renderer borrow.
                    let blocks: Vec<_> = root.children().collect();
                    for vrow in &visible {
                        let top_left = Pos2::new(canvas_rect.min.x, canvas_rect.min.y + vrow.top);
                        paint_row(ui, &mut r, root, &blocks, &vrow.id, top_left, 0.0);
                    }
                });
            });
        });
        start.elapsed().as_secs_f64() * 1000.0
    }

    let bench = |label: &str, md: &str, n_blocks: usize| {
        for (loc_label, idx) in [
            ("top  ", 0usize),
            ("mid  ", n_blocks.saturating_sub(1) / 2),
            ("end  ", n_blocks.saturating_sub(1)),
        ] {
            let mut samples = Vec::new();
            for _ in 0..n_iters {
                samples.push(time_cold_open(md, width, viewport_height, idx));
            }
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            eprintln!(
                "  {:<35} {} block_idx={:>4}/{:<4} mean={:>7.1}ms  range=[{:>6.1}, {:>6.1}]",
                label, loc_label, idx, n_blocks, mean, min, max,
            );
        }
    };

    let count_blocks = |md: &str| -> usize {
        let mut r = test_renderer(md);
        let arena = Arena::new();
        r.reparse(&arena).children().count()
    };

    let para = "The quick brown fox jumps over the lazy dog. ".repeat(4);
    let many_short_paras = (0..500).map(|_| format!("{para}\n\n")).collect::<String>();
    bench("500 short paragraphs", &many_short_paras, count_blocks(&many_short_paras));

    let bq_500 = (0..500)
        .map(|_| format!("> {para}\n>\n"))
        .collect::<String>();
    bench("blockquote, 500 inner paras", &bq_500, count_blocks(&bq_500));

    let mut rng = StdRng::seed_from_u64(0);
    let mut buf = vec![0u8; 32_768];
    rng.fill(&mut buf[..]);
    let mut src = ByteSource::new(&buf);
    let generated_100k = super::doc_gen::gen_doc_large(&mut src, 100_000);
    bench("gen_doc_large seed=0 ~100k", &generated_100k, count_blocks(&generated_100k));
}
