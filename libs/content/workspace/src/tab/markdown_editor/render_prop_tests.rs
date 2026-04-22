//! Property tests for `MdRender` — the core markdown renderer shared by
//! `MdLabel` and `MdEdit`.
//!
//! # Properties
//!
//! - **Width monotonicity.** For fixed `md`, widening the viewport cannot
//!   increase height: `w2 >= w1 => height(md, w2) <= height(md, w1)`.
//! - **No panic.** Rendering an arbitrary UTF-8 string does not panic.
//!   Static state only — operation sequences belong to `MdEdit` / `Editor`.
//! - **Galley source-range disjointness.** Each non-override galley carries
//!   the source range it was rendered from. No two galleys cover overlapping
//!   source bytes. (Override galleys — link previews, GFM alert labels —
//!   carry no source and are excluded.) Catches double-rendering of a span.
//! - **Galley rect disjointness.** No two galleys' on-screen rects overlap.
//!   Adjacency (touching edges) is allowed; positive intersection area is
//!   not. Catches layout-level overlap (e.g. an inline section that fails
//!   to advance the wrap cursor).
//! - **Galley rect within render area.** Every galley's on-screen rect lies
//!   inside the render area `[0, 0] – [width, reported_height]`. Catches a
//!   renderer that paints past the height it reports (clipping, scroll
//!   miscalculations) or past the width it was given (horizontal overflow).
//!
//! # Test structure
//!
//! Markdown strings come from two generators:
//! - `markdown_doc` — structured mix of paragraphs, headings, lists,
//!   emphasis, code spans (used by the semantic properties).
//! - `fuzz_text` — random-UTF-8 byte stream (used by no-panic).
//!
//! On failure the buffer is delta-debugged and the shrunken input printed.

use comrak::Arena;
use egui::{Pos2, RawInput, Rect, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::DocCharOffset;

use super::MdRender;
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::shrink::shrink;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt};
use rand::{Rng, SeedableRng, rngs::StdRng};

const WORDS: [&str; 6] = ["foo", "bar", "baz", "qux", "hello", "world"];
const SHORTCODES: [&str; 4] = ["smile", "heart", "thumbsup", "rocket"];
const LANGS: [&str; 4] = ["", "rust", "python", "text"];

/// Maximum depth of block-container nesting in a generated document. Two
/// levels lets us produce things like `> - paragraph` and `> 1. > nested`
/// without runaway recursion.
const MAX_BLOCK_DEPTH: usize = 2;

/// Builds a minimally-configured `MdRender` suitable for test calls. Mirrors
/// the setup `MdLabel` expects from its caller (theme + font system).
fn test_renderer(md: &str) -> MdRender {
    let r = MdRender::test(md);
    r.ctx.set_lb_theme(Theme::default(Mode::Dark));
    crate::register_font_system(&r.ctx);
    r
}

/// Returns the rendered height of `md` at `width`, using a fresh renderer.
fn render_height(md: &str, width: f32) -> f32 {
    let mut r = test_renderer(md);
    r.width = width;
    let arena = Arena::new();
    let root = r.reparse(&arena);
    r.height(root, &[root])
}

struct GalleySnapshot {
    range: (DocCharOffset, DocCharOffset),
    rect: Rect,
    is_override: bool,
}

/// Paints `md` at `width` and returns the render area together with a
/// snapshot of each rendered galley.
fn render_galleys(md: &str, width: f32) -> (Rect, Vec<GalleySnapshot>) {
    let mut r = test_renderer(md);
    let ctx = r.ctx.clone();
    let mut snap = Vec::new();
    let mut area = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 0.));
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            r.dark_mode = ui.style().visuals.dark_mode;
            r.width = width;
            let arena = Arena::new();
            let root = r.reparse(&arena);
            let height = r.height(root, &[root]);
            r.top_left = Pos2::ZERO;
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            area = rect;
            r.galleys.galleys.clear();
            r.bounds.wrap_lines.clear();
            r.text_areas.clear();
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                r.show_block(ui, root, Pos2::ZERO, &[root]);
            });
            snap = r
                .galleys
                .galleys
                .iter()
                .map(|g| GalleySnapshot {
                    range: g.range,
                    rect: g.rect,
                    is_override: g.is_override,
                })
                .collect();
        });
    });
    (area, snap)
}

/// Picks one inline token. Covers comrak's enabled inline extensions: emphasis
/// (italic, both delimiters), strong, underline (`__`), code, strikethrough,
/// highlight, spoiler, subscript, superscript, math, shortcode, links,
/// autolinks, wikilinks, images. Plain text is heavily weighted so generated
/// content reads like prose.
fn inline_token(src: &mut ByteSource) -> String {
    let w = WORDS[src.draw(WORDS.len())];
    let weights = &[
        20, // plain
        2, 2, 2, 2, // *_ ** italic / strong
        2, // __ underline
        1, // *** italic+strong
        2, 1, 1, 1, 1, 1, 1, // code, strike, highlight, spoiler, sub, sup, math
        1, // shortcode
        2, 1, 1, 1, // link, image, autolink, wikilink
    ];
    match src.bias(weights) {
        0 => w.to_string(),
        1 => format!("*{w}*"),
        2 => format!("_{w}_"),
        3 => format!("**{w}**"),
        4 => format!("***{w}***"),
        5 => format!("__{w}__"),
        6 => format!("***{w}***"),
        7 => format!("`{w}`"),
        8 => format!("~~{w}~~"),
        9 => format!("=={w}=="),
        10 => format!("||{w}||"),
        11 => format!("~{w}~"),
        12 => format!("^{w}^"),
        13 => format!("${w}$"),
        14 => format!(":{}:", SHORTCODES[src.draw(SHORTCODES.len())]),
        15 => format!("[{w}](https://x.test)"),
        16 => format!("![{w}](https://x.test/i.png)"),
        17 => "<https://x.test>".to_string(),
        18 => format!("[[{w}]]"),
        _ => unreachable!(),
    }
}

/// A space-separated sequence of inline tokens. Used by every block that
/// accepts inline content (paragraphs, headings, table cells, etc).
fn inline_seq(src: &mut ByteSource) -> String {
    let n = 1 + src.bias(&[2, 3, 3, 2, 1]);
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&inline_token(src));
    }
    out
}

/// Prefixes every line of `s` with `prefix`. Empty lines get the prefix
/// trimmed of trailing whitespace (so a "> " prefix becomes ">" on a blank
/// line, which keeps a block quote open without inserting trailing spaces).
fn prefix_lines(s: &str, prefix: &str) -> String {
    let trimmed = prefix.trim_end();
    s.lines()
        .map(|line| if line.is_empty() { trimmed.to_string() } else { format!("{prefix}{line}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Indents continuation lines (every line after the first) by `indent`.
/// Used for list-item content where the marker stands on the first line and
/// subsequent lines align to the marker's content column.
fn indent_continuation(s: &str, indent: &str) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(indent);
            }
        }
        out.push_str(line);
    }
    out
}

/// Top-level document. Optionally prepends front matter, then a sequence of
/// blocks at the maximum allowed nesting depth.
fn markdown_doc(src: &mut ByteSource) -> String {
    let mut out = String::new();
    if src.bias(&[6, 1]) == 1 {
        out.push_str("---\ntitle: ");
        out.push_str(&inline_seq(src));
        out.push_str("\n---\n\n");
    }
    out.push_str(&block_seq(src, MAX_BLOCK_DEPTH));
    out
}

/// 1–8 blocks separated by blank lines.
fn block_seq(src: &mut ByteSource, depth: usize) -> String {
    let n = 1 + src.bias(&[2, 3, 4, 4, 3, 2, 2, 1]);
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&block(src, depth));
    }
    out
}

/// One block. At `depth == 0`, only leaves; otherwise leaves dominate but
/// containers (which recurse with `depth - 1`) are mixed in.
fn block(src: &mut ByteSource, depth: usize) -> String {
    if depth == 0 || src.bias(&[7, 3]) == 0 {
        leaf_block(src)
    } else {
        container_block(src, depth - 1)
    }
}

/// Leaf blocks: no nested blocks. Inline-accepting leaves use `inline_seq`;
/// raw leaves (code, thematic break) emit literal syntax.
fn leaf_block(src: &mut ByteSource) -> String {
    match src.bias(&[5, 4, 4, 2, 1, 2, 1]) {
        0 => format!("{}\n", inline_seq(src)), // paragraph
        1 => {
            // ATX heading 1–6
            let level = 1 + src.draw(6);
            let hashes = "#".repeat(level);
            format!("{hashes} {}\n", inline_seq(src))
        }
        2 => {
            // fenced code block
            let lang = LANGS[src.draw(LANGS.len())];
            let body = inline_seq(src); // raw — backticks etc render literally
            format!("```{lang}\n{body}\n```\n")
        }
        3 => "---\n".to_string(), // thematic break
        4 => {
            // table with alignment row
            let h1 = inline_seq(src);
            let h2 = inline_seq(src);
            let r1 = inline_seq(src);
            let r2 = inline_seq(src);
            format!("| {h1} | {h2} |\n| :--- | ---: |\n| {r1} | {r2} |\n")
        }
        5 => {
            // indented code block (4-space indent)
            let body = inline_seq(src);
            format!("    {body}\n")
        }
        6 => {
            // HTML block (raw)
            format!("<div>{}</div>\n", inline_seq(src))
        }
        _ => unreachable!(),
    }
}

/// Container blocks: nest a `block_seq` inside, prefixing each line with the
/// container's syntax.
fn container_block(src: &mut ByteSource, depth: usize) -> String {
    match src.bias(&[5, 4, 2, 1]) {
        0 => {
            // block quote
            let inner = block_seq(src, depth);
            prefix_lines(&inner, "> ") + "\n"
        }
        1 => {
            // bullet or ordered list with 1–3 items
            let n = 1 + src.bias(&[2, 3, 2]);
            let style = src.bias(&[6, 2, 2, 3, 1]);
            let mut out = String::new();
            for i in 0..n {
                let (marker, indent) = match style {
                    0 => ("- ".to_string(), "  ".to_string()),
                    1 => ("* ".to_string(), "  ".to_string()),
                    2 => ("+ ".to_string(), "  ".to_string()),
                    3 => (format!("{}. ", i + 1), "   ".to_string()),
                    _ => (format!("{}) ", i + 1), "   ".to_string()),
                };
                let inner = block_seq(src, depth);
                let body = indent_continuation(&inner, &indent);
                out.push_str(&format!("{marker}{body}\n"));
            }
            out
        }
        2 => {
            // task list
            let n = 1 + src.draw(3);
            let mut out = String::new();
            for _ in 0..n {
                let mark = if src.bias(&[1, 1]) == 0 { ' ' } else { 'x' };
                out.push_str(&format!("- [{mark}] {}\n", inline_seq(src)));
            }
            out
        }
        3 => {
            // GFM alert (special block quote)
            let kind = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"][src.draw(5)];
            let inner = block_seq(src, depth);
            format!("> [!{kind}]\n{}\n", prefix_lines(&inner, "> "))
        }
        _ => unreachable!(),
    }
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

/// Property: widening the viewport cannot increase rendered height.
fn width_monotonic_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = markdown_doc(&mut src);
    let widths = [200.0, 400.0, 800.0, 1600.0];

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
fn galley_disjoint_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let md = markdown_doc(&mut src);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    let mut ranges: Vec<_> = render_galleys(&md, width)
        .1
        .into_iter()
        .filter(|g| !g.is_override && g.range.0 < g.range.1)
        .map(|g| g.range)
        .collect();
    ranges.sort_by_key(|r| r.0);
    for w in ranges.windows(2) {
        let (_, a_end) = w[0];
        let (b_start, _) = w[1];
        if a_end > b_start {
            return Err("two galleys cover overlapping source bytes");
        }
    }
    Ok(())
}

/// Property: no two galley rects overlap on screen.
fn galley_rect_disjoint_check(buf: &[u8]) -> Result<(), &'static str> {
    const EPS: f32 = 0.01;
    let mut src = ByteSource::new(buf);
    let md = markdown_doc(&mut src);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    let rects: Vec<Rect> = render_galleys(&md, width)
        .1
        .into_iter()
        .map(|g| g.rect)
        .filter(|r| r.width() > EPS && r.height() > EPS)
        .collect();
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let inter = rects[i].intersect(rects[j]);
            if inter.width() > EPS && inter.height() > EPS {
                return Err("two galley rects overlap on screen");
            }
        }
    }
    Ok(())
}

/// Property: every galley rect lies within the render area.
fn galley_rect_in_render_area_check(buf: &[u8]) -> Result<(), &'static str> {
    const EPS: f32 = 0.01;
    let mut src = ByteSource::new(buf);
    let md = markdown_doc(&mut src);
    let width = 250.0 + (src.draw(8) as f32) * 100.0;
    let (area, snaps) = render_galleys(&md, width);
    for g in snaps {
        if g.rect.left() < area.left() - EPS
            || g.rect.top() < area.top() - EPS
            || g.rect.right() > area.right() + EPS
            || g.rect.bottom() > area.bottom() + EPS
        {
            return Err("galley rect strays outside render area");
        }
    }
    Ok(())
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

fn run(check: fn(&[u8]) -> Result<(), &'static str>, gen: fn(&mut ByteSource) -> String) {
    for seed in 0..2048u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; 128];
        rng.fill(&mut buf[..]);
        if let Err(reason) = check(&buf) {
            let shrunk = shrink(buf, |b| check(b).is_err());
            let mut src = ByteSource::new(&shrunk);
            let md = gen(&mut src);
            panic!(
                "seed {seed} {reason}\nshrunk ({} bytes): {shrunk:?}\n\
                 markdown ({} chars, {} bytes, debug-escaped):\n{md:?}\n\
                 markdown (literal):\n{md}",
                shrunk.len(),
                md.chars().count(),
                md.len(),
            );
        }
    }
}

#[test]
fn width_monotonic() {
    run(width_monotonic_check, markdown_doc);
}

#[test]
fn no_panic() {
    run(no_panic_check, fuzz_text);
}

#[test]
fn galley_disjoint() {
    run(galley_disjoint_check, markdown_doc);
}

#[test]
fn galley_rect_disjoint() {
    run(galley_rect_disjoint_check, markdown_doc);
}

#[test]
fn galley_rect_in_render_area() {
    run(galley_rect_in_render_area_check, markdown_doc);
}
