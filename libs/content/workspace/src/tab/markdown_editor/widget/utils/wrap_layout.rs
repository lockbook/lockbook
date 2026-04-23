use egui::{Pos2, Rect, Sense, Stroke, Ui, Vec2};

use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::TextBufferArea;
use crate::tab::markdown_editor::MdRender;

struct SplitRow {
    text: String,
    range: (Grapheme, Grapheme),
}

/// One row's contribution to a section's layout. Built by `plan_section`,
/// consumed identically by `span_section` (sums advances) and
/// `show_override_section` (places + paints).
struct RowPlacement {
    /// Per-row text passed to `upsert_glyphon_buffer_unwrapped` at paint time.
    text: String,
    /// Source range covered by this row.
    range: (Grapheme, Grapheme),
    /// Natural shaped width of `text` (no re-wrap). Used by `plan_section`
    /// to detect when cosmic-text refused to wrap a row that's wider than
    /// `wrap.width`, so it can fall back to a different wrap mode.
    shaped_w: f32,
    /// Advance applied **before** placing the row. Combines (a) the
    /// section's pre-padding when this is the first row of a padded section
    /// starting mid-row, and (b) a section-break jump when the row doesn't
    /// fit in `row_remaining` but would on a fresh row.
    break_advance: f32,
    /// Advance applied **after** placing the row. Combines the row's own
    /// content advance with the section's post-padding when this is the
    /// last row of a padded section.
    advance: f32,
}
use crate::tab::markdown_editor::bounds::Lines;
use crate::tab::markdown_editor::galleys::GalleyInfo;
use crate::tab::markdown_editor::widget::inline::Response;

#[derive(Clone, Debug)]
pub struct Wrap {
    pub offset: f32,
    pub width: f32,
    pub row_height: f32, // overridden by headings
    pub row_spacing: f32,
    pub row_ranges: Lines,
}

impl Wrap {
    pub fn new(width: f32, row_height: f32, row_spacing: f32) -> Self {
        Self { offset: 0.0, width, row_height, row_spacing, row_ranges: Vec::new() }
    }

    /// The index of the current row
    pub fn row(&self) -> usize {
        (self.offset / self.width) as _
    }

    /// The start of the current row
    pub fn row_start(&self) -> f32 {
        self.row() as f32 * self.width
    }

    /// The end of the current row
    pub fn row_end(&self) -> f32 {
        self.row_start() + self.width
    }

    /// The offset from the start of the row
    pub fn row_offset(&self) -> f32 {
        self.offset - self.row_start()
    }

    /// The remaining space on the row
    pub fn row_remaining(&self) -> f32 {
        self.row_end() - self.offset
    }

    /// The height of the wrapped text; always at least [`Self::row_height`]
    pub fn height(&self) -> f32 {
        let num_rows = ((self.offset / self.width).ceil() as usize).max(1);
        let num_spacings = num_rows.saturating_sub(1);
        num_rows as f32 * self.row_height + num_spacings as f32 * self.row_spacing
    }

    pub fn add_range(&mut self, range: (Grapheme, Grapheme)) {
        let row = self.row();
        if let Some(line) = self.row_ranges.get_mut(row) {
            line.0 = line.0.min(range.0);
            line.1 = line.1.max(range.1);
        } else {
            self.row_ranges.push(range);

            // prefer next row
            if row > 0 {
                if let Some(prev_line) = self.row_ranges.get_mut(row - 1) {
                    // when two rows' ranges touch, shorten the earlier so that
                    // the boundary belongs to the later. Skip if the earlier
                    // row has nothing to give up (its range is empty); without
                    // this guard, an empty first row at position 0 underflows.
                    if prev_line.end() == range.start() && prev_line.1 > prev_line.0 {
                        prev_line.1 -= 1;
                    }
                }
            }
        }
    }
}

impl MdRender {
    pub fn new_wrap(&self, width: f32) -> Wrap {
        Wrap::new(width, self.layout.row_height, self.layout.row_spacing)
    }

    /// Returns the height of a single text section. Pass a fresh wrap initialized with the desired width.
    pub fn height_section(
        &self, wrap: &mut Wrap, range: (Grapheme, Grapheme), text_format: Format,
    ) -> f32 {
        wrap.offset += self.span_section(wrap, range, text_format);
        wrap.height()
    }

    /// Returns the wrap-cursor advance produced by laying out a text section
    /// — the same advance [`Self::show_override_section`] applies. Sums the
    /// per-row break + advance from a shared plan, so measure and render
    /// cannot disagree.
    pub fn span_section(
        &self, wrap: &Wrap, range: (Grapheme, Grapheme), text_format: Format,
    ) -> f32 {
        self.plan_section(wrap, None, range, &text_format)
            .into_iter()
            .map(|p| p.break_advance + p.advance)
            .sum()
    }

    /// Show source text specified by the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    pub fn show_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap, range: (Grapheme, Grapheme),
        text_format: Format,
    ) -> Response {
        self.show_override_section(ui, top_left, wrap, range, text_format, None, Sense::hover())
    }

    /// Returns the height of a single section that's not from the document's
    /// source text. Pass a fresh wrap initialized with the desired width.
    pub fn height_override_section(&self, wrap: &mut Wrap, text: &str, text_format: Format) -> f32 {
        wrap.offset += self.span_override_section(wrap, text, text_format);
        wrap.height()
    }

    /// Like [`Self::span_section`] but for text that isn't from the
    /// document's source (e.g. a shortcode emoji preview).
    pub fn span_override_section(&self, wrap: &Wrap, text: &str, text_format: Format) -> f32 {
        self.plan_section(wrap, Some(text), Default::default(), &text_format)
            .into_iter()
            .map(|p| p.break_advance + p.advance)
            .sum()
    }

    /// Splits text into rows. For override text all rows share `range`; for
    /// source text each row gets its sub-range (cloned from the buffer).
    /// `glyph_wrap` selects the wrap mode used to discover break points:
    /// `false` uses `WordOrGlyph` (word boundaries preferred); `true` uses
    /// `Glyph` and is the fallback `plan_section` reaches for when
    /// `WordOrGlyph` produces a row wider than the wrap width — a known
    /// cosmic-text quirk on some bold mixed-script content.
    fn split_rows(
        &self, override_text: Option<&str>, range: (Grapheme, Grapheme), font_size: f32,
        wrap: &Wrap, text_format: &Format, glyph_wrap: bool,
    ) -> Vec<SplitRow> {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let is_override = override_text.is_some();
        let shape = |s: &str, w: f32| {
            if glyph_wrap {
                self.upsert_glyphon_buffer_glyph(s, font_size, font_size, w, text_format)
            } else {
                self.upsert_glyphon_buffer(s, font_size, font_size, w, text_format)
            }
        };

        let first_row_bytes = {
            let tmp = shape(text, wrap.row_remaining());
            let tmp = tmp.read().unwrap();
            // Same display-vs-source ordering caveat as the per-run fold
            // below: in an RTL run the visually-last glyph is the
            // source-leftmost one, so `glyphs.last().end` would understate
            // the row's byte span. Take the max source byte over all glyphs.
            //
            // If the first run is empty (text starts with a BiDi paragraph
            // separator like `\u{85}` or `\u{1c}`), fall through to 0 so the
            // whole text goes into the second pass and gets split per
            // separator. Otherwise we'd lump separators into one row whose
            // re-shape produces a multi-row buffer and inflates the rect.
            tmp.layout_runs()
                .next()
                .filter(|run| !run.glyphs.is_empty())
                .map(|run| {
                    run.glyphs
                        .iter()
                        .fold(0usize, |hi, g| hi.max(g.start).max(g.end))
                })
                .unwrap_or(0)
        };

        let first_row_str = text[..first_row_bytes].to_string();
        let remaining_str = text[first_row_bytes..].to_string();

        let first_row_range = if is_override {
            range
        } else {
            let start = self.offset_to_byte(range.start());
            // `start + first_row_bytes` comes from cosmic-text glyph positions
            // which can land inside a grapheme cluster (an inserted combining
            // mark joining the preceding char). Snap the end up to the next
            // boundary so we don't `range_to_char` a non-boundary byte. Clamp
            // to `range.end()` so we don't extend past the section when the
            // snap crosses the section boundary (e.g. section ends inside a
            // cluster — the math closing `$` after a Hindi conjunct).
            let r = self.range_to_char_ceil((start, start + first_row_bytes));
            (r.0, r.1.min(range.end()))
        };

        let mut split = vec![SplitRow { text: first_row_str, range: first_row_range }];

        if !remaining_str.is_empty() {
            let remaining_start_byte = if is_override {
                Default::default() // unused
            } else {
                let remaining_range = (first_row_range.end(), range.end());
                self.range_to_byte(remaining_range).start()
            };

            let run_byte_ranges: Vec<(usize, usize)> = {
                let tmp = shape(&remaining_str, wrap.width);
                let tmp = tmp.read().unwrap();
                // Cosmic-text indexes glyphs into its own per-paragraph
                // buffer lines, not into the string we passed in. It splits
                // on Unicode BiDi paragraph separators (anything in BiDi
                // class B: `\n`, `\r`, `\u{85}`, `\u{1c}`-`\u{1e}`,
                // `\u{2029}`). For an input like
                //
                // ```text
                // hello\u{1c}world
                // ```
                //
                // we get two `LayoutRun`s, each numbering its glyphs from 0.
                // Translate to absolute offsets in `remaining_str` by tracking
                // each paragraph's start as we iterate.
                let mut line_base: usize = 0;
                let mut line_text_len: usize = 0;
                let mut prev_line_i: Option<usize> = None;
                tmp.layout_runs()
                    .map(|run| {
                        if prev_line_i != Some(run.line_i) {
                            let search_start = match prev_line_i {
                                Some(_) => line_base + line_text_len,
                                None => 0,
                            };
                            let sep = remaining_str[search_start..].find(run.text).unwrap_or(0);
                            line_base = search_start + sep;
                            line_text_len = run.text.len();
                            prev_line_i = Some(run.line_i);
                        }
                        // Cosmic-text reports glyph byte ranges in display
                        // order, not source order. For LTR runs that matches
                        // source order, but for an RTL run like
                        //
                        // ```text
                        // שלום
                        // ```
                        //
                        // the first glyph in `glyphs` is the visually leftmost
                        // = source-rightmost, and within an individual glyph
                        // `g.start` may itself exceed `g.end`. Treat each
                        // glyph as a logical (lo, hi) pair and take the
                        // overall min/max to recover the source byte range.
                        let (start, end) =
                            run.glyphs.iter().fold((usize::MAX, 0usize), |(lo, hi), g| {
                                let g_lo = g.start.min(g.end);
                                let g_hi = g.start.max(g.end);
                                (lo.min(g_lo), hi.max(g_hi))
                            });
                        let (start, end) =
                            if run.glyphs.is_empty() { (0, 0) } else { (start, end) };
                        (line_base + start, line_base + end)
                    })
                    .collect()
            };

            for (start, end) in run_byte_ranges {
                let skip_leading_space = remaining_str[start..].starts_with(' ');
                let text_start = if skip_leading_space { start + 1 } else { start };
                let row_range = if is_override {
                    range
                } else {
                    // Same cluster-snap-and-clamp concern as the first-row
                    // branch above.
                    let r = self.range_to_char_ceil((
                        remaining_start_byte + text_start,
                        remaining_start_byte + end,
                    ));
                    (r.0, r.1.min(range.end()))
                };
                split.push(SplitRow {
                    text: remaining_str[text_start..end].to_string(),
                    range: row_range,
                });
            }
        }

        split
    }

    /// Decides where each row of a section sits *without* mutating the
    /// caller's wrap. Returns one [`RowPlacement`] per row; both
    /// [`Self::span_section`] (measure) and [`Self::show_override_section`]
    /// (render) iterate this same plan and apply its advances.
    ///
    /// Folds in section-level padding for inlines with a background
    /// (highlights, code spans, etc.) — pre-pad lands in the first row's
    /// `break_advance`, post-pad in the last row's `advance` — so callers
    /// don't track padding separately from row layout.
    ///
    /// Tries `WordOrGlyph` wrap first; if cosmic-text leaves a row wider
    /// than the wrap width (a known mixed-script quirk where it refuses to
    /// break), falls back to `Glyph` mode for guaranteed wrapping.
    fn plan_section(
        &self, wrap: &Wrap, override_text: Option<&str>, range: (Grapheme, Grapheme),
        text_format: &Format,
    ) -> Vec<RowPlacement> {
        let plan = self.plan_section_with_mode(wrap, override_text, range, text_format, false);
        if plan.iter().any(|p| p.shaped_w > wrap.width + 0.5) {
            self.plan_section_with_mode(wrap, override_text, range, text_format, true)
        } else {
            plan
        }
    }

    fn plan_section_with_mode(
        &self, wrap: &Wrap, override_text: Option<&str>, range: (Grapheme, Grapheme),
        text_format: &Format, glyph_wrap: bool,
    ) -> Vec<RowPlacement> {
        let ppi = self.ctx.pixels_per_point();
        let font_size = if text_format.superscript || text_format.subscript {
            wrap.row_height * 0.75
        } else {
            wrap.row_height
        };
        let padded = text_format.background != egui::Color32::TRANSPARENT;

        // Pre-padding: keep the background's left edge from visually
        // overlapping the previous inline. Only when the section starts
        // mid-row.
        let pre_pad = if padded && wrap.row_offset() > 0.5 {
            self.layout.inline_padding.min(wrap.row_remaining())
        } else {
            0.0
        };

        let mut sim = Wrap { offset: wrap.offset + pre_pad, ..wrap.clone() };
        let split = self.split_rows(override_text, range, font_size, &sim, text_format, glyph_wrap);
        let split_len = split.len();
        let mut out = Vec::with_capacity(split_len);
        for (i, row) in split.into_iter().enumerate() {
            let shaped_w = self
                .upsert_glyphon_buffer_unwrapped(
                    &row.text,
                    font_size,
                    font_size,
                    sim.width,
                    text_format,
                )
                .read()
                .unwrap()
                .shaped_size(ppi)
                .x;
            // Section-break: if the row doesn't fit in what's left of the
            // current visual row but would fit on a fresh one, jump first.
            // Cosmic-text won't break unbreakable tokens (e.g. an Arabic
            // word) mid-cluster, so without this they'd overflow past the
            // wrap width.
            let mut break_advance = if i == 0 { pre_pad } else { 0.0 };
            if i == 0
                && sim.row_offset() > 0.5
                && shaped_w > sim.row_remaining() + 0.5
                && shaped_w <= sim.width + 0.5
            {
                // Snap to the next row's boundary explicitly. `sim.offset
                // += sim.row_remaining()` would land us mathematically on
                // the boundary, but f32 rounding can leave us a sub-pixel
                // short — and then `sim.row_remaining()` below returns ~0
                // instead of a full row, collapsing the next advance to
                // zero and stacking placements at the same x.
                let next_row_start = ((sim.offset / sim.width).floor() + 1.0) * sim.width;
                let jump = next_row_start - sim.offset;
                break_advance += jump;
                sim.offset = next_row_start;
            }
            // Advance after placement: a full row for non-final rows; for
            // the final row, the natural width capped at `row_remaining`.
            // The cap means an over-wide last row leaves the wrap cursor at
            // the row end — the galley extends past visually, but
            // `wrap.height()` doesn't over-count rows.
            let mut advance = if i < split_len - 1 {
                sim.row_remaining()
            } else {
                shaped_w.min(sim.row_remaining())
            };
            sim.offset += advance;
            // Post-padding on the final row of a padded section: same role
            // as `pre_pad`, on the right side.
            if i == split_len - 1 && padded {
                let post_pad = self.layout.inline_padding.min(sim.row_remaining());
                advance += post_pad;
                sim.offset += post_pad;
            }
            out.push(RowPlacement {
                text: row.text,
                range: row.range,
                shaped_w,
                break_advance,
                advance,
            });
        }
        out
    }

    /// Show source text specified by the given range or override text. In the
    /// override case, clicking the text will select the given range.
    ///
    /// The text must not contain newlines. It doesn't matter if it wraps. It
    /// doesn't have to be a whole line.
    #[allow(clippy::too_many_arguments)]
    pub fn show_override_section(
        &mut self, ui: &mut Ui, top_left: Pos2, wrap: &mut Wrap, range: (Grapheme, Grapheme),
        text_format: Format, override_text: Option<&str>, sense: Sense,
    ) -> Response {
        let text = override_text.unwrap_or(&self.buffer[range]);
        let ppi = self.ctx.pixels_per_point();
        let padded = text_format.background != egui::Color32::TRANSPARENT;
        let sense = if text_format.spoiler { Sense::hover() } else { sense };

        #[cfg(debug_assertions)]
        if text.contains('\n') {
            panic!("show_text_line: text contains newline: {text:?}");
        }

        let font_size = if text_format.superscript || text_format.subscript {
            wrap.row_height * 0.75
        } else {
            wrap.row_height
        };
        let y_offset = if text_format.subscript { wrap.row_height - font_size } else { 0. };
        let color = {
            let [r, g, b, a] = text_format.color.to_array();
            glyphon::Color::rgba(r, g, b, a)
        };

        struct ShapedRow {
            buffer: std::sync::Arc<std::sync::RwLock<glyphon::Buffer>>,
            size: Vec2,
            pos: Pos2,
            rect: Rect,
            range: (Grapheme, Grapheme),
        }

        // Plan and place. The same plan drives `span_section`, so the wrap
        // state ends in the same place either way. Section-level pre/post
        // padding for backgrounds is folded into the placements' advances.
        let plan = self.plan_section(wrap, override_text, range, &text_format);
        let mut shaped: Vec<ShapedRow> = Vec::with_capacity(plan.len());
        for placement in plan {
            wrap.offset += placement.break_advance;
            let buffer = self.upsert_glyphon_buffer_unwrapped(
                &placement.text,
                font_size,
                font_size,
                wrap.width,
                &text_format,
            );
            let size = buffer.read().unwrap().shaped_size(ppi);
            let pos = top_left
                + Vec2::new(
                    wrap.row_offset(),
                    wrap.row() as f32 * (wrap.row_height + wrap.row_spacing) + y_offset,
                );
            let rect = Rect::from_min_size(pos, size);
            wrap.add_range(placement.range);
            wrap.offset += placement.advance;
            shaped.push(ShapedRow { buffer, size, pos, rect, range: placement.range });
        }

        // sense
        let mut response = Response::default();
        for row in &shaped {
            let interact_rect = row
                .rect
                .expand2(Vec2::new(self.layout.inline_padding, self.layout.row_spacing / 2.));
            let id = ui.id().with((row.pos.x.to_bits(), row.pos.y.to_bits()));
            let egui_resp = ui.interact(interact_rect, id, sense);
            response.hovered |= egui_resp.hovered();
            response.clicked |= egui_resp.clicked();
            if sense == Sense::click() {
                self.touch_consuming_rects.push(interact_rect);
            }
        }

        // draw
        let color = if text_format.spoiler && !response.hovered {
            glyphon::Color::rgba(0, 0, 0, 0)
        } else {
            color
        };
        for row in shaped {
            self.galleys.push(GalleyInfo {
                is_override: override_text.is_some(),
                range: row.range,
                buffer: row.buffer.clone(),
                rect: row.rect,
                padded,
            });
            if ui.clip_rect().intersects(row.rect) {
                self.text_areas.push(TextBufferArea::new(
                    row.buffer,
                    row.rect,
                    color,
                    ui.ctx(),
                    ui.clip_rect(),
                ));
                if row.size.x > 0.001 {
                    self.draw_decorations(
                        ui,
                        row.pos,
                        row.size,
                        font_size,
                        &text_format,
                        response.hovered,
                    );
                }
            }
        }

        response
    }

    fn draw_decorations(
        &self, ui: &Ui, pos: Pos2, size: Vec2, font_size: f32, text_format: &Format, hovered: bool,
    ) {
        if text_format.background != egui::Color32::TRANSPARENT {
            let bg_rect =
                Rect::from_min_size(pos, size).expand2(Vec2::splat(self.layout.inline_padding));
            if text_format.spoiler && hovered {
                ui.painter().rect_stroke(
                    bg_rect,
                    2.0,
                    Stroke::new(1.0, text_format.background),
                    egui::epaint::StrokeKind::Inside,
                );
            } else {
                ui.painter().rect(
                    bg_rect,
                    2.0,
                    text_format.background,
                    Stroke::new(1.0, text_format.border),
                    egui::epaint::StrokeKind::Inside,
                );
            }
        }
        let stroke = Stroke::new(1.0, text_format.color);
        let x_range = pos.x..=(pos.x + size.x);
        if text_format.strikethrough {
            ui.painter()
                .hline(x_range.clone(), pos.y + font_size * 0.55, stroke);
        }
        if text_format.underline {
            ui.painter()
                .hline(x_range, pos.y + font_size * 0.95, stroke);
        }
    }
}

pub trait BufferExt {
    fn shaped_size(&self, ppi: f32) -> Vec2;
}

impl BufferExt for glyphon::Buffer {
    fn shaped_size(&self, ppi: f32) -> Vec2 {
        let mut result = Vec2::ZERO;
        for run in self.layout_runs() {
            result.y += self.metrics().line_height;
            if let Some(last_glyph) = run.glyphs.last() {
                result.x = result.x.max(last_glyph.x + last_glyph.w)
            }
        }
        result / ppi
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
