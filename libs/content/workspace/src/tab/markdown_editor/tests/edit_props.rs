//! Property tests driving `MdEdit` / `Editor` through the full
//! event → operation → buffer → reparse → render pipeline via
//! [`TestEditor`].

use super::super::input::{Advance, Bound, Event, Location, Region};
use super::doc_gen::{self, BlockFeatures, Features, ScriptFeatures, WhitespaceFeatures, gen_doc};
use super::harness::TestEditor;
use crate::test_utils::byte_source::ByteSource;
use lb_rs::model::text::offset_types::Grapheme;

// ── runners (one per corpus) ──
//
// Each pairs `crate::test_utils::prop::run`'s seed loop with the corpus a
// property draws from. `run` takes the `Features` corpus as a visible
// argument; the others name their corpus in the runner. The corpus also
// drives the failure reproducer, so the printed doc matches what failed.

/// Renders the shrunken failing buffer as `doc` plus the trailing event
/// stream — the repro for the editor-driving properties.
fn doc_events_repro(b: &[u8], doc: impl Fn(&mut ByteSource) -> String) -> String {
    let mut src = ByteSource::new(b);
    let initial = doc(&mut src);
    let n = event_count(&mut src);
    format!("initial buffer:\n{initial:?}\n~{n} events drawn from the rest of the byte stream")
}

/// `gen_doc(features)` corpus — `features` is the visible knob.
fn run<C, E>(check: C, features: Features, seeds: u64)
where
    C: Fn(&[u8], &Features) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(
        seeds,
        64,
        |b| check(b, &features),
        |b| doc_events_repro(b, |s| gen_doc(s, &features)),
    );
}

/// Full-feature `gen_doc(all())` corpus (corpus fixed inside the check).
fn run_all<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 64, check, |b| {
        doc_events_repro(b, |s| gen_doc(s, &Features::all()))
    });
}

/// Plain-ASCII `gen_doc(default())` corpus — the "general case".
fn run_simple<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 64, check, |b| {
        doc_events_repro(b, |s| gen_doc(s, &Features::default()))
    });
}

/// Nested-list structural corpus ([`gen_nested_list_doc`]).
fn run_lists<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 64, check, |b| {
        format!("doc:\n{:?}", gen_nested_list_doc(&mut ByteSource::new(b)).0)
    });
}

/// Nested-blockquote structural corpus ([`gen_nested_bq_doc`]).
fn run_quotes<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 64, check, |b| {
        format!("doc:\n{:?}", gen_nested_bq_doc(&mut ByteSource::new(b)).0)
    });
}

/// Corpus is chosen inside the check (varies per seed), so the repro is the
/// shrunken bytes — decode them with the check's own generator.
fn run_raw<C, E>(check: C, seeds: u64)
where
    C: Fn(&[u8]) -> Result<(), E>,
    E: std::fmt::Display,
{
    crate::test_utils::prop::run(seeds, 64, check, |_| {
        "corpus is internal to the check; decode the shrunken bytes with its generator".to_string()
    });
}

// ════════════════════════ property audit table ════════════════════════
// property → corpus → seeds. `run` takes the Features corpus explicitly;
// run_all / run_simple / run_lists / run_quotes / run_raw name the corpus
// they draw from. The per-offset sweeps cost O(doc_len) per seed, so the
// 120s budget may stop the slowest before all 100 seeds run. Bodies are
// under the implementations banner below.

// no panic
#[test]
fn no_panic_under_edits() {
    run_all(no_panic_under_edits_check, 1000);
}
#[test]
fn no_panic_structured_init() {
    run_all(no_panic_structured_init_check, 1000);
}
// lb-rs delta-math panic at seed 302 caps this; raise when that's fixed.
#[test]
fn no_panic_typing_triple_backtick() {
    run_all(no_panic_typing_triple_backtick_check, 302);
}

// cursor coverage & motion (per-offset sweeps; cursor_in_bounds excepted)
#[test]
fn cursor_in_bounds() {
    run_all(cursor_in_bounds_check, 1000);
}
#[test]
fn cursor_renders() {
    let all = Features::all();
    let f = Features {
        scripts: ScriptFeatures::default(),
        whitespace: WhitespaceFeatures { long_token: false, id_token: false, ..all.whitespace },
        blocks: BlockFeatures { table: false, ..all.blocks },
        ..all
    };
    run(cursor_renders_check, f, 100);
}
#[test]
fn cursor_renders_simple() {
    run(cursor_renders_check, Features::default(), 100);
}
// Only the plain-ASCII corpus: with syntax markers present (tier_b),
// override-collapsed markers (`*`, `_`, `**`, …) render zero-width when the
// cursor isn't on them, so adjacent offsets legitimately share an x and the
// "arrow always moves visually" invariant is false by design. A `tier_b`
// variant therefore can't hold; this ASCII version is the sound one.
#[test]
fn cursor_visibly_moves_simple() {
    run(cursor_visibly_moves_check, Features::default(), 100);
}
#[test]
fn cursor_visibly_moves_vertical_pure_ascii() {
    run_simple(cursor_visibly_moves_vertical_check_simple, 100);
}
#[test]
fn cursor_vertical_round_trip_pure_ascii() {
    run_simple(cursor_vertical_round_trip_check, 100);
}
#[test]
fn arrow_advance_one_grapheme() {
    run_simple(arrow_advance_one_grapheme_check, 100);
}
#[test]
fn cmd_line_jump_preserves_y() {
    run_simple(cmd_line_jump_preserves_y_check, 100);
}

// layout cache & scroll stability
#[test]
fn render_invariants_survive_edits() {
    run(render_invariants_survive_edits_check, Features::tier_b(), 1000);
}
#[test]
fn render_stable_after_event() {
    run_all(render_stable_after_event_check, 1000);
}
#[test]
fn fold_toggle_does_not_jump_scroll() {
    run_all(fold_toggle_does_not_jump_scroll_check, 1000);
}
#[test]
fn layout_cache_consistent() {
    run_all(layout_cache_consistent_check, 1000);
} // + embed resolver
#[test]
fn image_load_consistent() {
    run_all(image_load_consistent_check, 1000);
} // + image cache

// indent / structure
#[test]
fn indent_only_moves_selected_line() {
    run(indent_only_moves_selected_line_check, Features::nested_lists(), 1000);
}
#[test]
fn indent_preserves_marker() {
    run(indent_preserves_marker_check, Features::nested_lists(), 1000);
}
#[test]
fn indent_doesnt_break_document() {
    run_raw(indent_doesnt_break_document_check, 1000);
}
#[test]
fn indent_is_permissive() {
    run_raw(indent_is_permissive_check, 1000);
}
#[test]
fn shift_tab_strips_one_level() {
    run_lists(shift_tab_strips_one_level_check, 1000);
}
#[test]
fn shift_tab_preserves_item_lines() {
    run_lists(shift_tab_preserves_item_lines_check, 1000);
}
#[test]
fn shift_tab_strips_one_bq() {
    run_quotes(shift_tab_strips_one_bq_check, 1000);
}

// ═══════════════════════════ implementations ═══════════════════════════

/// Picks an offset in `[0, doc_len_chars]` (end inclusive so generators
/// can target the cursor-after-EOF boundary). Two-byte draw so long
/// docs aren't capped at 256.
fn offset(src: &mut ByteSource, doc_len_chars: usize) -> Grapheme {
    if doc_len_chars == 0 {
        return Grapheme(0);
    }
    let hi = src.draw(256);
    let lo = src.draw(256);
    Grapheme((hi * 256 + lo) % (doc_len_chars + 1))
}

fn region(src: &mut ByteSource, doc_len_chars: usize) -> Region {
    let a = offset(src, doc_len_chars);
    let b = offset(src, doc_len_chars);
    let (start, end) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    Region::BetweenLocations { start: Location::Grapheme(start), end: Location::Grapheme(end) }
}

/// Replacement text biased toward common cases first, exotic cases
/// (multi-line, markdown markers, complex-script codepoints) at the
/// tail.
/// Same gating rules as `typed_char_with`: syntax-rich and complex-
/// script candidates are only emitted when the matching `Features`
/// flag is on, with ASCII fallbacks otherwise.
fn replacement_text_with(src: &mut ByteSource, features: &Features) -> String {
    let s = &features.scripts;
    let i = &features.inlines;
    let b = &features.blocks;
    let any_complex_script = s.hindi || s.arabic || s.cjk || s.emoji || s.arbitrary;
    let strong_ok = i.strong;
    let list_ok = b.bullet_list;
    let heading_ok = b.atx_heading;
    match src.bias(&[5, 4, 3, 2, 2, 1, 1, 1, 3, 8]) {
        0 => "x".to_string(),
        1 => String::new(),
        2 => "foo".to_string(),
        3 => " ".to_string(),
        4 => "\n".to_string(),
        5 if strong_ok => "**bold**".to_string(),
        6 if list_ok => "- list\n- item\n".to_string(),
        7 if heading_ok => "# heading\n\nparagraph\n".to_string(),
        8 => doc_gen::gen_text_f(src, features),
        9 if any_complex_script => doc_gen::gen_complex_codepoint(src),
        _ => "x".to_string(),
    }
}

/// Picks one event biased toward what an interactive user generates:
/// mostly single-char inserts and ±1 cursor moves; bulk edits, big
/// jumps, and undo/redo are rare.
fn event(src: &mut ByteSource, cursor: Grapheme, doc_len_chars: usize) -> Event {
    event_with(src, cursor, doc_len_chars, &Features::all())
}

/// Feature-aware variant of `event`. Forwards `features` to
/// `typed_char_with` so typed characters respect the same generator
/// flags as the initial doc. Other event kinds (deletion, selection,
/// navigation, indent, undo/redo) are pure mutations that don't
/// introduce new content, so they pass through unchanged.
fn event_with(
    src: &mut ByteSource, cursor: Grapheme, doc_len_chars: usize, features: &Features,
) -> Event {
    let weights = &[
        25, // type a character (insert at cursor)
        12, // backspace (delete one grapheme before cursor)
        8,  // forward delete (delete one grapheme at cursor)
        15, // arrow move (collapse cursor ± 1)
        6,  // shift+arrow (extend selection ± 1)
        5,  // newline (Enter)
        3,  // tab / shift+tab (Indent)
        4,  // click somewhere (jump cursor to a random offset)
        2,  // big-range edit (the old broad-region mutation, kept for coverage)
        2,  // big-range delete
        2,  // big-range select
        2,  // undo
        2,  // redo
    ];
    let cursor_idx = cursor.0;
    let max = doc_len_chars;
    match src.bias(weights) {
        // type a character — insert at cursor (which may be a range; Replace
        // handles that as a substitution)
        0 => Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(cursor),
                end: Location::Grapheme(cursor),
            },
            text: typed_char_with(src, features),
            advance_cursor: true,
        },
        // backspace — delete one grapheme before the cursor (or no-op at start)
        1 => {
            let from = if cursor_idx == 0 { 0 } else { cursor_idx - 1 };
            Event::Delete {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(Grapheme(from)),
                    end: Location::Grapheme(cursor),
                },
            }
        }
        // forward delete (Del key) — delete one grapheme at the cursor
        2 => {
            let to = (cursor_idx + 1).min(max);
            Event::Delete {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(cursor),
                    end: Location::Grapheme(Grapheme(to)),
                },
            }
        }
        // arrow key — collapse selection to cursor ± 1
        3 => {
            let target = if src.bias(&[1, 1]) == 0 {
                cursor_idx.saturating_sub(1)
            } else {
                (cursor_idx + 1).min(max)
            };
            Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(Grapheme(target)),
                    end: Location::Grapheme(Grapheme(target)),
                },
            }
        }
        // shift+arrow — extend selection by 1
        4 => {
            let other = if src.bias(&[1, 1]) == 0 {
                cursor_idx.saturating_sub(1)
            } else {
                (cursor_idx + 1).min(max)
            };
            let (start, end) =
                if cursor_idx <= other { (cursor_idx, other) } else { (other, cursor_idx) };
            Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(Grapheme(start)),
                    end: Location::Grapheme(Grapheme(end)),
                },
            }
        }
        5 => Event::Newline { shift: src.bias(&[3, 1]) == 1 },
        6 => Event::Indent { deindent: src.bias(&[3, 1]) == 1 },
        // click somewhere — jump cursor to a random offset
        7 => {
            let target = offset(src, doc_len_chars);
            Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(target),
                    end: Location::Grapheme(target),
                },
            }
        }
        // big-range Replace — keeps the old broad-region edit in the mix
        8 => Event::Replace {
            region: region(src, doc_len_chars),
            text: replacement_text_with(src, features),
            advance_cursor: true,
        },
        9 => Event::Delete { region: region(src, doc_len_chars) },
        10 => Event::Select { region: region(src, doc_len_chars) },
        11 => Event::Undo,
        _ => Event::Redo,
    }
}

/// One "typed" character. Mostly ASCII; rare picks include complex-
/// script codepoints, tab/newline (possible from paste), and
/// markdown syntax characters (`` ` ``, `*`, `_`, `>`, `[`) so the
/// editor exercises the partial-syntax states a real user goes
/// through one keystroke at a time. Each draw category is gated by
/// the matching `Features` flag(s); if a category is disabled by
/// the config we fall back to a basic ASCII char so edit-event
/// coverage stays aligned with whatever tier the test declares.
fn typed_char_with(src: &mut ByteSource, features: &Features) -> String {
    let s = &features.scripts;
    let i = &features.inlines;
    let w = &features.whitespace;
    let any_complex_script = s.hindi || s.arabic || s.cjk || s.emoji || s.arbitrary;
    let any_syntax_inline =
        i.emph || i.strong || i.underline || i.code || i.link || i.autolink || i.wikilink;
    let block_quote = features.blocks.block_quote;
    match src.bias(&[20, 5, 3, 2, 1, 1, 3]) {
        0 => "x".to_string(),
        1 => " ".to_string(),
        2 => ",".to_string(),
        3 if any_complex_script => doc_gen::gen_complex_codepoint(src),
        4 if w.tab => "\t".to_string(),
        5 => doc_gen::gen_word_f(src, features),
        6 if any_syntax_inline || block_quote => {
            // Available syntax chars depend on which inline/block
            // features are on. Build a weighted choice over the
            // currently-enabled subset.
            let mut weights: Vec<u32> = Vec::new();
            let mut choices: Vec<&'static str> = Vec::new();
            if i.code {
                weights.push(3);
                choices.push("`");
            }
            if i.emph || i.strong {
                weights.push(1);
                choices.push("*");
            }
            if i.emph || i.strong || i.underline {
                weights.push(1);
                choices.push("_");
            }
            if block_quote {
                weights.push(1);
                choices.push(">");
            }
            if i.link || i.autolink || i.wikilink {
                weights.push(1);
                choices.push("[");
            }
            choices[src.bias(&weights)].to_string()
        }
        _ => "x".to_string(),
    }
}

/// Number of events to generate (1–8). Each event is built lazily
/// against the current buffer state, since the editor's contract is
/// that offsets are valid at the event's frame start.
fn event_count(src: &mut ByteSource) -> usize {
    1 + src.bias(&[3, 4, 4, 3, 2, 2, 1, 1])
}

/// Drives `f` once per generated event.
fn drive<F>(buf: &[u8], mut f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &mut ByteSource),
{
    let mut src = ByteSource::new(buf);
    let initial = initial_buffer(&mut src);
    let n = event_count(&mut src);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ws = TestEditor::new(&initial);
        for _ in 0..n {
            f(&mut ws, &mut src);
        }
    }));
    result.map_err(|_| ())
}

/// `drive` variant generating with a specific [`Features`] config. Lets
/// tests state exactly which generator axes their invariant covers.
/// Applies the random 1–8 edit sequence.
fn drive_features<F>(buf: &[u8], features: &Features, f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &mut ByteSource),
{
    drive_features_inner(buf, features, None, f)
}

/// Like [`drive_features`] but applies exactly `events` edits instead of the
/// random 1–8. For invariants over a single settled layout (navigation,
/// idempotence) where coverage comes from the generated doc, not the edit
/// history: one edited state is enough, and the per-offset sweeps these
/// tests run would otherwise re-render once per edit for no extra coverage.
fn drive_features_n<F>(buf: &[u8], features: &Features, events: usize, f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &mut ByteSource),
{
    drive_features_inner(buf, features, Some(events), f)
}

/// [`drive_features_n`] over the plain-ASCII corpus (`Features::default()`) —
/// the "general case" with no markdown syntax or complex scripts.
fn drive_simple_n<F>(buf: &[u8], events: usize, f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &mut ByteSource),
{
    drive_features_n(buf, &Features::default(), events, f)
}

fn drive_features_inner<F>(
    buf: &[u8], features: &Features, events: Option<usize>, mut f: F,
) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &mut ByteSource),
{
    let mut src = ByteSource::new(buf);
    let initial = gen_doc(&mut src, features);
    // Always draw the random count so a seed maps to the same doc + edit
    // bytes regardless of any cap; a fixed `events` only changes how many
    // of those edits we actually apply.
    let random_n = event_count(&mut src);
    let n = events.unwrap_or(random_n);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ws = TestEditor::new(&initial);
        for _ in 0..n {
            f(&mut ws, &mut src);
        }
    }));
    result.map_err(|_| ())
}

/// `drive` variant that wires a [`TestEmbeds`] embed resolver and gives
/// `f` a handle for completing fake loads — exercises `embeds_seq`'s
/// role in `HeightDeps`.
fn drive_with_embeds<F>(buf: &[u8], mut f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &std::sync::Arc<super::harness::TestEmbeds>, &mut ByteSource),
{
    let mut src = ByteSource::new(buf);
    let initial = initial_buffer(&mut src);
    let n = event_count(&mut src);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let lb = super::harness::build_lb();
        let (mut ws, embeds) = TestEditor::with_test_embeds(lb, &initial);
        for _ in 0..n {
            f(&mut ws, &embeds, &mut src);
        }
    }));
    result.map_err(|_| ())
}

/// `drive` variant that wires the production [`ImageEmbedResolver`] +
/// [`ImageCache`] so callers can mimic worker-thread completions via
/// `image_cache.complete_load(...)`. Unlike `drive_with_embeds`'s
/// [`TestEmbeds`] (which updates seq and dims atomically), this driver
/// exposes the production state-vs-dims separation — the gap window
/// that `size()` has to bridge correctly.
///
/// The initial buffer is prefixed with a real image markdown so every
/// seed exercises the image path, not just docs that randomly emit one.
fn drive_with_image_cache<F>(buf: &[u8], mut f: F) -> Result<(), ()>
where
    F: FnMut(&mut TestEditor, &crate::widgets::image_cache::ImageCache, &mut ByteSource),
{
    use crate::file_cache::FileCache;
    use crate::resolvers::image_embed::ImageEmbedResolver;
    use crate::widgets::image_cache::ImageCache;
    use crate::workspace::WsPersistentStore;
    use egui::Context;
    use lb_rs::Uuid;
    use std::sync::{Arc, RwLock};

    let mut src = ByteSource::new(buf);
    // Inject the image at a random line boundary so every seed
    // exercises the image path, but each picks a different topology:
    // image at top, mid-doc, end, inside a heading / blockquote /
    // list item / nested list, plus inline within a paragraph.
    let body = initial_buffer(&mut src);
    let initial = {
        const IMG: &str = "![alt](https://x.test/i.png)";
        let img_block = match src.bias(&[3, 2, 2, 2, 2, 2, 1, 1]) {
            0 => format!("{IMG}\n\n"),
            1 => format!("# heading {IMG}\n\n"),
            2 => format!("paragraph text {IMG} trailing text\n\n"),
            3 => format!("> {IMG}\n\n"),
            4 => format!("- {IMG}\n\n"),
            5 => format!("- outer\n  - {IMG}\n\n"),
            6 => format!("> > {IMG}\n\n"),
            _ => format!("1. ordered\n2. {IMG}\n\n"),
        };
        let lines: Vec<&str> = body.split_inclusive('\n').collect();
        let pos = if lines.is_empty() {
            0
        } else {
            (src.draw(256) * 256 + src.draw(256)) % (lines.len() + 1)
        };
        let mut out = String::with_capacity(body.len() + img_block.len());
        for (i, line) in lines.iter().enumerate() {
            if i == pos {
                out.push_str(&img_block);
            }
            out.push_str(line);
        }
        if pos >= lines.len() {
            out.push_str(&img_block);
        }
        out
    };
    let n = event_count(&mut src);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
        let editor = super::super::Editor::new(
            &initial,
            file_id,
            None,
            super::super::MdResources {
                ctx,
                core: lb,
                persistence,
                link_resolver: Box::new(()),
                embeds: embed,
                files,
            },
            super::super::MdConfig {
                readonly: false,
                ext: "md".to_string(),
                tablet_or_desktop: true,
            },
        );
        let mut ws = TestEditor::from_editor(editor);
        for _ in 0..n {
            f(&mut ws, &image_cache_handle, &mut src);
        }
    }));
    result.map_err(|_| ())
}

/// Current cursor (end of selection) and last cursor position.
fn cursor_and_len(ws: &TestEditor) -> (Grapheme, usize) {
    let buf = &ws.editor.edit.renderer.buffer.current;
    (buf.selection.1, buf.segs.last_cursor_position().0)
}

fn initial_buffer(src: &mut ByteSource) -> String {
    gen_doc(src, &Features::all())
}

fn no_panic_under_edits_check(buf: &[u8]) -> Result<(), &'static str> {
    drive(buf, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
    })
    .map_err(|_| "editor panicked under random edit sequence")
}

fn cursor_in_bounds_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut violation: Option<String> = None;
    drive(buf, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
        let buffer = &ws.editor.edit.renderer.buffer.current;
        let last = buffer.segs.last_cursor_position();
        let (s, e) = buffer.selection;
        if violation.is_none() && (s > e || e > last) {
            violation = Some(format!("selection ({},{}) out of bounds; last={}", s.0, e.0, last.0));
        }
    })
    .map_err(|_| "editor panicked while checking cursor")?;
    if violation.is_some() { Err("cursor selection out of buffer bounds") } else { Ok(()) }
}

/// Sweeps `[0, last_cursor_position]` of the editor's current buffer:
/// for each offset, jump the cursor there, re-render, and check that it
/// has an on-screen representation (`cursor_line` Some). Returns the
/// first offset that doesn't, or `None`. The cursor-jump + frame is what
/// catches reveal-gated holes (syntax that only emits fragments when the
/// cursor sits in it).
fn cursor_visible_sweep(ws: &mut TestEditor) -> Option<String> {
    let last = ws
        .editor
        .edit
        .renderer
        .buffer
        .current
        .segs
        .last_cursor_position();
    for i in 0..=last.0 {
        let g = Grapheme(i);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(g),
                end: Location::Grapheme(g),
            },
        });
        ws.enter_frame();
        if ws.editor.edit.cursor_line(g).is_none() {
            let text = ws.editor.edit.renderer.buffer.current.text.clone();
            return Some(format!(
                "offset {i} has no on-screen representation (last={})\n  buffer ({} bytes): {:?}",
                last.0,
                text.len(),
                text,
            ));
        }
    }
    None
}

/// Property: every valid cursor offset has an on-screen representation.
/// Sweeps the pristine generated doc *and* the state after each edit —
/// the pristine sweep is what catches fragile canonical structures
/// (`drive_features` applies an edit before its first callback, so
/// without this the as-generated doc is never checked on its own).
fn cursor_renders_check(buf: &[u8], features: &Features) -> Result<(), String> {
    let mut violation: Option<String> = None;
    let mut swept_pristine = false;
    drive_features(buf, features, |ws, src| {
        if violation.is_some() {
            return;
        }
        if !swept_pristine {
            swept_pristine = true;
            if let Some(v) = cursor_visible_sweep(ws) {
                violation = Some(v);
                return;
            }
        }
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event_with(src, cursor, len, features));
        ws.enter_frame();
        violation = cursor_visible_sweep(ws);
    })
    .map_err(|_| "editor panicked while checking cursor render".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: arrow-key navigation always visibly moves the cursor
/// (or stays put at the doc's edge). Pressing arrow-right at every
/// offset in `[0, last)` must yield a different `cursor_line` x/y;
/// pressing arrow-left at every offset in `(0, last]` likewise.
/// Catches the "cursor stuck" feel — advance logic that skips
/// graphemes which would render at the origin's visual position.
fn cursor_visibly_moves_check(buf: &[u8], features: &Features) -> Result<(), String> {
    use super::super::input::{Advance, Increment};
    fn vis(ws: &TestEditor, g: Grapheme) -> Option<(f32, f32, f32)> {
        ws.editor
            .edit
            .cursor_line(g)
            .map(|[top, bot]| (top.x, top.y, bot.y))
    }
    let arrow = |backwards: bool| Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Char),
            backwards,
            extend_selection: false,
        },
    };
    let mut violation: Option<String> = None;
    drive_features_n(buf, features, 1, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event_with(src, cursor, len, features));
        ws.enter_frame();
        if violation.is_some() {
            return;
        }
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        if last.0 == 0 {
            return;
        }
        for i in 0..=last.0 {
            let origin = Grapheme(i);
            // Place at origin, capture rendered position there.
            ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(origin),
                    end: Location::Grapheme(origin),
                },
            });
            ws.enter_frame();
            let Some(origin_pos) = vis(ws, origin) else { continue };

            for (backwards, label) in [(true, "arrow-left"), (false, "arrow-right")] {
                let at_edge = if backwards { i == 0 } else { i == last.0 };
                if at_edge {
                    continue;
                }
                ws.push(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(origin),
                        end: Location::Grapheme(origin),
                    },
                });
                ws.enter_frame();
                ws.push(arrow(backwards));
                ws.enter_frame();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let landed_pos = vis(ws, landed);
                let moved_visually = landed_pos.is_some_and(|p| p != origin_pos);
                if !moved_visually {
                    let text = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "{label} from offset {} landed at {} but cursor didn't move \
                         visually (origin_pos={:?}, landed_pos={:?}); buffer ({} bytes): {:?}",
                        origin.0,
                        landed.0,
                        origin_pos,
                        landed_pos,
                        text.len(),
                        text,
                    ));
                    return;
                }
            }
        }
    })
    .map_err(|_| "editor panicked while checking cursor visible movement".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: on simple ASCII docs (uniform-width whitespace-wrap),
/// cmd+left and cmd+right must not change the cursor's visual row.
/// The "cursor jumps to the next row" symptom — even on plain
/// English prose — manifests when `R(x)` from a row-interior x
/// lands at the row-end offset whose `fragment_at_offset` resolves
/// to the next row (shared wrap boundary).
///
/// Restricting input to simple ASCII rules out the mid-token /
/// non-whitespace-wrap cases where this is unavoidable, so any
/// failure here is a real bug in the general case.
fn cmd_line_jump_preserves_y_check(buf: &[u8]) -> Result<(), String> {
    fn line_jump(backwards: bool) -> Event {
        Event::Select {
            region: Region::ToAdvance {
                advance: Advance::To(Bound::Line),
                backwards,
                extend_selection: false,
            },
        }
    }
    fn place(g: Grapheme) -> Event {
        Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(g),
                end: Location::Grapheme(g),
            },
        }
    }
    fn y_top(ws: &TestEditor, g: Grapheme) -> Option<f32> {
        ws.editor.edit.cursor_line(g).map(|[top, _]| top.y)
    }
    let mut violation: Option<String> = None;
    drive_simple_n(buf, 1, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame_unfocused();
        if violation.is_some() {
            return;
        }
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        for i in 0..=last.0 {
            let origin = Grapheme(i);
            ws.push(place(origin));
            ws.enter_frame_unfocused();
            let Some(y_origin) = y_top(ws, origin) else { continue };

            for (backwards, label) in [(true, "cmd-left"), (false, "cmd-right")] {
                ws.push(place(origin));
                ws.enter_frame_unfocused();
                ws.push(line_jump(backwards));
                ws.enter_frame_unfocused();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let Some(y_landed) = y_top(ws, landed) else { continue };
                if (y_landed - y_origin).abs() > 0.5 {
                    let t = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "{label} changed row at offset {}: landed at offset {} \
                         y_origin={:.1} y_landed={:.1}; buffer ({} bytes): {:?}",
                        origin.0,
                        landed.0,
                        y_origin,
                        y_landed,
                        t.len(),
                        t,
                    ));
                    return;
                }
            }
        }
    })
    .map_err(|_| "editor panicked while checking cmd-line y preservation".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: on simple ASCII docs, char-arrow advances by exactly one
/// grapheme. The `Increment::Char` skip-collapsed loop legitimately
/// fast-forwards through positions that render at the same x (override
/// fragments, atomic clusters) — the ASCII generator produces neither,
/// so any multi-grapheme step here is a bug.
fn arrow_advance_one_grapheme_check(buf: &[u8]) -> Result<(), String> {
    use super::super::input::{Advance, Increment};
    let arrow = |backwards: bool| Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Char),
            backwards,
            extend_selection: false,
        },
    };
    fn place(g: Grapheme) -> Event {
        Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(g),
                end: Location::Grapheme(g),
            },
        }
    }
    let mut violation: Option<String> = None;
    drive_simple_n(buf, 1, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame_unfocused();
        if violation.is_some() {
            return;
        }
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        for i in 0..=last.0 {
            let origin = Grapheme(i);
            for (backwards, label) in [(true, "arrow-left"), (false, "arrow-right")] {
                let at_edge = if backwards { i == 0 } else { i == last.0 };
                if at_edge {
                    continue;
                }
                ws.push(place(origin));
                ws.enter_frame_unfocused();
                ws.push(arrow(backwards));
                ws.enter_frame_unfocused();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let expected = if backwards { Grapheme(i - 1) } else { Grapheme(i + 1) };
                if landed != expected {
                    let t = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "{label} from offset {} landed at {} (expected {}); \
                         buffer ({} bytes): {:?}",
                        origin.0,
                        landed.0,
                        expected.0,
                        t.len(),
                        t,
                    ));
                    return;
                }
            }
        }
    })
    .map_err(|_| "editor panicked while checking arrow-advance grapheme step".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: every cached entry in [`LayoutCache`] equals a freshly-
/// recomputed value. A mismatch means upstream state changed without
/// bumping the relevant dep seq — the cache holds a value that was
/// correct at write time but is stale now. Covers `height`,
/// `height_approx`, `hidden_by_fold`, and `line_prefix_len`.
fn layout_cache_consistent_check(buf: &[u8]) -> Result<(), &'static str> {
    const EPS: f32 = 0.5;
    let mut violation: Option<&'static str> = None;
    drive_with_embeds(buf, |ws, embeds, src| {
        if violation.is_some() {
            return;
        }
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
        // Follow up with a cursor jump that doesn't touch text. This
        // pairs a settled-text frame (cache populated) with a reveal-
        // change frame (cache should reflect the new reveal state) — the
        // shape that exercises `reveal_seq`'s job in `HeightDeps`.
        let (_, len) = cursor_and_len(ws);
        let jump = offset(src, len);
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(jump),
                end: Location::Grapheme(jump),
            },
        });
        // Occasionally complete a fake embed load — exercises
        // `embeds_seq`'s job in `HeightDeps`. The doc generator emits
        // `![alt](https://x.test/i.png)` inline; completing that URL
        // changes the rendered size, so heights for image paragraphs
        // shift across the seq bump.
        if src.bias(&[3, 1]) == 1 {
            let h = (src.draw(8) + 1) as f32 * 50.0;
            embeds.complete("https://x.test/i.png", egui::Vec2::new(300.0, h));
        }
        // Vary viewport width occasionally — exercises `width_seq`'s job
        // in `HeightDeps`. The tail-of-iteration enter_frame is the
        // natural place since width is set per-frame from the canvas
        // size.
        let widths = [800.0, 600.0, 1000.0, 480.0];
        let w = widths[src.bias(&[6, 1, 1, 1]) % widths.len()];
        ws.enter_frame_at(egui::Vec2::new(w, 600.0));

        let arena = comrak::Arena::new();
        let root = ws.editor.edit.renderer.reparse(&arena);
        let nodes: Vec<_> = root.descendants().collect();
        let r = &ws.editor.edit.renderer;

        let cached_h: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| r.get_cached_node_height(n).map(|h| (i, h)))
            .collect();
        let cached_a: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| r.get_cached_node_height_approx(n).map(|h| (i, h)))
            .collect();
        let cached_hbf: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| r.get_cached_hidden_by_fold(n).map(|v| (i, v)))
            .collect();
        let cached_lpl: Vec<_> = nodes
            .iter()
            .enumerate()
            .flat_map(|(i, n)| {
                let (s, e) = r.range_lines(r.node_range(n));
                (s..e).filter_map(move |li| {
                    let line = r.bounds.source_lines[li];
                    r.get_cached_line_prefix_len(n, line).map(|v| (i, line, v))
                })
            })
            .collect();

        // Clear all four caches and rebuild `hidden_by_fold` (a global
        // populate, unlike the others which are computed lazily on read).
        r.layout_cache.height.borrow_mut().clear();
        r.layout_cache.height_approx.borrow_mut().clear();
        r.layout_cache.hidden_by_fold.borrow_mut().clear();
        r.layout_cache.hidden_by_fold_deps.set(None);
        r.layout_cache.line_prefix_len.borrow_mut().clear();
        r.populate_hidden_by_fold(root);

        // populate writes one entry per descendant; if the cache ends up
        // smaller, two different nodes mapped to the same key and one
        // silently clobbered the other.
        let descendant_count = root.descendants().count();
        let cache_size = r.layout_cache.hidden_by_fold.borrow().len();
        if cache_size < descendant_count {
            violation = Some("hidden_by_fold cache key collision (cache shorter than node count)");
        }

        for (i, v) in cached_hbf {
            let fresh = r.hidden_by_fold(nodes[i]);
            if v != fresh {
                violation = Some("cached hidden_by_fold differs from fresh recomputation");
            }
        }
        for (i, line, v) in cached_lpl {
            let fresh = r.line_prefix_len(nodes[i], line);
            if v != fresh {
                violation = Some("cached line_prefix_len differs from fresh recomputation");
            }
        }
        for (i, h) in cached_h {
            let fresh = r.height(nodes[i]);
            if (h - fresh).abs() > EPS {
                violation = Some("cached height differs from fresh recomputation");
            }
        }
        for (i, h) in cached_a {
            let fresh = r.height_approx(nodes[i]);
            if (h - fresh).abs() > EPS {
                violation = Some("cached height_approx differs from fresh recomputation");
            }
        }
    })
    .map_err(|_| "editor panicked while checking cache consistency")?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: every cached layout entry equals a freshly-recomputed
/// value, even when image loads complete mid-stream. Drives the
/// production [`ImageEmbedResolver`] + [`ImageCache`] (rather than the
/// atomic-update `TestEmbeds`) so the state-vs-dims gap is reachable.
///
/// Per iteration: push a random event, optionally complete the fake
/// load with a randomly-sized texture, enter a frame, then snapshot
/// every cached `height` / `height_approx`, clear the caches, and
/// assert each fresh recomputation matches. A bug like "size() reads
/// dims before show() populates it" leaves the cached image-paragraph
/// height at the placeholder while the fresh value reflects the real
/// texture dims, and this check fires.
fn image_load_consistent_check(buf: &[u8]) -> Result<(), &'static str> {
    use egui::{Color32, ColorImage, ImageData, TextureOptions};
    use std::sync::Arc;

    const EPS: f32 = 0.5;
    let mut violation: Option<&'static str> = None;
    drive_with_image_cache(buf, |ws, image_cache, src| {
        if violation.is_some() {
            return;
        }
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();

        // Probabilistically complete a fake load. Random texture
        // dimensions vary the post-load image height so a stale cache
        // is detectably different from a fresh recomputation.
        if src.bias(&[2, 1]) == 1 {
            let w = (src.draw(8) + 1) * 100;
            let h = (src.draw(8) + 1) * 25;
            let pixels = vec![Color32::WHITE; w * h];
            let image = ImageData::Color(Arc::new(ColorImage::new([w, h], pixels)));
            let ctx = ws.editor.edit.renderer.ctx.clone();
            let tex_id = ctx.tex_manager().write().alloc(
                "test_image".into(),
                image,
                TextureOptions::default(),
            );
            image_cache.complete_load("https://x.test/i.png", Ok(tex_id));
        }
        ws.enter_frame();

        let arena = comrak::Arena::new();
        let root = ws.editor.edit.renderer.reparse(&arena);
        let nodes: Vec<_> = root.descendants().collect();
        let r = &ws.editor.edit.renderer;

        let cached_h: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| r.get_cached_node_height(n).map(|h| (i, h)))
            .collect();
        let cached_a: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| r.get_cached_node_height_approx(n).map(|h| (i, h)))
            .collect();

        r.layout_cache.height.borrow_mut().clear();
        r.layout_cache.height_approx.borrow_mut().clear();

        for (i, h) in cached_h {
            let fresh = r.height(nodes[i]);
            if (h - fresh).abs() > EPS {
                violation = Some("cached height differs from fresh recomputation");
            }
        }
        for (i, h) in cached_a {
            let fresh = r.height_approx(nodes[i]);
            if (h - fresh).abs() > EPS {
                violation = Some("cached height_approx differs from fresh recomputation");
            }
        }
    })
    .map_err(|_| "editor panicked while checking image-load consistency")?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: applying a programmatic text edit (one that doesn't push a
/// cursor-moving op) shouldn't move the scroll offset. The bug class:
/// the edit OT-shifts the cursor's offset, the editor mistakes that for
/// a user cursor move, and scroll-to-cursor pulls the viewport toward
/// an off-screen cursor — visible as a "scroll jumped to bottom" when
/// the cursor happens to be near the doc end.
fn fold_toggle_does_not_jump_scroll_check(buf: &[u8]) -> Result<(), &'static str> {
    use super::super::scroll_content::DocScrollContent;
    use crate::widgets::affine_scroll::Action;
    let mut violation: Option<&'static str> = None;
    drive(buf, |ws, src| {
        if violation.is_some() {
            return;
        }
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
        // Place cursor far from where we'll scroll, then scroll away so
        // the cursor is offscreen — the bug only manifests when the
        // cursor isn't visible (otherwise scroll-to-cursor is a no-op).
        let len_after = ws.editor.edit.renderer.last_cursor_position().0;
        if len_after < 10 {
            return;
        }
        let near_end = Grapheme(len_after.saturating_sub(2));
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(near_end),
                end: Location::Grapheme(near_end),
            },
        });
        ws.enter_frame();
        ws.enter_frame(); // settle scroll-to-cursor
        // Scroll toward the top.
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
                .handle(&content, Action::ScrollByPixels(-1500.0));
        }
        ws.enter_frame();
        let before = ws.editor.edit.scroll_area.stored_offset();

        // Programmatic edit: insert a single char somewhere far from the
        // cursor (advance_cursor=false → no Select op, so the user's
        // cursor logically stays put). Mirror what fold-toggle does.
        let edit_pos =
            (src.draw(usize::MAX) % len_after.saturating_sub(1)).min(near_end.0.saturating_sub(1));
        ws.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(Grapheme(edit_pos)),
                end: Location::Grapheme(Grapheme(edit_pos)),
            },
            text: "x".to_string(),
            advance_cursor: false,
        });
        ws.enter_frame();
        ws.enter_frame();
        let after = ws.editor.edit.scroll_area.stored_offset();
        if before != after {
            violation = Some("programmatic edit (no cursor move) shifted scroll offset");
        }
    })
    .map_err(|_| "editor panicked while checking fold-toggle stability")?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: rendering one input-bearing frame followed by an idle frame
/// produces the same scroll offset on both. A difference means the first
/// frame's render landed at a transient scroll position that snapped on
/// the next frame — a one-frame flash for the user.
fn render_stable_after_event_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut violation: Option<&'static str> = None;
    drive(buf, |ws, src| {
        if violation.is_some() {
            return;
        }
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
        let after_event = ws.editor.edit.scroll_area.stored_offset();
        ws.enter_frame();
        let after_idle = ws.editor.edit.scroll_area.stored_offset();
        if after_event != after_idle {
            violation = Some("scroll offset shifted on the idle frame after an event");
        }
    })
    .map_err(|_| "editor panicked while checking stability")?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Property: after any edit sequence, the resulting buffer renders with the
/// same source-range and rect disjointness invariants enforced by the
/// render-only property tests. Checked after every intermediate frame —
/// some bugs only manifest on a single frame and the next frame fixes them.
fn render_invariants_survive_edits_check(
    buf: &[u8], features: &Features,
) -> Result<(), &'static str> {
    const EPS: f32 = 0.01;
    let mut violation: Option<&'static str> = None;
    drive_features(buf, features, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event_with(src, cursor, len, features));
        ws.enter_frame();

        let frame_fragments: Vec<_> = ws
            .editor
            .edit
            .renderer
            .fragments
            .iter()
            .map(|f| (f.source_range, f.rect))
            .collect();

        // source-range disjointness (non-empty)
        let mut ranges: Vec<_> = frame_fragments
            .iter()
            .filter(|(r, _)| r.0 < r.1)
            .map(|(r, _)| *r)
            .collect();
        ranges.sort_by_key(|r| r.0);
        for w in ranges.windows(2) {
            if w[0].1 > w[1].0 {
                violation = violation.or(Some("two fragments cover overlapping source bytes"));
            }
        }

        // rect disjointness
        let rects: Vec<_> = frame_fragments
            .iter()
            .map(|(_, r)| *r)
            .filter(|r| r.width() > EPS && r.height() > EPS)
            .collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let inter = rects[i].intersect(rects[j]);
                if inter.width() > EPS && inter.height() > EPS {
                    violation = violation.or(Some("two fragment rects overlap on screen"));
                }
            }
        }
    })
    .map_err(|_| "editor panicked during render check")?;

    violation.map(Err).unwrap_or(Ok(()))
}

/// Up/down arrow invariant: pressing arrow-up at any offset whose y is
/// strictly greater than the doc's minimum y must produce a cursor
/// position with strictly smaller y (and likewise arrow-down vs. max y).
/// Catches both "cursor stuck on same row" and "cursor moves but stays
/// at same y" bugs.
fn cursor_visibly_moves_vertical_check_simple(buf: &[u8]) -> Result<(), String> {
    use super::super::input::{Advance, Increment};
    // Midpoint y, not rect top: a fragment's rect top shifts by
    // `inline_pad` when the fragment carries a background, so two
    // cursors on the same logical row can have different `top.y`.
    // The midpoint stays at the row center regardless of bg padding.
    fn mid_y(ws: &TestEditor, g: Grapheme) -> Option<f32> {
        ws.editor
            .edit
            .cursor_line(g)
            .map(|[top, bot]| (top.y + bot.y) * 0.5)
    }
    let arrow = |backwards: bool| Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Lines(1)),
            backwards,
            extend_selection: false,
        },
    };
    let mut violation: Option<String> = None;
    drive_simple_n(buf, 1, |ws, src| {
        let (cursor, len) = cursor_and_len(ws);
        ws.push(event(src, cursor, len));
        ws.enter_frame();
        if violation.is_some() {
            return;
        }
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        if last.0 == 0 {
            return;
        }
        // First sweep: place at every offset and record min/max y so we
        // can tell which origins have a row above / below them.
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for i in 0..=last.0 {
            ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(Grapheme(i)),
                    end: Location::Grapheme(Grapheme(i)),
                },
            });
            ws.enter_frame();
            if let Some(ty) = mid_y(ws, Grapheme(i)) {
                min_y = min_y.min(ty);
                max_y = max_y.max(ty);
            }
        }
        if !min_y.is_finite() || !max_y.is_finite() || max_y - min_y < 0.5 {
            return; // single-row doc; up/down has nowhere to go
        }
        const EPS: f32 = 0.5;
        for i in 0..=last.0 {
            let origin = Grapheme(i);
            ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(origin),
                    end: Location::Grapheme(origin),
                },
            });
            ws.enter_frame();
            let Some(origin_y) = mid_y(ws, origin) else { continue };
            for (backwards, label) in [(true, "arrow-up"), (false, "arrow-down")] {
                let has_neighbor =
                    if backwards { origin_y > min_y + EPS } else { origin_y < max_y - EPS };
                if !has_neighbor {
                    continue;
                }
                ws.push(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(origin),
                        end: Location::Grapheme(origin),
                    },
                });
                ws.enter_frame();
                ws.push(arrow(backwards));
                ws.enter_frame();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let Some(landed_y) = mid_y(ws, landed) else { continue };
                let strictly_progressed =
                    if backwards { landed_y < origin_y - EPS } else { landed_y > origin_y + EPS };
                if !strictly_progressed {
                    let text = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "{label} from offset {} (y={:.1}) landed at {} (y={:.1}) but \
                         doc has rows beyond (min_y={:.1}, max_y={:.1}); \
                         buffer ({} bytes): {:?}",
                        origin.0,
                        origin_y,
                        landed.0,
                        landed_y,
                        min_y,
                        max_y,
                        text.len(),
                        text,
                    ));
                    return;
                }
            }
        }
    })
    .map_err(|_| "editor panicked while checking vertical visible movement".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

/// Round-trip invariant: pressing up then down (or down then up)
/// should leave the cursor on the **same visual row** it started on
/// — provided origin is not at the top edge (for up-then-down) or
/// bottom edge (for down-then-up). x may drift by ≤ one cluster
/// width due to cluster-snap on the intermediate row, but y must
/// return to the origin row. Catches "arrow moves the cursor far to
/// one side and doesn't come back" bugs.
fn cursor_vertical_round_trip_check(buf: &[u8]) -> Result<(), String> {
    cursor_vertical_round_trip_check_inner(buf, None)
}

fn cursor_vertical_round_trip_check_inner(
    buf: &[u8], features: Option<&Features>,
) -> Result<(), String> {
    use super::super::input::{Advance, Increment};
    fn mid_y(ws: &TestEditor, g: Grapheme) -> Option<f32> {
        ws.editor
            .edit
            .cursor_line(g)
            .map(|[top, bot]| (top.y + bot.y) * 0.5)
    }
    let arrow = |backwards: bool| Event::Select {
        region: Region::ToAdvance {
            advance: Advance::By(Increment::Lines(1)),
            backwards,
            extend_selection: false,
        },
    };
    let place = |g: Grapheme| Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(g),
            end: Location::Grapheme(g),
        },
    };
    let mut violation: Option<String> = None;
    let body = |ws: &mut TestEditor, src: &mut ByteSource| {
        let (cursor, len) = cursor_and_len(ws);
        let ev = match features {
            Some(f) => event_with(src, cursor, len, f),
            None => event(src, cursor, len),
        };
        ws.push(ev);
        ws.enter_frame();
        if violation.is_some() {
            return;
        }
        let last = ws
            .editor
            .edit
            .renderer
            .buffer
            .current
            .segs
            .last_cursor_position();
        if last.0 == 0 {
            return;
        }
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for i in 0..=last.0 {
            ws.push(place(Grapheme(i)));
            ws.enter_frame();
            if let Some(ty) = mid_y(ws, Grapheme(i)) {
                min_y = min_y.min(ty);
                max_y = max_y.max(ty);
            }
        }
        if !min_y.is_finite() || !max_y.is_finite() || max_y - min_y < 0.5 {
            return;
        }
        const EPS: f32 = 0.5;
        for i in 0..=last.0 {
            let origin = Grapheme(i);
            ws.push(place(origin));
            ws.enter_frame();
            let Some(origin_y) = mid_y(ws, origin) else { continue };
            // up-then-down: only meaningful if a row exists above.
            if origin_y > min_y + EPS {
                ws.push(place(origin));
                ws.enter_frame();
                ws.push(arrow(true));
                ws.enter_frame();
                ws.push(arrow(false));
                ws.enter_frame();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let Some(landed_y) = mid_y(ws, landed) else { continue };
                if (landed_y - origin_y).abs() > EPS {
                    let text = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "up-down round-trip from offset {} (y={:.1}) landed at {} \
                         (y={:.1}); buffer ({} bytes): {:?}",
                        origin.0,
                        origin_y,
                        landed.0,
                        landed_y,
                        text.len(),
                        text,
                    ));
                    return;
                }
            }
            // down-then-up: only meaningful if a row exists below.
            if origin_y < max_y - EPS {
                ws.push(place(origin));
                ws.enter_frame();
                ws.push(arrow(false));
                ws.enter_frame();
                ws.push(arrow(true));
                ws.enter_frame();
                let landed = ws.editor.edit.renderer.buffer.current.selection.1;
                let Some(landed_y) = mid_y(ws, landed) else { continue };
                if (landed_y - origin_y).abs() > EPS {
                    let text = ws.editor.edit.renderer.buffer.current.text.clone();
                    violation = Some(format!(
                        "down-up round-trip from offset {} (y={:.1}) landed at {} \
                         (y={:.1}); buffer ({} bytes): {:?}",
                        origin.0,
                        origin_y,
                        landed.0,
                        landed_y,
                        text.len(),
                        text,
                    ));
                    return;
                }
            }
        }
    };
    let result = match features {
        Some(f) => drive_features_n(buf, f, 1, body),
        None => drive_simple_n(buf, 1, body),
    };
    result.map_err(|_| "editor panicked while checking vertical round trip".to_string())?;
    violation.map(Err).unwrap_or(Ok(()))
}

fn no_panic_structured_init_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = gen_doc(&mut src, &Features::all());
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ws = TestEditor::new(&md);
    }))
    .map(|_| ())
    .map_err(|_| "editor panicked on init")
}

/// Column count of leading whitespace, expanding tabs to 4-col stops.
fn leading_indent_cols(text: &str) -> usize {
    let mut cols = 0;
    for c in text.chars() {
        match c {
            ' ' => cols += 1,
            '\t' => cols = (cols / 4 + 1) * 4,
            _ => break,
        }
    }
    cols
}

/// Tab indents only the selected line; it never touches any other
/// line's source. Children are not dragged along — a tight child
/// simply re-parents to a sibling. This is the conservation form of
/// "indenting a list item must not also indent its children": every
/// non-selected line is byte-identical before and after.
fn indent_only_moves_selected_line_check(buf: &[u8], f: &Features) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let doc = gen_doc(&mut src, f);
    let before: Vec<&str> = doc.split_inclusive('\n').collect();
    if before.len() < 2 {
        return Ok(());
    }

    // A non-first line: indent only nests a line under a *prior* sibling,
    // so first-line (and any-line) selection never fires a real indent
    // (0%). Restricting to non-first lines exercises an actual indent on
    // ~7% of seeds — the case where dragging a child would be the bug.
    let line_idx = 1 + src.draw(usize::MAX) % (before.len() - 1);
    let line_text = before[line_idx].trim_end_matches('\n');
    let line_start: usize = before
        .iter()
        .take(line_idx)
        .map(|l| l.chars().count())
        .sum();
    let line_len = line_text.chars().count();
    if line_len == 0 {
        return Ok(());
    }
    let in_line_offset = (src.draw(usize::MAX) % line_len).max(1);
    let cursor = Grapheme(line_start + in_line_offset);

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();

    let after_text = ws.get_text().to_string();
    let after: Vec<&str> = after_text.split_inclusive('\n').collect();
    if before.len() != after.len() {
        return Err("tab changed line count");
    }
    for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
        if i != line_idx && b != a {
            return Err("tab modified a line other than the selected one");
        }
    }
    Ok(())
}

/// Encode `cols` of indentation using a randomly chosen mix of spaces
/// and tabs. Tabs use 4-col stops; some col counts can only be
/// produced by mixed encoding.
fn gen_indent_encoding(cols: usize, src: &mut ByteSource) -> String {
    if cols == 0 {
        return String::new();
    }
    match src.bias(&[3, 1, 1]) {
        // pure spaces
        0 => " ".repeat(cols),
        // pure tabs (only valid when cols aligns to tab stops)
        1 if cols % 4 == 0 => "\t".repeat(cols / 4),
        // mixed: as many tabs as fit, spaces for the remainder
        _ => {
            let tabs = cols / 4;
            let trailing_spaces = cols - tabs * 4;
            "\t".repeat(tabs) + &" ".repeat(trailing_spaces)
        }
    }
}

/// Generate a nested-list doc 1-4 levels deep, mixing four marker
/// kinds: plain `- `, task `- [ ] `, ordered `1. `, and numbered task
/// `1. [ ] `. Each level's indent equals the cumulative sum of prior
/// levels' paddings (so structure parses cleanly). Indent encoding
/// (pure spaces / pure tabs / mixed) is randomized per level.
///
/// Returns the doc plus per-level padding so callers can predict the
/// expected deindent amount as the parent level's padding.
fn gen_nested_list_doc(src: &mut ByteSource) -> (String, Vec<usize>) {
    let depth = 1 + src.bias(&[3, 3, 2, 1]);
    let mut lines = Vec::new();
    let mut paddings = Vec::new();
    let mut indent_cols = 0usize;
    for level in 0..depth {
        let indent = gen_indent_encoding(indent_cols, src);
        let (marker, padding) = pick_marker(src);
        lines.push(format!("{indent}{marker}item{level}"));
        indent_cols += padding;
        paddings.push(padding);
    }
    (lines.join("\n") + "\n", paddings)
}

/// Per CommonMark §5.2, a list marker is "the bullet/number followed
/// by 1-4 spaces" (extra trailing spaces past the standard one bump
/// the per-level padding). Task items have `[ ]` as content (per
/// comrak), so the `[X]` variants don't change padding. Numbered
/// markers contribute `digits + . + spaces` to padding.
fn pick_marker(src: &mut ByteSource) -> (&'static str, usize) {
    match src.bias(&[3, 1, 1, 1, 1, 1, 1, 1, 1]) {
        // plain items
        0 => ("- ", 2),
        1 => ("-  ", 3),  // 2 spaces after marker → padding 3
        2 => ("-   ", 4), // 3 spaces after marker → padding 4
        // task items (`[ ]` is content, doesn't affect padding)
        3 => ("- [ ] ", 2),
        4 => ("- [x] ", 2), // checked
        5 => ("- [X] ", 2), // checked variant
        // ordered
        6 => ("1. ", 3),
        7 => ("1.  ", 4), // extra space
        // numbered task item
        _ => ("1. [ ] ", 3),
    }
}

/// Property: shift-tab on the deepest line of a nested list strips
/// exactly one level of indent — the parent ancestor's marker-width
/// in cols — regardless of whether the indent is encoded with pure
/// spaces, pure tabs, or a mix. Catches the regression class where
/// outer-item-claims-everything (via `consume_indent_columns(text,
/// MAX)`) caused deindent to strip the whole indent in one shot.
///
/// Other lines must be unchanged; the targeted line's content past
/// the leading whitespace must be unchanged.
fn shift_tab_strips_one_level_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let (doc, paddings) = gen_nested_list_doc(&mut src);
    let lines: Vec<&str> = doc.split_inclusive('\n').collect();
    if lines.len() < 2 {
        return Ok(()); // top-level only — shift-tab is a no-op, nothing to verify
    }

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let line_idx = lines.len() - 1;
    let target_line = lines[line_idx].trim_end_matches('\n');
    let initial_cols = leading_indent_cols(target_line);

    let line_start: usize = lines.iter().take(line_idx).map(|l| l.chars().count()).sum();
    let cursor = Grapheme(line_start + target_line.chars().count());
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();

    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();

    let new_doc = ws.get_text().to_string();
    let new_lines: Vec<&str> = new_doc.split_inclusive('\n').collect();
    if new_lines.len() != lines.len() {
        return Err("shift-tab changed the line count");
    }
    for (i, (old, new)) in lines.iter().zip(new_lines.iter()).enumerate() {
        if i != line_idx && old != new {
            return Err("shift-tab changed a line other than the cursor's");
        }
    }

    // Expected change: parent level's marker padding (2 for `- `,
    // 3 for `1. `).
    let parent_padding = paddings[paddings.len() - 2];
    let new_target = new_lines[line_idx].trim_end_matches('\n');
    let final_cols = leading_indent_cols(new_target);
    let expected_cols = initial_cols.saturating_sub(parent_padding);
    if final_cols != expected_cols {
        return Err("shift-tab indent change != parent level's padding");
    }

    let initial_content = target_line.trim_start_matches([' ', '\t']);
    let final_content = new_target.trim_start_matches([' ', '\t']);
    if initial_content != final_content {
        return Err("shift-tab changed line content past leading whitespace");
    }

    Ok(())
}

/// Shift-tab on an item must not break syntax: every line that was a
/// list item before the deindent must still be a list item after.
/// Catches the bug where deindenting a parent leaves descendants at
/// indents that re-parse as code blocks or orphaned content.
fn shift_tab_preserves_item_lines_check(buf: &[u8]) -> Result<(), &'static str> {
    use comrak::Arena;
    use comrak::nodes::NodeValue;
    use lb_rs::model::text::offset_types::RangeExt as _;

    let mut src = ByteSource::new(buf);
    // Deindent operates on leading whitespace; the engine nests lists
    // via same-line markers (`+ - [ ]`), where deindent finds nothing
    // to strip (0% fire). The bespoke whitespace-indented chain is what
    // gives this check teeth (~60% of seeds apply a real deindent).
    let (doc, _paddings) = gen_nested_list_doc(&mut src);
    let lines: Vec<&str> = doc.split_inclusive('\n').collect();
    if lines.len() < 2 {
        return Ok(());
    }

    let line_idx = src.draw(usize::MAX) % lines.len();
    let line_text = lines[line_idx].trim_end_matches('\n');
    let line_start: usize = lines.iter().take(line_idx).map(|l| l.chars().count()).sum();
    let line_len = line_text.chars().count();
    if line_len == 0 {
        return Ok(());
    }
    let in_line_offset = (src.draw(usize::MAX) % line_len).max(1);
    let cursor = Grapheme(line_start + in_line_offset);

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let item_lines = |ws: &mut TestEditor| -> Vec<bool> {
        let arena = Arena::new();
        let root = ws.editor.edit.renderer.reparse(&arena);
        (0..ws.editor.edit.renderer.bounds.source_lines.len())
            .map(|i| {
                let line = ws.editor.edit.renderer.bounds.source_lines[i];
                let container = ws
                    .editor
                    .edit
                    .renderer
                    .deepest_container_block_at_offset(root, line.end());
                matches!(
                    &container.data.borrow().value,
                    NodeValue::Item(_) | NodeValue::TaskItem(_)
                )
            })
            .collect()
    };
    let before = item_lines(&mut ws);

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();

    let after = item_lines(&mut ws);
    if before.len() != after.len() {
        return Err("shift-tab changed line count");
    }
    for (b, a) in before.iter().zip(after.iter()) {
        if *b && !*a {
            return Err("shift-tab made a list item line stop being a list item");
        }
    }
    Ok(())
}

/// Property: typing the opening triple-backtick of a code block at
/// any cursor position, optionally followed by a language tag /
/// newline / content (a real user's full code-block flow), must not
/// panic. Reported as an intermittent crash; `no_panic_under_edits`
/// is biased toward single keystrokes so adjacent triples are rare.
///
/// The table caps this at seed 302: that seed exposes an
/// index-out-of-bounds panic in `lb_rs/src/model/text/unicode_segs.rs:25`
/// while `Buffer::update` inverts a delta whose grapheme range walks off
/// the end of the snapshot — buffer-side input handling, outside the
/// layout pipeline. Raise the cap once the lb-rs delta math is tightened.
fn no_panic_typing_triple_backtick_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let doc = gen_doc(&mut src, &Features::all());
    let max_cursor = doc.chars().count();
    let cursor = if max_cursor == 0 { 0 } else { src.draw(usize::MAX) % (max_cursor + 1) };

    // After ```, optionally type one of these continuations.
    // Single-letter lang tokens (`s` → ARM Assembly, etc.) catch
    // grammars that compile-panic when first used.
    let suffix: &str = match src.bias(&[2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1]) {
        0 => "",
        1 => "\n",
        2 => "rust",
        3 => "rust\n",
        4 => "rust\nfn main() {}\n",
        5 => "\n```",
        6 => "\nfoo\n```",
        7 => "s",
        8 => "s\nx\n```",
        9 => "c",
        10 => "r",
        _ => "d",
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ws = TestEditor::new(&doc);
        ws.enter_frame();
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(Grapheme(cursor)),
                end: Location::Grapheme(Grapheme(cursor)),
            },
        });
        ws.enter_frame();
        // type each grapheme of "```{suffix}" in its own frame
        let typed: String = format!("```{suffix}");
        for ch in typed.chars() {
            ws.push(Event::Replace {
                region: Region::Selection,
                text: ch.to_string(),
                advance_cursor: true,
            });
            ws.enter_frame();
        }
    }));
    result.map_err(|_| "panicked typing triple backtick + continuation")
}

fn line_kinds(ws: &mut TestEditor) -> Vec<Option<&'static str>> {
    use comrak::Arena;
    use comrak::nodes::NodeValue;
    use lb_rs::model::text::offset_types::RangeExt as _;
    let arena = Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    (0..ws.editor.edit.renderer.bounds.source_lines.len())
        .map(|i| {
            let line = ws.editor.edit.renderer.bounds.source_lines[i];
            let container = ws
                .editor
                .edit
                .renderer
                .deepest_container_block_at_offset(root, line.end());
            match &container.data.borrow().value {
                NodeValue::Item(_) | NodeValue::TaskItem(_) => Some("item"),
                NodeValue::BlockQuote => Some("bq"),
                NodeValue::Alert(_) => Some("alert"),
                _ => None,
            }
        })
        .collect()
}

fn pos_in_line(src: &mut ByteSource, lines: &[&str], line_idx: usize) -> Grapheme {
    let line_text = lines[line_idx].trim_end_matches('\n');
    let line_start: usize = lines.iter().take(line_idx).map(|l| l.chars().count()).sum();
    let line_len = line_text.chars().count().max(1);
    let off = (src.draw(usize::MAX) % line_len).max(1).min(line_len);
    Grapheme(line_start + off)
}

/// Tab/shift-tab don't break the document: structural line kinds
/// (Item, TaskItem, BlockQuote, Alert) survive the op, and line
/// count is preserved. Random direction and selection (single
/// cursor or multi-row range).
fn indent_doesnt_break_document_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    // One container family per seed keeps nesting homogeneous, so a
    // deindent never escapes into a different-family container (the
    // case where a line legitimately changes structural kind). Lists
    // and blockquotes are exercised across seeds, not within one doc.
    let f = if src.bias(&[1, 1]) == 0 {
        Features::nested_lists()
    } else {
        Features::nested_blockquotes()
    };
    let doc = gen_doc(&mut src, &f);
    let lines: Vec<&str> = doc.split_inclusive('\n').collect();
    if lines.is_empty() {
        return Ok(());
    }

    let deindent = src.bias(&[1, 1]) == 0;
    let multi_row = src.bias(&[1, 1]) == 1;

    let a_idx = src.draw(usize::MAX) % lines.len();
    let b_idx = if multi_row { src.draw(usize::MAX) % lines.len() } else { a_idx };
    let (lo, hi) = if a_idx <= b_idx { (a_idx, b_idx) } else { (b_idx, a_idx) };
    let lo_pos = pos_in_line(&mut src, &lines, lo);
    let hi_pos = pos_in_line(&mut src, &lines, hi);

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    let before = line_kinds(&mut ws);

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(lo_pos),
            end: Location::Grapheme(hi_pos),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent });
    ws.enter_frame();

    let after = line_kinds(&mut ws);
    if before.len() != after.len() {
        return Err("operation changed line count");
    }
    for (b, a) in before.iter().zip(after.iter()) {
        if let Some(b_kind) = b {
            if a.as_deref() != Some(b_kind) {
                return Err("structural line kind changed (line broke)");
            }
        }
    }
    Ok(())
}

/// Per source line: does it *begin its own* non-empty list item? Distinct
/// from [`line_kinds`], which only asks "is this line inside an item" — a
/// collapsed marker is still inside its parent's item, so the coarse check
/// misses it. Empty-content items are excluded: an empty marker can't
/// interrupt a paragraph either, so indenting one legitimately collapses it.
fn item_begin_lines(ws: &mut TestEditor) -> Vec<bool> {
    use comrak::Arena;
    use comrak::nodes::NodeValue;
    use lb_rs::model::text::offset_types::RangeExt as _;
    let arena = Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    (0..ws.editor.edit.renderer.bounds.source_lines.len())
        .map(|i| {
            let r = &ws.editor.edit.renderer;
            let line = r.bounds.source_lines[i];
            let container = r.deepest_container_block_at_offset(root, line.end());
            let is_item = matches!(
                container.data.borrow().value,
                NodeValue::Item(_) | NodeValue::TaskItem(_)
            );
            if !is_item || r.node_first_line_idx(container) != i {
                return false;
            }
            let content = r.line_content(container, line);
            !r.buffer[content].trim().is_empty()
        })
        .collect()
}

/// Indenting a non-empty ordered-list item must not collapse it into the
/// parent item's paragraph: it has to stay its own list-item marker. Anchored
/// by line index (indent only inserts leading whitespace, never adds/removes
/// lines), and immune to the indent/deindent child-cascade asymmetry because
/// it only inspects the indented marker line itself.
fn indent_preserves_marker_check(buf: &[u8], f: &Features) -> Result<(), String> {
    let mut src = ByteSource::new(buf);
    let doc = gen_doc(&mut src, f);
    let lines: Vec<&str> = doc.split_inclusive('\n').collect();
    if lines.len() < 2 {
        return Ok(());
    }

    // a non-first line, so there's a prior sibling to nest under
    let line_idx = 1 + (src.draw(usize::MAX) % (lines.len() - 1));
    let line_len = lines[line_idx].trim_end_matches('\n').chars().count();
    if line_len == 0 {
        return Ok(());
    }
    let line_start: usize = lines.iter().take(line_idx).map(|l| l.chars().count()).sum();
    let cursor = Grapheme(line_start + (src.draw(usize::MAX) % line_len).max(1));

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();
    let before = item_begin_lines(&mut ws);

    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent: false });
    ws.enter_frame();

    let after = item_begin_lines(&mut ws);
    if before.len() != after.len() {
        return Err(format!("indent changed line count: {doc:?}"));
    }
    for i in 0..before.len() {
        if before[i] && !after[i] {
            return Err(format!("line {i} stopped beginning a list item after indent: {doc:?}"));
        }
    }
    Ok(())
}

/// Permissiveness — catches a trivial "always no-op" impl that
/// would pass `indent_doesnt_break_document` vacuously. When the op
/// is clearly applicable, it must mutate.
///
/// Clearly applicable:
/// - shift-tab: cursor's line is structurally nested inside a same-
///   family container (nested item / nested quote).
/// - tab: cursor isn't on line 0, and the prior line sits inside a
///   structural container the cursor's line isn't already inside.
fn indent_is_permissive_check(buf: &[u8]) -> Result<(), &'static str> {
    use comrak::Arena;
    use comrak::nodes::NodeValue;
    use lb_rs::model::text::offset_types::RangeExt as _;

    let mut src = ByteSource::new(buf);
    // One-item-per-line, single-family docs: this check predicts the
    // editor's indent applicability and asserts the op isn't a no-op,
    // so it needs whitespace-indented nesting. The unified generator's
    // same-line marker nesting (`+ - [ ] foo`) deindents to a no-op the
    // predictor doesn't model — a separate edge case, tracked apart.
    let doc = if src.bias(&[1, 1]) == 0 {
        gen_nested_list_doc(&mut src).0
    } else {
        gen_nested_bq_doc(&mut src).0
    };
    let lines: Vec<&str> = doc.split_inclusive('\n').collect();
    if lines.is_empty() {
        return Ok(());
    }

    let deindent = src.bias(&[1, 1]) == 0;
    let line_idx = src.draw(usize::MAX) % lines.len();
    if lines[line_idx].trim_end_matches('\n').is_empty() {
        return Ok(()); // blank-line cursor is a boundary case, out of scope
    }
    let cursor = pos_in_line(&mut src, &lines, line_idx);

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    let should_change = {
        let arena = Arena::new();
        let root = ws.editor.edit.renderer.reparse(&arena);
        let line = ws.editor.edit.renderer.bounds.source_lines[line_idx];
        let cur = ws
            .editor
            .edit
            .renderer
            .deepest_container_block_at_offset(root, line.end());
        if deindent {
            let cur_kind = &cur.data.borrow().value;
            let same_family = |a: &comrak::nodes::AstNode<'_>| match cur_kind {
                NodeValue::Item(_) | NodeValue::TaskItem(_) => {
                    matches!(&a.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
                }
                NodeValue::BlockQuote | NodeValue::Alert(_) => {
                    matches!(&a.data.borrow().value, NodeValue::BlockQuote | NodeValue::Alert(_))
                }
                _ => false,
            };
            cur.ancestors().skip(1).any(same_family)
        } else if line_idx == 0 {
            false
        } else {
            let prior = ws.editor.edit.renderer.bounds.source_lines[line_idx - 1];
            let prior_cur = ws
                .editor
                .edit
                .renderer
                .deepest_container_block_at_offset(root, prior.end());
            // any structural ancestor of prior that's not already in cur's chain
            prior_cur.ancestors().any(|a| {
                let structural = matches!(
                    &a.data.borrow().value,
                    NodeValue::Item(_)
                        | NodeValue::TaskItem(_)
                        | NodeValue::BlockQuote
                        | NodeValue::Alert(_)
                        | NodeValue::FootnoteDefinition(_)
                );
                let in_cur = cur.ancestors().any(|c| c.same_node(a));
                structural && !in_cur
            })
        }
    };

    if !should_change {
        return Ok(());
    }

    let before = ws.get_text().to_string();
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();
    ws.push(Event::Indent { deindent });
    ws.enter_frame();
    let after = ws.get_text().to_string();

    if before == after {
        return Err("op was clearly applicable but produced a no-op");
    }
    Ok(())
}

/// Per CommonMark §5.1, a block quote marker is `>` optionally
/// followed by a single space. Both forms must work — and they can
/// be mixed at different nesting levels (e.g., `>> foo` is outer `>`
/// + inner `> `).
fn pick_bq_marker(src: &mut ByteSource) -> &'static str {
    if src.bias(&[3, 1]) == 0 { "> " } else { ">" }
}

/// Generate a nested blockquote doc 1-4 levels deep with random
/// space / no-space marker variants per level. Returns the doc plus
/// per-level markers for prediction.
fn gen_nested_bq_doc(src: &mut ByteSource) -> (String, Vec<&'static str>) {
    let depth = 1 + src.bias(&[3, 3, 2, 1]);
    let mut markers = Vec::new();
    let mut prefix = String::new();
    for _ in 0..depth {
        let m = pick_bq_marker(src);
        markers.push(m);
        prefix += m;
    }
    (format!("{prefix}foo\n"), markers)
}

/// Property: shift-tab on the content of a nested blockquote removes
/// exactly the innermost `>`/`> ` marker chars, leaving outer markers
/// intact. Mirrors `shift_tab_strips_one_level` but for marker-based
/// containers (bqs, where the level isn't whitespace).
fn shift_tab_strips_one_bq_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let (doc, markers) = gen_nested_bq_doc(&mut src);
    if markers.len() < 2 {
        return Ok(()); // single-level — shift-tab fully exits the bq, separate concern
    }

    let outer_prefix_len: usize = markers
        .iter()
        .take(markers.len() - 1)
        .map(|m| m.chars().count())
        .sum();
    let inner_marker_len = markers.last().unwrap().chars().count();

    let mut ws = TestEditor::new(&doc);
    ws.enter_frame();

    // Cursor at end of the line content (after all markers + "foo").
    let total_prefix_len: usize = markers.iter().map(|m| m.chars().count()).sum();
    let cursor = Grapheme(total_prefix_len + 3);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(cursor),
            end: Location::Grapheme(cursor),
        },
    });
    ws.enter_frame();

    ws.push(Event::Indent { deindent: true });
    ws.enter_frame();

    let new_doc = ws.get_text().to_string();
    let mut expected = String::new();
    expected.extend(doc.chars().take(outer_prefix_len));
    expected.extend(doc.chars().skip(outer_prefix_len + inner_marker_len));
    if new_doc != expected {
        return Err("shift-tab on nested bq didn't remove exactly the innermost marker");
    }
    Ok(())
}
