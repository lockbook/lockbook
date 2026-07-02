use egui::{Pos2, Rect, Stroke, Ui, Vec2};

use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::TextBufferArea;
use crate::tab::markdown_editor::MdRender;
use crate::widgets::glyphon_cache::{GlyphonCache, GlyphonCacheKey, GlyphonFontFamily};

pub trait BufferExt {
    fn shaped_size(&self, ppi: f32) -> Vec2;
    fn shaped_left(&self, ppi: f32) -> f32;
}

impl BufferExt for glyphon::Buffer {
    fn shaped_size(&self, ppi: f32) -> Vec2 {
        // Visual extent = max(g.x + g.w) - min(g.x) across glyphs. For LTR
        // runs glyphs span [0, V] so this equals V; for RTL runs cosmic-text
        // right-aligns to the buffer width, so glyphs span [W-V, W] and the
        // raw `last.x + last.w` would track buffer width rather than the
        // text's own extent.
        let mut result = Vec2::ZERO;
        for run in self.layout_runs() {
            result.y += self.metrics().line_height;
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            for g in run.glyphs.iter() {
                min_x = min_x.min(g.x);
                max_x = max_x.max(g.x + g.w);
            }
            if min_x.is_finite() {
                result.x = result.x.max(max_x - min_x);
            }
        }
        result / ppi
    }

    /// Smallest glyph x in the buffer. Zero for LTR; positive for an
    /// RTL-paragraph run (cosmic-text right-aligns it to the buffer
    /// width). Subtract from the painting `left` so glyphs land within
    /// `[pos.x, pos.x + shaped_size.x]` instead of at their raw
    /// buffer-relative positions.
    fn shaped_left(&self, ppi: f32) -> f32 {
        let mut min_x = f32::INFINITY;
        for run in self.layout_runs() {
            for g in run.glyphs.iter() {
                min_x = min_x.min(g.x);
            }
        }
        if min_x.is_finite() { min_x / ppi } else { 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Sans,
    Mono,
    Icons,
}

#[derive(Clone, Debug)]
pub struct Format {
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    pub color: egui::Color32,

    pub underline: bool,
    pub strikethrough: bool,
    pub background: egui::Color32,
    pub border: egui::Color32,
    pub spoiler: bool,
    pub superscript: bool,
    pub subscript: bool,
}

// ═════════════════════════════════════════════════════════════════════
// Wrap-layout pipeline:
//   walker           → Layout { visible, format_runs, source_segments,
//                               style_events }
//   shape_to_items   → Vec<InlineItem>  (Box | Glue | Pad | StyleOpen
//                                        | StyleClose | Break)
//   greedy_break     → Vec<row break index>
//   build_rows       → Vec<Row { fragments, anchors, source_range, … }>
//   show_wrap_layout → paints fragments; mirrors them onto
//                      MdRender::fragments for cursor / hit-test
//
// Invariants: layout-coords == paint-coords, one fragment per
// InlineItem (no coalescing), smart-corner bg fixup is paint-time
// only. Tabs use pixel-stops resolved at walker emit time — paint
// reshapes against a tab-substituted text so cosmic-text never sees `\t`.
// ═════════════════════════════════════════════════════════════════════

/// One item in the inline stream consumed by the breaker.
/// `Box` and `Glue` advance and (for `Glue`) admit breaks; `Pad`
/// advances without a break opportunity; `StyleOpen`/`StyleClose`
/// pass through the breaker and feed `build_rows`.
#[allow(dead_code)] // `Pad` not constructed until walkers are migrated
#[derive(Clone, Debug)]
pub enum InlineItem {
    /// A shaped non-blank chunk. `buffer` is the pre-shaped glyph
    /// data the walker produced; paint reuses it directly. `advance`
    /// is the buffer's painted width (typically) or, for tabs, the
    /// walker-overridden pixel-stop advance. `cluster_advances` is
    /// per-source-grapheme along `source_range`, derived from the
    /// buffer at shape time. `visible_byte_range` is informational —
    /// used by the row builder to place zero-visible anchors.
    Box {
        advance: f32,
        source_range: (Grapheme, Grapheme),
        visible_byte_range: std::ops::Range<u32>,
        buffer: std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>,
        cluster_advances: Vec<f32>,
        /// `false` when visible bytes map 1:1 to source bytes
        /// (normal text); `true` for override-text spans. Forwarded
        /// to `Fragment::atomic`.
        atomic: bool,
    },
    /// Flexible whitespace; the breaker's break opportunity. Same
    /// shape as `Box` but `natural` is its advance.
    Glue {
        natural: f32,
        source_range: (Grapheme, Grapheme),
        visible_byte_range: std::ops::Range<u32>,
        buffer: std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>,
        cluster_advances: Vec<f32>,
    },
    /// Rigid advance with no glyphs and no break opportunity.
    /// Emitted around backgrounded-inline-scope boundaries so
    /// `Fragment::rect` includes the bg breathing room. `source_pos`
    /// is where a click on the pad should place the cursor — the
    /// scope's start for a leading pad, its end for a trailing pad.
    Pad {
        advance: f32,
        source_pos: Grapheme,
    },
    /// Forced row break. Emitted by `<br>` (line_break) and bare
    /// newlines (soft_break). Has a source range for cursor
    /// positioning; treated by the row builder like a wrap-break-
    /// glue (zero-width fragment, excluded from row source_range).
    Break {
        source_range: (Grapheme, Grapheme),
        visible_byte_range: std::ops::Range<u32>,
    },
    /// Inline embedded content (currently images). Atomic; breaker
    /// treats it like a `Box`.
    Image(ImageSpec),
    /// Open an inline-box style scope.
    StyleOpen(StyleInfo),
    /// Close the most recent open. AST nesting guarantees LIFO.
    StyleClose,
    /// Tags subsequent fragments until `InteractionClose`.
    InteractionOpen(egui::Id, egui::Sense),
    InteractionClose,
}

/// Inline image's painted box and source span. `advance` is the box
/// width; the box sits `ascent` above and `descent` below the row's
/// text baseline. `source_range` covers the full `![alt](url)` syntax.
#[derive(Clone, Debug)]
pub struct ImageSpec {
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub source_range: (Grapheme, Grapheme),
    pub url: String,
}

/// Style record for one inline-box instance. Snapshotted into each
/// `Fragment::style_stack` at emit time. Sense routing reads the
/// innermost entry's `source_range` to identify the AST node.
#[derive(Clone, Debug)]
pub struct StyleInfo {
    pub format: Format,
    /// The inline-box's full source span (e.g. for `**bold**`, the
    /// whole `0..8` range). Identifies the AST node at click time
    /// without storing a pointer.
    pub source_range: (Grapheme, Grapheme),
    /// Paint the background as a compact capsule hugging the text
    /// middle (the fold `···` chip) instead of filling the row.
    pub chip: bool,
}

impl StyleInfo {
    pub fn new(format: Format, source_range: (Grapheme, Grapheme)) -> Self {
        Self { format, source_range, chip: false }
    }
}

/// A strike/underline rule, painted on top of text (see `deco_lines`).
#[derive(Clone, Debug)]
pub struct DecoLine {
    pub x: std::ops::RangeInclusive<f32>,
    pub y: f32,
    pub color: egui::Color32,
}

/// One visual row of a wrap unit, emitted by `build_rows`.
#[derive(Clone, Debug)]
pub struct Row {
    /// Y of the row's top relative to the wrap unit's top-left.
    pub y_top: f32,
    /// Tallest ascent + descent in the row; row height.
    pub ascent: f32,
    pub descent: f32,
    /// Concatenated source range of this row's content. Used to
    /// populate `bounds.wrap_lines` for `Bound::Line` navigation.
    /// Wrap-break-glue source range is **excluded** so cmd+right
    /// from row interior lands at the row's last visible offset,
    /// not at the shared boundary with the next row.
    pub source_range: (Grapheme, Grapheme),
    /// Per-(InlineItem × row) fragments. One per Box, Glue, Pad
    /// that landed in this row's break window. Style scope events
    /// don't emit fragments; they update the running stack between
    /// fragments.
    pub fragments: Vec<Fragment>,
    /// Cursor-only positions for source bytes with no glyph
    /// representation (folded HTML inline alongside visible
    /// content, thematic-break interiors). Consumed by cursor /
    /// hit-test code; not painted.
    pub anchors: Vec<Anchor>,
}

/// Output of laying out one wrap unit. Coordinates are relative to
/// the wrap unit's top-left.
#[derive(Clone, Debug)]
pub struct WrapUnitLayout {
    pub source_range: (Grapheme, Grapheme),
    pub rows: Vec<Row>,
    pub height: f32,
    pub width: f32,
    /// Row height used for shaping. Headings override the default.
    /// Per-fragment paint Buffer must shape at this metric or
    /// painted glyphs will sit at a different scale from the
    /// fragment rects.
    pub row_height: f32,
}

/// Per-side padding between a fragment's outer rect and its glyph
/// content. Under no-coalescing, vertical inset matches `inline_pad`
/// for backgrounded fragments and zero otherwise; horizontal inset
/// is zero (any Pad lives in its own standalone Pad fragment).
#[derive(Clone, Copy, Debug, Default)]
pub struct FragmentInset {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// One painted rectangle. The output of layout that paint, cursor
/// math, and hit-testing all consume.
///
/// `rect` is the fragment's **full visual extent** — paint covers
/// exactly `rect` (bg + glyphs + decorations), nothing extends past
/// it. Glyphs sit inset within `rect` by `content_inset`. See the
/// design doc for why layout-coords == paint-coords is load-bearing.
#[derive(Clone, Debug)]
pub struct Fragment {
    pub rect: Rect,
    pub content_inset: FragmentInset,
    pub source_range: (Grapheme, Grapheme),
    /// Style stack at emit time, outermost first. Innermost
    /// (`.last()`) carries the format that applies to this
    /// fragment's glyphs and identifies the AST node for sense
    /// routing.
    pub style_stack: Vec<StyleInfo>,
    /// What to paint at `rect`.
    pub content: FragmentContent,
    /// Hit-test atomicity (per design Q4): clicks inside an atomic
    /// fragment snap to `source_range.start` / `source_range.end`,
    /// and `fragment_x` for an offset uses the midpoint rule.
    /// Char-arrow ignores this flag. Always `false` for non-override
    /// text; set from `SourceSegment::one_to_one == false` for
    /// override segments.
    pub atomic: bool,
    /// Id salt + sense for `interact_fragments`. Innermost open
    /// scope wins; `None` means no per-fragment interact.
    pub interaction: Option<(egui::Id, egui::Sense)>,
}

/// What a fragment renders.
#[derive(Clone, Debug)]
pub enum FragmentContent {
    /// Pre-shaped text. The walker shaped this Buffer once when it
    /// emitted the inline item; paint reuses it as a
    /// `TextBufferArea` (no re-shape). `cluster_advances` is per-
    /// source-grapheme along `source_range`, derived from the
    /// buffer at shape time so cursor math doesn't re-read it.
    Glyphs {
        buffer: std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>,
        cluster_advances: Vec<f32>,
    },
    /// No glyphs; the rect just paints bg from `style_stack`.
    /// Standalone Pad / wrap-break-glue / Break / Anchor fragments.
    Spacer,
    /// Embedded image; paint via `MdRender::embeds.show(ui, &url, rect)`.
    Image { url: String },
}

/// Zero-width cursor position for source bytes with no painted
/// representation. Carries source range + x in the row; consumed by
/// cursor / hit-test code only.
#[derive(Clone, Debug)]
pub struct Anchor {
    pub source_range: (Grapheme, Grapheme),
    pub x: f32,
    pub y_top: f32,
    pub height: f32,
}

/// Walker output. Accumulates the wrap unit's visible text + format
/// runs + source-byte mapping + style events. Consumed by
/// `compute_layout_from`.
pub struct Layout {
    visible: String,
    /// `(byte_start, byte_end, Format)` runs over `visible`. Non-
    /// overlapping, in order; concatenation covers all of `visible`
    /// that the walker tagged. (Empty stretches inherit the default
    /// `Attrs` at shape time.)
    format_runs: Vec<(usize, usize, Format)>,
    /// Each visible-byte range and the source range it came from.
    /// `one_to_one == true` ⇒ visible bytes equal source bytes
    /// (slicing into the buffer gives the same text); `false` ⇒
    /// override (footnote ref, alert title, shortcode emoji). One
    /// segment per `push_*` call.
    source_segments: Vec<SourceSegment>,
    /// `(visible_byte_pos, event)` — flow events at inline-box
    /// boundaries (style open/close) and force-break points (soft
    /// break, hard break). Merged with shaped words by byte position
    /// in `shape_to_items` to form the final InlineItem stream.
    events: Vec<(usize, FlowEvent)>,
    /// Source range this wrap unit covers (informational; used to
    /// stamp the resulting `WrapUnitLayout::source_range`).
    source_range: (Grapheme, Grapheme),
    /// Byte positions in `visible` of source tabs, in order. Paint
    /// substitutes these to spaces too — cosmic-text never sees `\t`.
    /// Walker computes pixel-stop advance for each at shape time.
    tab_positions: Vec<usize>,
}

#[derive(Clone, Debug)]
struct SourceSegment {
    visible: std::ops::Range<usize>,
    source: (Grapheme, Grapheme),
    one_to_one: bool,
}

#[derive(Clone, Debug)]
enum FlowEvent {
    Open(StyleInfo),
    Close,
    /// Forced row break (soft break, hard break) at a source range.
    /// Translates to an `InlineItem::Break` at this point in the
    /// shaped stream.
    Break((Grapheme, Grapheme)),
    /// Embedded image; translates 1:1 to `InlineItem::Image`.
    Image(ImageSpec),
    InteractionOpen(egui::Id, egui::Sense),
    InteractionClose,
}

impl Layout {
    pub fn new(source_range: (Grapheme, Grapheme)) -> Self {
        Self {
            visible: String::new(),
            format_runs: Vec::new(),
            source_segments: Vec::new(),
            events: Vec::new(),
            source_range,
            tab_positions: Vec::new(),
        }
    }

    pub fn source_range(&self) -> (Grapheme, Grapheme) {
        self.source_range
    }

    /// Append a span of source text. Caller passes the slice
    /// (the module doesn't depend on `Buffer`) and asserts the
    /// slice equals `buffer[source]` so `one_to_one` mapping holds.
    /// Newlines are sanitised to spaces (cosmic-text would split a
    /// `Buffer` on BiDi class-B chars otherwise) and tabs are noted
    /// in `tab_positions` for pixel-stop resolution at shape time.
    pub fn push_source(&mut self, source: (Grapheme, Grapheme), text: &str, format: Format) {
        if text.is_empty() {
            return;
        }
        let start = self.visible.len();
        push_sanitised(&mut self.visible, &mut self.tab_positions, text);
        let end = self.visible.len();
        self.source_segments
            .push(SourceSegment { visible: start..end, source, one_to_one: true });
        self.format_runs.push((start, end, format));
    }

    /// Append an override-text span. Visible bytes differ from
    /// source bytes; cursor mapping (atomic hit-test, char-arrow
    /// over source bytes) is driven by the `one_to_one: false` flag
    /// on the segment.
    ///
    /// Empty `text` records a zero-visible segment used as an
    /// anchor for content folded to nothing (e.g. an `<!-- fold -->`
    /// HTML inline marker rendering as blank). Row builder places
    /// an `Anchor` at the segment's visible position; no fragment.
    pub fn push_override(&mut self, source: (Grapheme, Grapheme), text: &str, format: Format) {
        let start = self.visible.len();
        push_sanitised(&mut self.visible, &mut self.tab_positions, text);
        let end = self.visible.len();
        self.source_segments
            .push(SourceSegment { visible: start..end, source, one_to_one: false });
        if start < end {
            self.format_runs.push((start, end, format));
        }
    }

    /// Open an inline-box scope. Pairs LIFO with `style_close`. The
    /// row builder uses these to attribute styles per fragment.
    pub fn style_open(&mut self, style: StyleInfo) {
        self.events
            .push((self.visible.len(), FlowEvent::Open(style)));
    }

    pub fn style_close(&mut self) {
        self.events.push((self.visible.len(), FlowEvent::Close));
    }

    /// Emit a forced row break (soft break, hard break) at the
    /// current position. Translates to an `InlineItem::Break`
    /// covering `source_range`.
    pub fn push_break(&mut self, source_range: (Grapheme, Grapheme)) {
        self.events
            .push((self.visible.len(), FlowEvent::Break(source_range)));
    }

    /// Emit a pre-sized inline image at the current position.
    pub fn push_image(&mut self, spec: ImageSpec) {
        self.events
            .push((self.visible.len(), FlowEvent::Image(spec)));
    }

    /// Open an interaction scope; fragments emitted before the matching
    /// `interaction_close` carry `(id, sense)`.
    pub fn interaction_open(&mut self, id: egui::Id, sense: egui::Sense) {
        self.events
            .push((self.visible.len(), FlowEvent::InteractionOpen(id, sense)));
    }

    pub fn interaction_close(&mut self) {
        self.events
            .push((self.visible.len(), FlowEvent::InteractionClose));
    }
}

/// Sanitise BiDi paragraph-separator chars (LF, CR, FS, GS, RS,
/// NEL, PS) to ASCII spaces, byte-for-byte. cosmic-text starts a
/// new `BufferLine` at any class-B char; a wrap unit's text must
/// stay one buffer-line tall. Tab positions are recorded for
/// pixel-stop resolution.
fn push_sanitised(out: &mut String, tabs: &mut Vec<usize>, text: &str) {
    for c in text.chars() {
        let pos = out.len();
        match c {
            '\t' => {
                tabs.push(pos);
                out.push(' ');
            }
            '\n' | '\r' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2029}' => {
                // Replace with the same number of bytes of space so
                // byte offsets in `visible` still correspond to
                // source byte offsets for one_to_one segments.
                for _ in 0..c.len_utf8() {
                    out.push(' ');
                }
            }
            _ => out.push(c),
        }
    }
}

impl MdRender {
    /// Run the post-walker pipeline (`shape → break → build_rows`)
    /// on a populated `Layout`. The public entry point until the
    /// AST-driven `compute_wrap_layout` is wired in (next migration
    /// step).
    pub fn compute_layout_from(
        &self, layout: Layout, width: f32, row_height: f32,
    ) -> WrapUnitLayout {
        let row_spacing = self.layout.row_spacing;
        let inline_pad = self.layout.inline_padding;
        let source_range = layout.source_range();
        let items =
            shape_to_items(self, &layout, row_height, width, inline_pad).unwrap_or_default();
        let rows = if items.is_empty() {
            // Empty unit (no items): emit one empty row so the wrap
            // unit still takes vertical space and contributes a
            // `wrap_lines` entry. Anchors from zero-visible source
            // segments are attached to this row; if no anchors
            // exist (the unit is genuinely empty — blank line
            // between paragraphs, etc.), emit one at the source
            // range start so cursor lookup at that offset resolves.
            let ascent = row_height * 0.8;
            let descent = row_height * 0.2;
            let mut anchors: Vec<Anchor> = Vec::new();
            for seg in &layout.source_segments {
                if seg.visible.start == seg.visible.end {
                    anchors.push(Anchor {
                        source_range: seg.source,
                        x: 0.0,
                        y_top: 0.0,
                        height: ascent + descent,
                    });
                }
            }
            if anchors.is_empty() {
                anchors.push(Anchor { source_range, x: 0.0, y_top: 0.0, height: ascent + descent });
            }
            vec![Row { y_top: 0.0, ascent, descent, source_range, fragments: Vec::new(), anchors }]
        } else {
            let breaks = greedy_break(&items, width, inline_pad);
            build_rows(&breaks, &items, &layout, row_height, row_spacing, inline_pad)
        };
        // Wrap unit height = stacked row glyph rects, no extra padding
        // for bg-styled rows. Per design, inline backgrounds contribute
        // to layout only via side (left/right) padding; vertical bg
        // breathing room is drawn outside the fragment rect at paint
        // time (fitting into `row_spacing`, which is `2 × inline_pad`).
        let height = if rows.is_empty() {
            row_height
        } else {
            let last = rows.last().unwrap();
            last.y_top + last.ascent + last.descent
        };
        WrapUnitLayout { source_range, rows, height, width, row_height }
    }

    /// Convenience: lay out a single source range with one format
    /// (no style brackets, no overrides, no embeds).
    pub fn compute_section_layout_new(
        &self, range: (Grapheme, Grapheme), width: f32, row_height: f32, format: Format,
    ) -> WrapUnitLayout {
        let mut l = Layout::new(range);
        l.push_source(range, &self.buffer[range], format);
        self.compute_layout_from(l, width, row_height)
    }

    /// Convenience: lay out a single override range (visible text
    /// differs from source bytes).
    pub fn compute_override_section_layout_new(
        &self, source: (Grapheme, Grapheme), text: &str, width: f32, row_height: f32,
        format: Format,
    ) -> WrapUnitLayout {
        let mut l = Layout::new(source);
        l.push_override(source, text, format);
        self.compute_layout_from(l, width, row_height)
    }
}

// ─── shape (Layout → Vec<InlineItem>) ────────────────────────────────

/// Side padding inside a chip capsule, as a fraction of row height.
/// Shared by the walker (which lays the pads between the glyph and
/// the capsule edges) and by cursor math (which backs them out so the
/// caret beside the atom renders beside the capsule).
const CHIP_SIDE_PAD: f32 = 0.3;

/// Tab pixel-stop interval. Walker resolves tab advance from running
/// x within the wrap unit (`ceil(x/stop) * stop - x`). Matches the
/// monospace 4-character convention pixel-for-pixel in code blocks
/// since `em ≈ row_height` for our metrics; defined for proportional
/// contexts too.
fn tab_stop_distance(row_height: f32) -> f32 {
    4.0 * row_height
}

/// Build a `Vec<InlineItem>` from a populated `Layout` by:
///
///  1. computing chunk boundaries in `Layout::visible` — the union
///     of unicode-linebreak break opportunities, format-run edges,
///     and flow-event positions;
///  2. for each `[chunk_lo, chunk_hi)` range, shaping its text into
///     its own glyphon `Buffer` with the right attrs (one buffer
///     per future fragment), classifying as `Box` (non-blank) or
///     `Glue` (blank), and reading the painted advance back from
///     that buffer's `layout_runs()`;
///  3. interspersing flow events (`StyleOpen` / `StyleClose` /
///     `Break`) at their positions.
///
/// Paint reuses the buffer the walker built — no second shape pass.
/// Walker advance == paint advance by construction.
///
/// Returns `None` on font-system unavailability or panic during
/// shape (caller falls back to an empty layout).
fn shape_to_items(
    renderer: &MdRender, layout: &Layout, row_height: f32, width: f32, inline_pad: f32,
) -> Option<Vec<InlineItem>> {
    use std::sync::{Arc, Mutex};

    // Track open bg-scopes' source ranges + side-pad widths, so the
    // matching close can emit the trailing `Pad` with the scope's end
    // as its source position. `None` entries hold place for non-bg
    // scopes (Pad not emitted, but stack depth must mirror
    // StyleOpen/Close). AST guarantees LIFO nesting.
    let mut bg_scope_stack: Vec<Option<((Grapheme, Grapheme), f32)>> = Vec::new();
    let emit_event = |items: &mut Vec<InlineItem>,
                      bg_scope_stack: &mut Vec<Option<((Grapheme, Grapheme), f32)>>,
                      pos: usize,
                      ev: &FlowEvent| {
        let p = pos as u32;
        match ev {
            FlowEvent::Open(s) => {
                let bg = s.format.background != egui::Color32::TRANSPARENT;
                items.push(InlineItem::StyleOpen(s.clone()));
                if bg {
                    // Chips pad wide enough for the glyphs to clear the
                    // capsule's curved ends.
                    let pad = if s.chip { row_height * CHIP_SIDE_PAD } else { inline_pad };
                    items
                        .push(InlineItem::Pad { advance: pad, source_pos: s.source_range.start() });
                    bg_scope_stack.push(Some((s.source_range, pad)));
                } else {
                    bg_scope_stack.push(None);
                }
            }
            FlowEvent::Close => {
                if let Some(Some((scope_range, pad))) = bg_scope_stack.pop() {
                    items.push(InlineItem::Pad { advance: pad, source_pos: scope_range.end() });
                }
                items.push(InlineItem::StyleClose);
            }
            FlowEvent::Image(spec) => {
                items.push(InlineItem::Image(spec.clone()));
            }
            FlowEvent::Break(r) => {
                items.push(InlineItem::Break { source_range: *r, visible_byte_range: p..p });
            }
            FlowEvent::InteractionOpen(id, sense) => {
                items.push(InlineItem::InteractionOpen(*id, *sense));
            }
            FlowEvent::InteractionClose => {
                items.push(InlineItem::InteractionClose);
            }
        }
    };

    if layout.visible.is_empty() {
        // Still emit flow events so the row stack reflects open/close
        // and any naked break carries through.
        let mut items = Vec::new();
        for (pos, ev) in &layout.events {
            emit_event(&mut items, &mut bg_scope_stack, *pos, ev);
        }
        return Some(items);
    }

    let fs: Arc<Mutex<glyphon::FontSystem>> = renderer
        .ctx
        .data(|d| d.get_temp::<Arc<Mutex<glyphon::FontSystem>>>(egui::Id::NULL))?;
    let cache: Arc<Mutex<GlyphonCache>> = renderer
        .ctx
        .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL))?;
    let ppi = renderer.ctx.pixels_per_point();

    // Build the set of byte positions where a chunk *must* end.
    // Sort and dedupe; iterate consecutive positions as chunks.
    let mut breaks: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    breaks.insert(0);
    breaks.insert(layout.visible.len());
    // unicode-linebreak break opportunities (UAX#14). Both Mandatory
    // and Allowed positions become chunk edges; we sanitised
    // mandatory chars to spaces in `Layout::visible` so Mandatory
    // shouldn't appear in practice, but treat them the same way.
    for (pos, _opp) in unicode_linebreak::linebreaks(&layout.visible) {
        breaks.insert(pos);
    }
    // Format-run boundaries. Different attrs ⇒ different buffer ⇒
    // separate chunk.
    for (s, e, _) in &layout.format_runs {
        breaks.insert(*s);
        breaks.insert(*e);
    }
    // Flow event positions.
    for (pos, _) in &layout.events {
        breaks.insert(*pos);
    }
    // Whitespace transitions — split Box from Glue, and split every
    // consecutive whitespace pair so the breaker can wrap inside a
    // run (otherwise trailing whitespace would either overshoot
    // `width` or vanish into wrap-break-glue).
    let bytes = layout.visible.as_bytes();
    for i in 1..bytes.len() {
        if !layout.visible.is_char_boundary(i) {
            continue;
        }
        let prev_ws = bytes[i - 1].is_ascii_whitespace();
        let cur_ws = bytes[i].is_ascii_whitespace();
        if prev_ws != cur_ws || (prev_ws && cur_ws) {
            breaks.insert(i);
        }
    }

    let break_points: Vec<usize> = breaks.into_iter().collect();
    let stop = tab_stop_distance(row_height);
    let tab_set: std::collections::HashSet<usize> = layout.tab_positions.iter().copied().collect();
    let mut items: Vec<InlineItem> = Vec::new();
    let mut ev_idx = 0usize;
    let mut running_x = 0.0f32;
    // Break opps land at chunk edges (UAX#14, format-run, event,
    // whitespace). The greedy breaker only breaks at `Glue`; emit a
    // zero-width Glue between adjacent non-blank chunks to expose the
    // break opportunity at the boundary.
    let mut prev_was_nonblank = false;

    for win in break_points.windows(2) {
        let chunk_lo = win[0];
        let chunk_hi = win[1];

        // Flush flow events at positions in [prev_chunk_end, chunk_lo].
        while ev_idx < layout.events.len() && layout.events[ev_idx].0 <= chunk_lo {
            let (pos, ev) = &layout.events[ev_idx];
            emit_event(&mut items, &mut bg_scope_stack, *pos, ev);
            ev_idx += 1;
        }

        if chunk_lo >= chunk_hi {
            continue;
        }
        let chunk_text = &layout.visible[chunk_lo..chunk_hi];
        let format = format_for_range(&layout.format_runs, chunk_lo, chunk_hi)
            .unwrap_or_else(default_format);
        let blank = chunk_text.chars().all(char::is_whitespace);

        // Shape this chunk into its own Buffer. The buffer is what
        // paint will use; the advance comes from the buffer's
        // measured glyph extent.
        let shape_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            shape_chunk(&fs, &cache, chunk_text, &format, row_height, width, ppi)
        }));
        let (buffer, measured_advance) = match shape_result {
            Ok(v) => v,
            Err(_) => {
                fs.clear_poison();
                continue;
            }
        };

        // Tabs: walker overrides advance to pixel-stop value. The
        // buffer's painted glyph (a substituted space) sits at the
        // left edge of the fragment; remaining space is empty.
        let advance = if chunk_text
            .char_indices()
            .any(|(i, _)| tab_set.contains(&(chunk_lo + i)))
        {
            let x = running_x;
            let mut a = (((x / stop).floor() + 1.0) * stop) - x;
            let min_advance = row_height * 0.5;
            if a < min_advance {
                a += stop;
            }
            a.max(measured_advance)
        } else {
            measured_advance
        };

        let source_range = source_range_for_visible(renderer, layout, chunk_lo, chunk_hi);
        let atomic = !is_one_to_one_range(layout, chunk_lo, chunk_hi);
        let cluster_advances = if atomic {
            Vec::new()
        } else {
            cluster_advances_from_buffer(renderer, layout, &buffer, chunk_lo, source_range, ppi)
        };

        let vis_range = (chunk_lo as u32)..(chunk_hi as u32);
        if blank {
            items.push(InlineItem::Glue {
                natural: advance,
                source_range,
                visible_byte_range: vis_range,
                buffer: buffer.clone(),
                cluster_advances,
            });
            prev_was_nonblank = false;
        } else {
            if prev_was_nonblank {
                let lo_u = chunk_lo as u32;
                items.push(InlineItem::Glue {
                    natural: 0.0,
                    source_range: (source_range.start(), source_range.start()),
                    visible_byte_range: lo_u..lo_u,
                    buffer: empty_buffer(&fs, row_height, ppi),
                    cluster_advances: Vec::new(),
                });
            }
            if advance > width
                && !atomic
                && source_range.end().0 > source_range.start().0 + 1
                && cluster_advances.len() == source_range.end().0 - source_range.start().0
            {
                emit_cluster_split_boxes(
                    renderer,
                    layout,
                    &fs,
                    &cache,
                    &mut items,
                    chunk_text,
                    chunk_lo,
                    source_range,
                    &format,
                    &cluster_advances,
                    row_height,
                    width,
                    ppi,
                );
            } else {
                items.push(InlineItem::Box {
                    advance,
                    source_range,
                    visible_byte_range: vis_range,
                    buffer: buffer.clone(),
                    cluster_advances,
                    atomic,
                });
            }
            prev_was_nonblank = true;
        }
        running_x += advance;
    }

    // Flush trailing flow events.
    while ev_idx < layout.events.len() {
        let (pos, ev) = &layout.events[ev_idx];
        emit_event(&mut items, &mut bg_scope_stack, *pos, ev);
        ev_idx += 1;
    }

    Some(items)
}

/// Shape a single chunk of text with a single `Format` into a glyphon
/// `Buffer`. Routes through `GlyphonCache` so repeat-shapes of the
/// same (text, format, width) get a cache hit and skip rustybuzz /
/// ttf_parser entirely. Returns the buffer wrapped in
/// `Arc<RwLock<_>>` (for sharing with paint) and the buffer's
/// painted-glyph extent in logical pixels.
fn shape_chunk(
    fs: &std::sync::Arc<std::sync::Mutex<glyphon::FontSystem>>,
    cache: &std::sync::Arc<std::sync::Mutex<GlyphonCache>>, text: &str, format: &Format,
    row_height: f32, width: f32, ppi: f32,
) -> (std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>, f32) {
    let metric = row_height * ppi;
    let w = (width * ppi).max(1.0);
    let key = shape_cache_key(text, format, metric, w);

    let buffer = {
        let format = format.clone();
        let fs = fs.clone();
        cache.lock().unwrap().get_or_shape(key, move || {
            let mut guard = fs.lock().unwrap();
            let mut b = glyphon::Buffer::new(&mut guard, glyphon::Metrics::new(metric, metric));
            b.set_size(&mut guard, Some(w), None);
            b.set_wrap(&mut guard, glyphon::Wrap::None);
            b.set_tab_width(&mut guard, 4);
            let attrs = format_to_attrs(&format, metric);
            // Per-grapheme: route emoji to Twemoji Mozilla so VS-16
            // (or SMP) codepoints render in color. Without this, e.g.
            // `:warning:` → `⚠️` shapes against SansSerif (SF Pro Text
            // on Mac) which has a monochrome `⚠` outline.
            let emoji_attrs = glyphon::AttrsOwned::new(
                &glyphon::Attrs::new().family(glyphon::Family::Name("Twemoji Mozilla")),
            );
            let text = text.to_string();
            let spans = text.graphemes(true).map(|g| {
                if shape_as_emoji(&format.family, g) {
                    (g, emoji_attrs.as_attrs())
                } else {
                    (g, attrs.as_attrs())
                }
            });
            b.set_rich_text(&mut guard, spans, &attrs.as_attrs(), glyphon::Shaping::Advanced, None);
            b
        })
    };

    let advance = {
        let guard = buffer.read().unwrap();
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        for run in guard.layout_runs() {
            for g in run.glyphs.iter() {
                let l = g.x;
                let r = g.x + g.w;
                if l < min_x {
                    min_x = l;
                }
                if r > max_x {
                    max_x = r;
                }
            }
        }
        if min_x.is_finite() && max_x > min_x { (max_x - min_x) / ppi } else { 0.0 }
    };
    (buffer, advance)
}

/// Build the `GlyphonCache` key for a single-span shape. Super/sub
/// scales the per-span metric to 0.75×, so we mirror that into the
/// key's `font_size_bits` to keep cache entries separate from same-
/// text non-super/sub shapes. `width_bits ^ 1` matches the
/// `Wrap::None` marker convention from `block::height_buffer`.
fn shape_cache_key(text: &str, format: &Format, metric: f32, width_px: f32) -> GlyphonCacheKey {
    let key_metric = if format.superscript || format.subscript { metric * 0.75 } else { metric };
    GlyphonCacheKey::single(
        text,
        match format.family {
            FontFamily::Sans => GlyphonFontFamily::SansSerif,
            FontFamily::Mono => GlyphonFontFamily::Monospace,
            FontFamily::Icons => GlyphonFontFamily::Named("Nerd Fonts Mono Symbols".into()),
        },
        format.bold,
        format.italic,
        Some(format.color.to_array()),
        key_metric.to_bits(),
        key_metric.to_bits(),
        width_px.to_bits() ^ 1,
    )
}

/// Per-source-grapheme advances read from a pre-shaped chunk
/// buffer. Walks the buffer's glyphs, maps each glyph's start byte
/// (in chunk-local coords) back to a source grapheme via
/// `layout.source_segments`, and sums each glyph's width into the
/// owning grapheme's slot.
fn cluster_advances_from_buffer(
    renderer: &MdRender, layout: &Layout,
    buffer: &std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>, chunk_lo: usize,
    source_range: (Grapheme, Grapheme), ppi: f32,
) -> Vec<f32> {
    let grapheme_count = source_range.end().0.saturating_sub(source_range.start().0);
    if grapheme_count == 0 {
        return Vec::new();
    }
    let segs = &renderer.buffer.current.segs;
    let mut advances = vec![0.0f32; grapheme_count];
    let buf = buffer.read().unwrap();
    for run in buf.layout_runs() {
        for g in run.glyphs.iter() {
            let abs_byte = chunk_lo + g.start;
            let mapped = layout.source_segments.iter().find_map(|seg| {
                if !seg.one_to_one || abs_byte < seg.visible.start || abs_byte >= seg.visible.end {
                    return None;
                }
                let seg_byte_start = segs.offset_to_byte(seg.source.start());
                let head_bytes = abs_byte - seg.visible.start;
                Some(segs.byte_to_char_floor(seg_byte_start + head_bytes))
            });
            let Some(grapheme) = mapped else { continue };
            let Some(idx) = grapheme.0.checked_sub(source_range.start().0) else { continue };
            if idx >= grapheme_count {
                continue;
            }
            advances[idx] += g.w / ppi;
        }
    }
    advances
}

/// Find the `Format` covering `[lo, hi)` in `format_runs`. Returns
/// the first run that overlaps; chunks straddle format boundaries
/// because we added those as chunk edges already.
fn format_for_range(
    format_runs: &[(usize, usize, Format)], lo: usize, hi: usize,
) -> Option<Format> {
    for (rs, re, fmt) in format_runs {
        if *re <= lo || *rs >= hi {
            continue;
        }
        return Some(fmt.clone());
    }
    None
}

fn default_format() -> Format {
    Format {
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
    }
}

/// Over-wide unbreakable token: split into per-grapheme sub-Boxes,
/// each shaped into its own buffer. Inserts a zero-width Glue
/// separator between consecutive sub-Boxes so the greedy breaker
/// can wrap inside the token.
#[allow(clippy::too_many_arguments)]
fn emit_cluster_split_boxes(
    renderer: &MdRender, layout: &Layout,
    fs: &std::sync::Arc<std::sync::Mutex<glyphon::FontSystem>>,
    cache: &std::sync::Arc<std::sync::Mutex<GlyphonCache>>, items: &mut Vec<InlineItem>,
    chunk_text: &str, chunk_lo: usize, source_range: (Grapheme, Grapheme), format: &Format,
    cluster_advances: &[f32], row_height: f32, width: f32, ppi: f32,
) {
    let cluster_count = cluster_advances.len();
    if cluster_count == 0 {
        return;
    }
    let segs = &renderer.buffer.current.segs;
    let mut cluster_byte_lo: Vec<usize> = vec![usize::MAX; cluster_count];
    let mut cluster_byte_hi: Vec<usize> = vec![0usize; cluster_count];
    for (chunk_byte, _ch) in chunk_text.char_indices() {
        let abs_byte = chunk_lo + chunk_byte;
        let mapped = layout.source_segments.iter().find_map(|seg| {
            if !seg.one_to_one || abs_byte < seg.visible.start || abs_byte >= seg.visible.end {
                return None;
            }
            let seg_byte_start = segs.offset_to_byte(seg.source.start());
            let head_bytes = abs_byte - seg.visible.start;
            Some(segs.byte_to_char_floor(seg_byte_start + head_bytes))
        });
        let Some(grapheme) = mapped else { continue };
        let Some(idx) = grapheme.0.checked_sub(source_range.start().0) else { continue };
        if idx >= cluster_count {
            continue;
        }
        if chunk_byte < cluster_byte_lo[idx] {
            cluster_byte_lo[idx] = chunk_byte;
        }
        let byte_end = chunk_byte + chunk_text[chunk_byte..].chars().next().unwrap().len_utf8();
        if byte_end > cluster_byte_hi[idx] {
            cluster_byte_hi[idx] = byte_end;
        }
    }

    let start_g = source_range.start();
    for i in 0..cluster_count {
        if cluster_byte_lo[i] == usize::MAX {
            continue;
        }
        let abs_lo = (chunk_lo + cluster_byte_lo[i]) as u32;
        let abs_hi = (chunk_lo + cluster_byte_hi[i]) as u32;
        if i > 0 {
            items.push(InlineItem::Glue {
                natural: 0.0,
                source_range: (start_g + i, start_g + i),
                visible_byte_range: abs_lo..abs_lo,
                buffer: empty_buffer(fs, row_height, ppi),
                cluster_advances: Vec::new(),
            });
        }
        let sub_source_end =
            if i + 1 == cluster_count { source_range.end() } else { start_g + i + 1 };
        let sub_text = &chunk_text[cluster_byte_lo[i]..cluster_byte_hi[i]];
        let (sub_buffer, sub_advance) =
            shape_chunk(fs, cache, sub_text, format, row_height, width, ppi);
        items.push(InlineItem::Box {
            advance: sub_advance,
            source_range: (start_g + i, sub_source_end),
            visible_byte_range: abs_lo..abs_hi,
            buffer: sub_buffer,
            cluster_advances: vec![sub_advance],
            atomic: false,
        });
    }
}

/// An empty (no-text) buffer used for zero-width separators. Cheap;
/// glyphon caches the empty-shape result.
fn empty_buffer(
    fs: &std::sync::Arc<std::sync::Mutex<glyphon::FontSystem>>, row_height: f32, ppi: f32,
) -> std::sync::Arc<std::sync::RwLock<glyphon::Buffer>> {
    use std::sync::{Arc, RwLock};
    let metric = row_height * ppi;
    let buffer = {
        let mut guard = fs.lock().unwrap();
        glyphon::Buffer::new(&mut guard, glyphon::Metrics::new(metric, metric))
    };
    Arc::new(RwLock::new(buffer))
}

/// Map a visible byte range back to source. Returns `(start, start)`
/// for an empty intersection (caller treats this as no-source).
fn source_range_for_visible(
    renderer: &MdRender, layout: &Layout, visible_lo: usize, visible_hi: usize,
) -> (Grapheme, Grapheme) {
    let segs = &renderer.buffer.current.segs;
    let mut lo = layout.source_range.end();
    let mut hi = layout.source_range.start();
    for seg in &layout.source_segments {
        if seg.visible.start == seg.visible.end {
            continue;
        }
        if seg.visible.end <= visible_lo || seg.visible.start >= visible_hi {
            continue;
        }
        if seg.one_to_one {
            let seg_byte_start = segs.offset_to_byte(seg.source.start());
            let head_bytes = visible_lo.saturating_sub(seg.visible.start);
            let tail_bytes = visible_hi.min(seg.visible.end) - seg.visible.start;
            let s = segs.byte_to_char_floor(seg_byte_start + head_bytes);
            let e = segs.byte_to_char_ceil(seg_byte_start + tail_bytes);
            if s < lo {
                lo = s;
            }
            if e > hi {
                hi = e;
            }
        } else {
            if seg.source.start() < lo {
                lo = seg.source.start();
            }
            if seg.source.end() > hi {
                hi = seg.source.end();
            }
        }
    }
    if lo > hi { (layout.source_range.start(), layout.source_range.start()) } else { (lo, hi) }
}

/// `true` when every byte in the visible range falls inside a
/// `one_to_one == true` segment (so per-grapheme advances are
/// well-defined and the resulting fragment is not atomic).
fn is_one_to_one_range(layout: &Layout, visible_lo: usize, visible_hi: usize) -> bool {
    if visible_lo >= visible_hi {
        return true;
    }
    let mut covered = visible_lo;
    while covered < visible_hi {
        let mut advanced = false;
        for seg in &layout.source_segments {
            if seg.visible.contains(&covered) {
                if !seg.one_to_one {
                    return false;
                }
                covered = seg.visible.end;
                advanced = true;
                break;
            }
        }
        if !advanced {
            return false;
        }
    }
    true
}

/// Whether grapheme `g` should shape against the emoji font (Twemoji) rather
/// than the format's own font. Icon-family spans are excluded: Nerd Font icon
/// glyphs sit in the supplementary PUA, which overlaps the emoji range, and the
/// emoji font carries no format color — so they'd render in the default fg
/// instead of blue (#4653).
pub(crate) fn shape_as_emoji(family: &FontFamily, g: &str) -> bool {
    !matches!(family, FontFamily::Icons) && crate::widgets::glyphon_label::is_emoji_grapheme(g)
}

fn format_to_attrs(format: &Format, base_row_height: f32) -> glyphon::AttrsOwned {
    let color = {
        let [r, g, b, a] = format.color.to_array();
        glyphon::Color::rgba(r, g, b, a)
    };
    let family = match format.family {
        FontFamily::Sans => glyphon::Family::SansSerif,
        FontFamily::Mono => glyphon::Family::Monospace,
        FontFamily::Icons => glyphon::Family::Name("Nerd Fonts Mono Symbols"),
    };
    let mut attrs = glyphon::Attrs::new()
        .color(color)
        .family(family)
        .weight(if format.bold { glyphon::Weight::BOLD } else { glyphon::Weight::NORMAL })
        .style(if format.italic { glyphon::Style::Italic } else { glyphon::Style::Normal });
    if format.superscript || format.subscript {
        let size = base_row_height * 0.75;
        attrs = attrs.metrics(glyphon::Metrics::new(size, size));
    }
    glyphon::AttrsOwned::new(&attrs)
}

// ─── greedy breaker (items → row break indices) ──────────────────────

/// `breaks[i]..breaks[i+1]` is row `i`'s items (with a sentinel at
/// the end equal to `items.len()`). Breaks the row when a `Box`
/// overflows, falling back to the last seen `Glue`.
pub fn greedy_break(items: &[InlineItem], width: f32, inline_pad: f32) -> Vec<usize> {
    let mut breaks = vec![0usize];
    let mut cur_width = 0.0f32;
    // Two wrap-point trackers so we can prefer whitespace breaks over
    // intra-token break opportunities (UAX#14 hyphen / slash / period
    // marks that I emit as zero-width `Glue` separators between
    // adjacent non-blank chunks). At overflow, wrap at the latest
    // whitespace glue if one exists on the row; fall back to any-glue
    // for tokens that have no surrounding whitespace (very long URLs
    // / identifiers).
    let mut last_whitespace_glue: Option<usize> = None;
    let mut last_any_glue: Option<usize> = None;
    let mut scope_bg_stack: Vec<bool> = Vec::new();
    let bg_open = |stack: &[bool]| stack.iter().any(|b| *b);
    // Pick where to wrap: prefer whitespace, but fall back to any-glue
    // if wrapping at whitespace would leave the current overflow item
    // still wider than the budget (long unbreakable tokens). Without
    // this fallback, "foo bar-baz" at a width where neither
    // "foo bar-baz" nor "bar-baz" fits would wrap at the space and
    // then have no way to wrap inside "bar-baz" — the row overshoots.
    let pick_wrap_point =
        |whitespace: Option<usize>, any: Option<usize>, i: usize, budget: f32| -> Option<usize> {
            let ws_fits = whitespace.is_some_and(|w| {
                let sum: f32 = items[w + 1..=i].iter().map(item_advance).sum();
                sum <= budget
            });
            if ws_fits { whitespace } else { any.or(whitespace) }
        };
    let mut i = 0;
    while i < items.len() {
        let effective_width =
            if bg_open(&scope_bg_stack) { width - 2.0 * inline_pad } else { width };
        match &items[i] {
            InlineItem::Box { advance, .. } | InlineItem::Image(ImageSpec { advance, .. }) => {
                let lg = if cur_width + advance > effective_width {
                    pick_wrap_point(last_whitespace_glue, last_any_glue, i, effective_width)
                } else {
                    None
                };
                if let Some(lg) = lg {
                    breaks.push(lg + 1);
                    cur_width = items[lg + 1..=i].iter().map(item_advance).sum::<f32>();
                    if bg_open(&scope_bg_stack) {
                        cur_width += inline_pad;
                    }
                    last_whitespace_glue = None;
                    last_any_glue = None;
                } else {
                    cur_width += advance;
                }
                i += 1;
            }
            InlineItem::Glue { natural, .. } => {
                // Per-space splits make trailing whitespace a series of
                // Glues. Without this overflow check they'd silently
                // accumulate past `width` (the Box-only check needs a
                // following Box to fire).
                let lg = if cur_width + natural > effective_width {
                    pick_wrap_point(last_whitespace_glue, last_any_glue, i, effective_width)
                } else {
                    None
                };
                if let Some(lg) = lg {
                    breaks.push(lg + 1);
                    cur_width = items[lg + 1..=i].iter().map(item_advance).sum::<f32>();
                    if bg_open(&scope_bg_stack) {
                        cur_width += inline_pad;
                    }
                    last_whitespace_glue = None;
                } else {
                    cur_width += natural;
                }
                // The current Glue is itself a candidate for the next
                // overflow. `last_whitespace_glue` is preserved across
                // a non-whitespace Glue so a zero-width intra-token
                // separator can't displace a preferred whitespace.
                last_any_glue = Some(i);
                if *natural > 0.0 {
                    last_whitespace_glue = Some(i);
                }
                i += 1;
            }
            InlineItem::Pad { advance, .. } => {
                cur_width += advance;
                i += 1;
            }
            InlineItem::Break { .. } => {
                breaks.push(i + 1);
                cur_width = 0.0;
                last_whitespace_glue = None;
                last_any_glue = None;
                i += 1;
            }
            InlineItem::StyleOpen(s) => {
                scope_bg_stack.push(s.format.background != egui::Color32::TRANSPARENT);
                i += 1;
            }
            InlineItem::StyleClose => {
                scope_bg_stack.pop();
                i += 1;
            }
            InlineItem::InteractionOpen(..) | InlineItem::InteractionClose => {
                i += 1;
            }
        }
    }
    breaks.push(items.len());
    breaks
}

fn item_advance(item: &InlineItem) -> f32 {
    match item {
        InlineItem::Box { advance, .. } => *advance,
        InlineItem::Glue { natural, .. } => *natural,
        InlineItem::Pad { advance, .. } => *advance,
        InlineItem::Image(spec) => spec.advance,
        InlineItem::Break { .. }
        | InlineItem::StyleOpen(_)
        | InlineItem::StyleClose
        | InlineItem::InteractionOpen(..)
        | InlineItem::InteractionClose => 0.0,
    }
}

/// `Some(ascent, descent)` for items that override row metrics; row
/// `ascent`/`descent` are the max of these and the text default.
fn item_metrics(item: &InlineItem) -> Option<(f32, f32)> {
    match item {
        InlineItem::Image(spec) => Some((spec.ascent, spec.descent)),
        _ => None,
    }
}

// ─── row construction (items + breaks → Vec<Row>) ────────────────────

/// Walk the broken stream, emitting one `Row` per break window with
/// one fragment per Box/Glue/Pad (no coalescing). Style scope events
/// update the running stack between fragments. The wrap-break-glue
/// (last item of a row when a next row follows, if it's a Glue)
/// renders as a zero-width fragment and is excluded from the row's
/// `source_range`.
pub fn build_rows(
    breaks: &[usize], items: &[InlineItem], layout: &Layout, row_height: f32, row_spacing: f32,
    inline_pad: f32,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::with_capacity(breaks.len().saturating_sub(1));
    let default_ascent = row_height * 0.8;
    let default_descent = row_height * 0.2;
    let mut style_stack: Vec<StyleInfo> = Vec::new();
    let mut interaction_stack: Vec<(egui::Id, egui::Sense)> = Vec::new();
    let mut y_top = 0.0f32;

    for win in breaks.windows(2) {
        let (start, end) = (win[0], win[1]);
        // Row baseline = `y_top + ascent`. Text fragments span the
        // text band (`baseline ± default_*`); image fragments hang
        // above the baseline so `image_bottom == baseline`.
        let (ascent, descent) = items[start..end]
            .iter()
            .filter_map(item_metrics)
            .fold((default_ascent, default_descent), |(a, d), (ia, id)| (a.max(ia), d.max(id)));
        let baseline = y_top + ascent;
        let text_top = baseline - default_ascent;
        let text_bottom = baseline + default_descent;
        // Last item is the row's "wrap break" — its advance is
        // suppressed and its source range is excluded from
        // `row.source_range`. Exactly one item per row plays this
        // role; per-space splits in `shape_to_items` plus `Glue`-aware
        // overflow checking in `greedy_break` ensure trailing source
        // whitespace that doesn't fit wraps onto the next row instead
        // of all collapsing into the wrap-break here.
        let wrap_break_idx = if end > start && end < items.len() {
            match &items[end - 1] {
                InlineItem::Glue { .. } | InlineItem::Break { .. } => Some(end - 1),
                _ => None,
            }
        } else {
            None
        };

        let mut x = 0.0f32;
        let mut row_fragments: Vec<Fragment> = Vec::new();
        let mut src_lo = layout.source_range.end();
        let mut src_hi = layout.source_range.start();
        let mut row_visible_lo: Option<u32> = None;
        let mut row_visible_hi: Option<u32> = None;
        let mut byte_x: Vec<(u32, f32)> = Vec::new();

        // Per-row pills: if this row opens already inside a bg scope
        // (the StyleOpen was processed in a prior row), prepend a
        // synthetic left-pad fragment so the row's pill has a left
        // edge. The matching scope-close-side pad is the walker's
        // explicit `Pad` item before `StyleClose`. `source_range` is
        // patched to `(src_lo, src_lo)` after the item loop so a
        // click on the pad lands at the row's first visible offset,
        // not back at the bg scope's start on a previous row.
        let mut left_pad_idx: Option<usize> = None;
        if style_stack
            .last()
            .is_some_and(|s| s.format.background != egui::Color32::TRANSPARENT)
        {
            let info = style_stack.last().unwrap();
            let rect =
                Rect::from_min_max(Pos2::new(x, text_top), Pos2::new(x + inline_pad, text_bottom));
            left_pad_idx = Some(row_fragments.len());
            row_fragments.push(Fragment {
                rect,
                content_inset: FragmentInset::default(),
                source_range: (info.source_range.start(), info.source_range.start()),
                style_stack: style_stack.clone(),
                content: FragmentContent::Spacer,
                atomic: false,
                interaction: interaction_stack.last().copied(),
            });
            x += inline_pad;
        }

        for (rel_idx, it) in items[start..end].iter().enumerate() {
            let abs_idx = start + rel_idx;
            let is_wrap_break = wrap_break_idx == Some(abs_idx);
            // Super/sub stick to text-band edges, not `ascent` extremes —
            // an image-inflated row would otherwise drag them outward.
            let (item_y_min, item_y_max) = {
                let f = style_stack.last().map(|s| &s.format);
                let text_height = default_ascent + default_descent;
                let small = text_height * 0.75;
                match f {
                    Some(f) if f.superscript => (text_top, text_top + small),
                    Some(f) if f.subscript => (text_bottom - small, text_bottom),
                    _ => (text_top, text_bottom),
                }
            };
            match it {
                InlineItem::Box {
                    advance,
                    source_range,
                    visible_byte_range,
                    buffer,
                    cluster_advances,
                    atomic,
                } => {
                    // Rect = text extent only. Inline bg padding is paint-
                    // time only; it extends outside the rect and fits
                    // into `row_spacing` (= 2 × inline_pad).
                    let rect = Rect::from_min_max(
                        Pos2::new(x, item_y_min),
                        Pos2::new(x + advance, item_y_max),
                    );
                    src_lo = src_lo.min(source_range.start());
                    src_hi = src_hi.max(source_range.end());
                    byte_x.push((visible_byte_range.start, x));
                    row_visible_lo = Some(
                        row_visible_lo
                            .map_or(visible_byte_range.start, |v| v.min(visible_byte_range.start)),
                    );
                    row_fragments.push(Fragment {
                        rect,
                        content_inset: FragmentInset::default(),
                        source_range: *source_range,
                        style_stack: style_stack.clone(),
                        content: FragmentContent::Glyphs {
                            buffer: buffer.clone(),
                            cluster_advances: cluster_advances.clone(),
                        },
                        atomic: *atomic,
                        interaction: interaction_stack.last().copied(),
                    });
                    x += advance;
                    byte_x.push((visible_byte_range.end, x));
                    row_visible_hi = Some(
                        row_visible_hi
                            .map_or(visible_byte_range.end, |v| v.max(visible_byte_range.end)),
                    );
                }
                InlineItem::Glue {
                    natural,
                    source_range,
                    visible_byte_range,
                    buffer,
                    cluster_advances,
                } => {
                    let effective_advance = if is_wrap_break { 0.0 } else { *natural };
                    let rect = Rect::from_min_max(
                        Pos2::new(x, item_y_min),
                        Pos2::new(x + effective_advance, item_y_max),
                    );
                    // Wrap-break-glue contributes zero-width fragment
                    // but DOES claim its source range so a char-arrow
                    // cursor on its byte resolves to this fragment
                    // (row N, right edge), not row N+1's start.
                    // Excluded from row.source_range so cmd+right
                    // lands at last *visible* offset.
                    if !is_wrap_break {
                        src_lo = src_lo.min(source_range.start());
                        src_hi = src_hi.max(source_range.end());
                    }
                    byte_x.push((visible_byte_range.start, x));
                    row_visible_lo = Some(
                        row_visible_lo
                            .map_or(visible_byte_range.start, |v| v.min(visible_byte_range.start)),
                    );
                    // Wrap-break-glue and zero-width inter-chunk glues
                    // emit no glyphs: wrap-break to keep the trailing
                    // whitespace from painting past the row edge,
                    // zero-width to keep paint+hit-test from
                    // discovering two fragments at the same origin.
                    let content = if is_wrap_break || *natural == 0.0 {
                        FragmentContent::Spacer
                    } else {
                        FragmentContent::Glyphs {
                            buffer: buffer.clone(),
                            cluster_advances: cluster_advances.clone(),
                        }
                    };
                    row_fragments.push(Fragment {
                        rect,
                        content_inset: FragmentInset::default(),
                        source_range: *source_range,
                        style_stack: style_stack.clone(),
                        content,
                        atomic: false,
                        interaction: interaction_stack.last().copied(),
                    });
                    x += effective_advance;
                    byte_x.push((visible_byte_range.end, x));
                    row_visible_hi = Some(
                        row_visible_hi
                            .map_or(visible_byte_range.end, |v| v.max(visible_byte_range.end)),
                    );
                }
                InlineItem::Pad { advance, source_pos } => {
                    let rect = Rect::from_min_max(
                        Pos2::new(x, text_top),
                        Pos2::new(x + advance, text_bottom),
                    );
                    row_fragments.push(Fragment {
                        rect,
                        content_inset: FragmentInset::default(),
                        source_range: (*source_pos, *source_pos),
                        style_stack: style_stack.clone(),
                        content: FragmentContent::Spacer,
                        atomic: false,
                        interaction: interaction_stack.last().copied(),
                    });
                    x += advance;
                }
                InlineItem::Image(spec) => {
                    let img_top = baseline - spec.ascent;
                    let img_bottom = baseline + spec.descent;
                    let rect = Rect::from_min_max(
                        Pos2::new(x, img_top),
                        Pos2::new(x + spec.advance, img_bottom),
                    );
                    src_lo = src_lo.min(spec.source_range.start());
                    src_hi = src_hi.max(spec.source_range.end());
                    row_fragments.push(Fragment {
                        rect,
                        content_inset: FragmentInset::default(),
                        source_range: spec.source_range,
                        style_stack: style_stack.clone(),
                        content: FragmentContent::Image { url: spec.url.clone() },
                        atomic: true,
                        interaction: interaction_stack.last().copied(),
                    });
                    x += spec.advance;
                }
                InlineItem::Break { source_range, visible_byte_range } => {
                    let rect =
                        Rect::from_min_max(Pos2::new(x, text_top), Pos2::new(x, text_bottom));
                    byte_x.push((visible_byte_range.start, x));
                    byte_x.push((visible_byte_range.end, x));
                    row_visible_lo = Some(
                        row_visible_lo
                            .map_or(visible_byte_range.start, |v| v.min(visible_byte_range.start)),
                    );
                    row_visible_hi = Some(
                        row_visible_hi
                            .map_or(visible_byte_range.end, |v| v.max(visible_byte_range.end)),
                    );
                    row_fragments.push(Fragment {
                        rect,
                        content_inset: FragmentInset::default(),
                        source_range: *source_range,
                        style_stack: style_stack.clone(),
                        content: FragmentContent::Spacer,
                        atomic: false,
                        interaction: interaction_stack.last().copied(),
                    });
                }
                InlineItem::StyleOpen(info) => style_stack.push(info.clone()),
                InlineItem::StyleClose => {
                    style_stack.pop();
                }
                InlineItem::InteractionOpen(id, sense) => {
                    interaction_stack.push((*id, *sense));
                }
                InlineItem::InteractionClose => {
                    interaction_stack.pop();
                }
            }
        }

        // Patch the row's synthetic left-pad source position to the
        // first visible offset on this row (excluding wrap-break-glue,
        // which `src_lo` already skips). A click on the pad lands at
        // row start instead of the scope's start on a previous row.
        if let Some(idx) = left_pad_idx {
            if src_lo <= src_hi {
                row_fragments[idx].source_range = (src_lo, src_lo);
            }
        }

        // Per-row pills: if this row ends still inside a bg scope (the
        // StyleClose lives in a future row), append a synthetic right-
        // pad fragment so the row's pill has a right edge. Source
        // position is `src_hi` — the row's last visible offset within
        // the scope, excluding wrap-break-glue — so a click on the
        // pad lands at the row's end rather than the wrap boundary
        // (which renders on the next row).
        if style_stack
            .last()
            .is_some_and(|s| s.format.background != egui::Color32::TRANSPARENT)
        {
            let info = style_stack.last().unwrap();
            let source_pos = if src_lo <= src_hi { src_hi } else { info.source_range.start() };
            let rect =
                Rect::from_min_max(Pos2::new(x, text_top), Pos2::new(x + inline_pad, text_bottom));
            row_fragments.push(Fragment {
                rect,
                content_inset: FragmentInset::default(),
                source_range: (source_pos, source_pos),
                style_stack: style_stack.clone(),
                content: FragmentContent::Spacer,
                atomic: false,
                interaction: interaction_stack.last().copied(),
            });
        }

        // Anchors for zero-visible source segments whose visible
        // position falls within this row's covered byte range.
        let mut anchors: Vec<Anchor> = Vec::new();
        if let (Some(lo), Some(hi)) = (row_visible_lo, row_visible_hi) {
            for seg in &layout.source_segments {
                if seg.visible.start != seg.visible.end {
                    continue;
                }
                let v = seg.visible.start as u32;
                if v < lo || v > hi {
                    continue;
                }
                let anchor_x = byte_x
                    .iter()
                    .find(|(b, _)| *b == v)
                    .map(|(_, x)| *x)
                    .unwrap_or(0.0);
                src_lo = src_lo.min(seg.source.start());
                src_hi = src_hi.max(seg.source.end());
                anchors.push(Anchor {
                    source_range: seg.source,
                    x: anchor_x,
                    y_top: text_top,
                    height: text_bottom - text_top,
                });
            }
        }

        if src_lo > src_hi {
            src_lo = layout.source_range.start();
            src_hi = layout.source_range.start();
        }
        rows.push(Row {
            y_top,
            ascent,
            descent,
            source_range: (src_lo, src_hi),
            fragments: row_fragments,
            anchors,
        });
        y_top += ascent + descent + row_spacing;
    }
    rows
}

// ─── paint (WrapUnitLayout → painted glyphs + MdRender::fragments) ───

impl MdRender {
    /// Paint a wrap unit. For each row, paint each fragment in
    /// turn; the screen-space fragment is mirrored onto
    /// `self.fragments` for cursor / hit-test lookups.
    ///
    /// Backgrounds use smart corners: adjacent same-bg fragments
    /// square off the touching corners so the joined visual looks
    /// like one continuous pill instead of a chain of pills.
    pub fn show_wrap_layout(&mut self, ui: &mut Ui, top_left: Pos2, layout: &WrapUnitLayout) {
        use crate::theme::palette_v2::ThemeExt as _;
        let ppi = self.ctx.pixels_per_point();
        let row_height = layout.row_height;
        let inline_pad = self.layout.inline_padding;
        let paint_top_left = top_left;
        let neutral_color = {
            let c = self.ctx.get_lb_theme().neutral_fg();
            let [r, g, b, a] = c.to_array();
            glyphon::Color::rgba(r, g, b, a)
        };

        for row in &layout.rows {
            let bg_of = |f: &Fragment| -> Option<egui::Color32> {
                f.style_stack
                    .last()
                    .map(|s| s.format.background)
                    .filter(|c| *c != egui::Color32::TRANSPARENT)
            };
            let chip_of = |f: &Fragment| -> bool { f.style_stack.last().is_some_and(|s| s.chip) };

            // Pass 1: coalesce same-bg chains into one pill (single
            // fill, single stroke — no seams at fragment boundaries).
            // Paint-only; `self.fragments` keeps per-fragment shape
            // for cursor / hit-test.
            let mut i = 0;
            while i < row.fragments.len() {
                let Some(bg) = bg_of(&row.fragments[i]) else {
                    i += 1;
                    continue;
                };
                let chip = chip_of(&row.fragments[i]);
                let border = row.fragments[i]
                    .style_stack
                    .last()
                    .map(|s| s.format.border)
                    .unwrap_or(egui::Color32::TRANSPARENT);
                let chain_start = i;
                let mut chain_end = i + 1;
                while chain_end < row.fragments.len()
                    && bg_of(&row.fragments[chain_end]) == Some(bg)
                    && chip_of(&row.fragments[chain_end]) == chip
                {
                    chain_end += 1;
                }
                let chain_left = row.fragments[chain_start].rect.left() + paint_top_left.x;
                let chain_right = row.fragments[chain_end - 1].rect.right() + paint_top_left.x;
                let row_top = row.fragments[chain_start].rect.top() + paint_top_left.y;
                let row_bottom = row.fragments[chain_start].rect.bottom() + paint_top_left.y;
                let mut bg_rect = Rect::from_min_max(
                    egui::Pos2::new(chain_left, row_top - inline_pad),
                    egui::Pos2::new(chain_right, row_bottom + inline_pad),
                );
                let mut radius = 2.0;
                if chip {
                    // Capsule spanning exactly the row box — no inline_pad
                    // extension, so it stays inside the line where full-row
                    // pills like inline code bleed past it.
                    bg_rect = Rect::from_min_max(
                        egui::Pos2::new(chain_left, row_top),
                        egui::Pos2::new(chain_right, row_bottom),
                    );
                    radius = (row_bottom - row_top) / 2.0;
                }
                if ui.clip_rect().intersects(bg_rect) {
                    let rounding = corner_rounding(true, true, radius);
                    ui.painter().rect_filled(bg_rect, rounding, bg);
                    if border != egui::Color32::TRANSPARENT {
                        ui.painter().rect_stroke(
                            bg_rect,
                            rounding,
                            Stroke::new(1.0, border),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                i = chain_end;
            }

            // Pass 2: per-fragment glyphs, decorations, and mirror-
            // onto-self.fragments.
            for frag in row.fragments.iter() {
                let screen_rect = frag.rect.translate(paint_top_left.to_vec2());
                if !ui.clip_rect().intersects(screen_rect) {
                    let mut f = frag.clone();
                    f.rect = screen_rect;
                    self.fragments.push(f);
                    continue;
                }

                // Glyphs: reuse the walker-shaped Buffer. Walker
                // shaped at device-pixel metrics; `TextBufferArea::new`
                // multiplies the rect by ppi so the two agree.
                if let FragmentContent::Glyphs { buffer, .. } = &frag.content {
                    let glyph_origin = screen_rect.min
                        + Vec2::new(frag.content_inset.left, frag.content_inset.top);
                    let shaped_left = buffer.read().unwrap().shaped_left(ppi);
                    let paint_rect = Rect::from_min_size(
                        glyph_origin - Vec2::new(shaped_left, 0.0),
                        screen_rect.size(),
                    );
                    self.text_areas.push(TextBufferArea::new(
                        buffer.clone(),
                        paint_rect,
                        neutral_color,
                        ui.ctx(),
                        ui.clip_rect(),
                    ));
                }

                if let FragmentContent::Image { url } = &frag.content {
                    self.embeds.show(ui, url, screen_rect);

                    // The image's opaque fill hides the selection slot behind it,
                    // so tint it when the selection covers it (a partial overlap
                    // reveals it to raw text instead). iOS draws this natively.
                    if ui.ctx().os() != egui::os::OperatingSystem::IOS {
                        use crate::theme::palette_v2::ThemeExt as _;
                        let sel = self
                            .in_progress_selection
                            .unwrap_or(self.buffer.current.selection);
                        let sr = frag.source_range;
                        if !sel.is_empty() && sel.contains_range(&sr, true, true) {
                            let theme = self.ctx.get_lb_theme();
                            let accent = theme.bg().get_color(theme.prefs().primary);
                            let tint = egui::Color32::from_rgba_unmultiplied(
                                accent.r(),
                                accent.g(),
                                accent.b(),
                                90,
                            );
                            ui.painter().rect_filled(screen_rect, 2.0, tint);
                        }
                    }
                }

                // Collected here, painted after the text callback (#4617).
                if let Some(style) = frag.style_stack.last() {
                    let fmt = &style.format;
                    let baseline_top = screen_rect.min.y + frag.content_inset.top;
                    let x = screen_rect.left()..=screen_rect.right();
                    if fmt.strikethrough {
                        self.deco_lines.push(DecoLine {
                            x: x.clone(),
                            y: baseline_top + row_height * 0.55,
                            color: fmt.color,
                        });
                    }
                    if fmt.underline {
                        self.deco_lines.push(DecoLine {
                            x,
                            y: baseline_top + row_height * 0.95,
                            color: fmt.color,
                        });
                    }
                }

                // Mirror onto the flat fragment list for cursor / hit-test.
                let mut f = frag.clone();
                f.rect = screen_rect;
                self.fragments.push(f);
            }

            // Per-row anchors (zero-visible source segments) become
            // zero-width Spacer fragments in screen coords so cursor
            // / hit-test lookups can resolve them via the same
            // `fragments` list. They have empty `style_stack` so paint
            // doesn't draw anything for them. `atomic` so a click on
            // an invisible override (e.g. the fold-tag HTML comment at
            // a heading's end) selects the whole source range —
            // typing then replaces the tag and unfolds the section
            // rather than inserting next to it.
            for anchor in &row.anchors {
                let pos = paint_top_left + Vec2::new(anchor.x, anchor.y_top);
                let rect = Rect::from_min_size(pos, Vec2::new(0.0, anchor.height));
                self.fragments.push(Fragment {
                    rect,
                    content_inset: FragmentInset::default(),
                    source_range: anchor.source_range,
                    style_stack: Vec::new(),
                    content: FragmentContent::Spacer,
                    atomic: true,
                    interaction: None,
                });
            }

            // Record the row's source range for Bound::Line.
            self.bounds.wrap_lines.push(row.source_range);
        }
    }
}

/// Per-corner rounding for smart-corner bg painting. Returns a
/// `CornerRadius` (egui 0.31's per-corner type) with `radius` on
/// outer corners and `0` on touching corners.
fn corner_rounding(round_left: bool, round_right: bool, radius: f32) -> egui::CornerRadius {
    let r = radius as u8;
    let z = 0u8;
    egui::CornerRadius {
        nw: if round_left { r } else { z },
        sw: if round_left { r } else { z },
        ne: if round_right { r } else { z },
        se: if round_right { r } else { z },
    }
}

// ─── cursor + hit-test helpers (read-only over fragments) ────────────

impl MdRender {
    /// Find the fragment whose `source_range` contains `offset`,
    /// inclusive of both endpoints (so cursor positions at line-
    /// trailing `\n` bytes — excluded from any source line by
    /// `range_split_newlines` — still resolve). Reverse search:
    /// at a shared boundary `(a, b)`/`(b, c)`, the *later* fragment
    /// wins (cursor-affinity matches paint).
    pub fn fragment_at_offset(&self, offset: Grapheme) -> Option<&Fragment> {
        self.fragments.iter().rev().find(|f| {
            let (s, e) = f.source_range;
            s <= offset && offset <= e
        })
    }

    /// Find the fragment whose rect contains `pos` (clip-aware
    /// hit-test). Atomic fragments are checked by the caller
    /// (`pos_to_range`) for edge-snap behavior; this routine just
    /// returns the rect owner.
    pub fn fragment_at_pos(&self, pos: Pos2) -> Option<&Fragment> {
        self.fragments.iter().rev().find(|f| f.rect.contains(pos))
    }

    /// Allocate `ui.interact` for every fragment tagged with
    /// `(salt, sense)` and union the per-fragment responses by parent
    /// id into `self.interaction_responses`. Consumers compute
    /// `ui.id().with(salt)` to look up the merged response.
    ///
    /// Must run after the editor's own `ui.interact` so per-fragment
    /// rects sit on top in egui's z-order.
    pub fn interact_fragments(&mut self, ui: &mut egui::Ui) {
        let mut per_parent: std::collections::HashMap<egui::Id, egui::Response> =
            std::collections::HashMap::new();
        let parent_base = ui.id();
        for (i, f) in self.fragments.iter().enumerate() {
            let Some((salt, sense)) = f.interaction else {
                continue;
            };
            let parent_id = parent_base.with(salt);
            let r = ui.interact(f.rect, parent_id.with(i), sense);
            let merged = match per_parent.remove(&parent_id) {
                Some(prev) => prev.union(r),
                None => r,
            };
            per_parent.insert(parent_id, merged);
        }
        self.interaction_responses = per_parent;
    }

    /// Find the closest fragment to `pos` by (y_dist, x_dist), with
    /// preference for empty-range fragments (anchors) and for
    /// atomic Spacer anchors (invisible-override markers like the
    /// fold tag, which need to win the tie against the adjacent text
    /// box so a click resolves to "select the whole override" via
    /// `pos_to_range`'s atomic branch). Returns `None` only when
    /// `self.fragments` is empty.
    pub fn closest_fragment_at_pos(&self, pos: Pos2) -> Option<usize> {
        let mut closest: Option<usize> = None;
        let mut closest_dist = (f32::INFINITY, f32::INFINITY);
        for (i, f) in self.fragments.iter().enumerate() {
            if f.rect.contains(pos) {
                return Some(i);
            }
            let y_dist = if f.rect.y_range().contains(pos.y) {
                0.0
            } else {
                (pos.y - f.rect.top())
                    .abs()
                    .min((pos.y - f.rect.bottom()).abs())
            };
            let x_dist = if f.rect.x_range().contains(pos.x) {
                0.0
            } else {
                (pos.x - f.rect.left())
                    .abs()
                    .min((pos.x - f.rect.right()).abs())
            };
            let dist = (y_dist, x_dist);
            let prefer = dist == closest_dist
                && (f.source_range.start() == f.source_range.end()
                    || (f.atomic && matches!(f.content, FragmentContent::Spacer)));
            if dist < closest_dist || prefer {
                closest = Some(i);
                closest_dist = dist;
            }
        }
        closest
    }

    /// Cursor screen x for `offset` within `fragment`. For atomic
    /// fragments, snap to the nearest edge of `source_range` by
    /// the offset's position within the range (midpoint rule). For
    /// non-atomic, sum cluster advances from `source_range.start`.
    pub fn fragment_x(&self, fragment: &Fragment, offset: Grapheme) -> f32 {
        let chip_scope = fragment
            .style_stack
            .last()
            .filter(|s| s.chip)
            .map(|s| s.source_range);
        if fragment.atomic {
            // The caret beside a chip atom sits beside the capsule: back
            // out the side pad between this fragment's glyph rect and the
            // capsule edge (pads span the row, so height ≈ row height).
            let pad =
                if chip_scope.is_some() { fragment.rect.height() * CHIP_SIDE_PAD } else { 0.0 };
            // Absolute midpoint offset of the range; offsets in the first
            // half snap to the left edge, the rest to the right edge.
            let mid = fragment.source_range.start().0
                + (fragment
                    .source_range
                    .end()
                    .0
                    .saturating_sub(fragment.source_range.start().0))
                    / 2;
            if offset.0 < mid {
                fragment.rect.min.x + fragment.content_inset.left - pad
            } else {
                fragment.rect.max.x - fragment.content_inset.right + pad
            }
        } else {
            // A chip's scope pads are the capsule's side padding; the
            // caret at their source position renders at the capsule's
            // outer edge — left edge for the leading pad, right for the
            // trailing.
            if let Some(scope) = chip_scope {
                if fragment.source_range.is_empty()
                    && matches!(fragment.content, FragmentContent::Spacer)
                {
                    return if fragment.source_range.start() == scope.start() {
                        fragment.rect.min.x
                    } else {
                        fragment.rect.max.x
                    };
                }
            }
            let into = offset.0.saturating_sub(fragment.source_range.start().0);
            if let FragmentContent::Glyphs { cluster_advances, .. } = &fragment.content {
                if !cluster_advances.is_empty() {
                    let sum: f32 = cluster_advances.iter().take(into).sum();
                    return fragment.rect.min.x + fragment.content_inset.left + sum;
                }
            }
            // Fallback: snap to nearest edge by midpoint.
            let range_count = fragment
                .source_range
                .end()
                .0
                .saturating_sub(fragment.source_range.start().0);
            if range_count == 0 || into * 2 < range_count {
                fragment.rect.min.x + fragment.content_inset.left
            } else {
                fragment.rect.max.x - fragment.content_inset.right
            }
        }
    }

    /// Inverse of `fragment_x` for hit-test: find the offset within
    /// `fragment` closest to screen x. Atomic fragments snap to
    /// source_range.start / .end based on the midpoint rule.
    pub fn fragment_offset(&self, fragment: &Fragment, x: f32) -> Grapheme {
        if fragment.atomic {
            let mid_x = (fragment.rect.min.x + fragment.rect.max.x) / 2.0;
            return if x < mid_x {
                fragment.source_range.start()
            } else {
                fragment.source_range.end()
            };
        }
        if let FragmentContent::Glyphs { cluster_advances, .. } = &fragment.content {
            if !cluster_advances.is_empty() {
                let mut cur = fragment.rect.min.x + fragment.content_inset.left;
                for (i, adv) in cluster_advances.iter().enumerate() {
                    let next = cur + adv;
                    if x < (cur + next) / 2.0 {
                        return fragment.source_range.start() + i;
                    }
                    cur = next;
                }
                return fragment.source_range.end();
            }
        }
        // Empty advances: snap by midpoint.
        let mid_x = (fragment.rect.min.x + fragment.rect.max.x) / 2.0;
        if x < mid_x { fragment.source_range.start() } else { fragment.source_range.end() }
    }
}
