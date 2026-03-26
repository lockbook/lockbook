use std::sync::{Arc, RwLock};

use egui::{Id, Key, Pos2, Rect, Sense, Ui, Vec2};
use glyphon::{Buffer as GlyphonBuffer, Metrics, Shaping, Weight};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::TextBufferArea;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{BufferExt as _, FontFamily, Format};
use crate::tab::markdown_editor::widget::utils::{
    COMPLETION_FONT_SIZE, COMPLETION_LINE_HEIGHT, COMPLETION_MEASURE_WIDTH, COMPLETION_ROW_HEIGHT,
    base_attrs, draw_completion_popup, sans_fmt, to_glyphon,
};

const MAX_RESULTS: usize = 5;
const MIN_QUERY_LEN: usize = 2;
const POPUP_CHROME: f32 = 56.0; // 8 left pad + label + 8 gap + ~40 hint + 8 right pad approximation

#[derive(Default)]
pub struct EmojiCompletions {
    /// Set each frame before process_events so translate_egui_keyboard_event
    /// can check it and swallow arrow/enter keys when the popup is open.
    pub active: bool,
    /// Keyboard-highlighted result index.
    pub selected: usize,
    /// The search term range in the document excluding the opening colon,
    /// e.g. `smil` in `:smil`. Set when active so show_text() can split
    /// rendering and draw the term in the accent color.
    pub search_term_range: Option<(DocCharOffset, DocCharOffset)>,
    /// When Escape is pressed we store the current query string here.
    /// stays hidden while the live query equals this exactly. Typing more characters
    /// changes the query and un-suppresses automatically — no explicit clear needed.
    suppressed: Option<String>,
}

impl EmojiCompletions {
    /// Recomputes whether the popup should be active. Must be called before
    /// process_events so that translate_egui_keyboard_event can consult
    /// self.emoji_completions.active when deciding whether to swallow keys.
    pub fn update_active_state(&mut self, buffer: &Buffer) {
        self.active = false;
        self.search_term_range = None;

        let Some(range) = detect_query(buffer) else { return };
        let Some(query) = query_from_range(buffer, range) else { return };
        if self.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }
        // A complete shortcode `:smile:` means the cursor navigated into existing syntax.
        // Only show the popup when the token is still being typed (no closing colon yet).
        let raw = &buffer[range];
        if raw.starts_with(':') && raw.ends_with(':') && raw.len() > 2 {
            return;
        }

        let has_results = !search(&query).is_empty();
        self.active = has_results;
        // +1 to skip the opening colon
        self.search_term_range =
            if has_results { Some((DocCharOffset(range.0.0 + 1), range.1)) } else { None };
    }
}

/// Returns the range `(start, end)` of the shortcode token under the cursor,
/// including the opening `:` and the closing `:` if present. For example,
/// with the cursor anywhere inside `:smile:` this returns the range covering
/// that entire token. The caller reads `buffer[range]` and strips colons to
/// get the search query.
fn detect_query(buffer: &Buffer) -> Option<(DocCharOffset, DocCharOffset)> {
    let selection = buffer.current.selection;

    // Only trigger for a collapsed cursor — an active selection means the user
    // is not in the middle of typing a shortcode.
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let text = buffer.current.text.to_string();
    let chars: Vec<char> = text.chars().collect();

    // Scan backward to find the opening ':'.
    let mut i = cursor_idx;
    let colon_idx;
    loop {
        if i == 0 {
            // Walked all the way to the document start without finding ':'.
            return None;
        }
        i -= 1;

        let c = chars[i];

        if c == ':' {
            // Don't trigger when ':' immediately follows a word character, so
            // tokens like "http://", "e.g.:", or "v1.0:" are ignored.
            if i > 0 && chars[i - 1].is_alphanumeric() {
                return None;
            }
            colon_idx = i;
            break;
        }

        // Hit whitespace before finding ':' — not inside a shortcode token.
        if c.is_whitespace() {
            return None;
        }

        // Cap the backward scan to avoid O(n) work on long lines without ':'.
        if cursor_idx - i > 30 {
            return None;
        }
    }

    // Scan forward from the cursor to get the rest of the word. Stop at
    // whitespace, a closing ':', or the scan limit. This means the cursor can
    // be anywhere inside `:smile:` and the full shortcode is still the query.
    let mut j = cursor_idx;
    while j < chars.len() && !chars[j].is_whitespace() && j - cursor_idx <= 30 {
        if chars[j] == ':' {
            // Found a closing colon — include it in the replacement range.
            j += 1;
            break;
        }
        j += 1;
    }

    Some((DocCharOffset(colon_idx), DocCharOffset(j)))
}

fn query_from_range(buffer: &Buffer, range: (DocCharOffset, DocCharOffset)) -> Option<String> {
    let raw = &buffer[range];
    // Strip surrounding colons to get the bare shortcode name.
    let query = raw.trim_matches(':').to_string();
    if query.len() < MIN_QUERY_LEN {
        return None;
    }
    // A space inside means ':' was prose punctuation, not a shortcode opener.
    if query.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    Some(query)
}

/// Returns a bool per shortcode character: true if that character was consumed
/// by the subsequence match.
fn match_positions(query: &str, shortcode: &str) -> Vec<bool> {
    let mut result = vec![false; shortcode.chars().count()];
    let mut qi = query.chars().peekable();
    for (i, sc) in shortcode.chars().enumerate() {
        if qi.peek() == Some(&sc) {
            result[i] = true;
            qi.next();
        }
    }
    result
}

/// True if every character of `needle` appears in `haystack` in order.
/// Allows "smil" to match "smiling_face", "poop" to match "poop", etc.
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut haystack_chars = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in &mut haystack_chars {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// Splits the label into (text, is_bold) spans. Characters in the shortcode
/// that matched the query are bold; chrome and unmatched chars are normal.
/// Consecutive chars with the same weight are grouped into one span.
fn build_label_spans(emoji_str: &str, shortcode: &str, query: &str) -> Vec<(String, bool)> {
    let flags = match_positions(query, shortcode);
    let mut spans: Vec<(String, bool)> = vec![(format!("{} :", emoji_str), false)];
    let mut cur = String::new();
    let mut cur_bold = false;
    for (ch, &matched) in shortcode.chars().zip(flags.iter()) {
        if matched != cur_bold && !cur.is_empty() {
            spans.push((cur.clone(), cur_bold));
            cur.clear();
        }
        cur_bold = matched;
        cur.push(ch);
    }
    if !cur.is_empty() {
        spans.push((cur, cur_bold));
    }
    spans.push((":".to_string(), false));
    spans
}

/// Returns the best matching shortcode for `emoji` given `query`, along with
/// its tier (0 = exact, 1 = prefix, 2 = subsequence). Returns `None` if no
/// shortcode matches at all.
fn best_shortcode<'a>(emoji: &'a emojis::Emoji, query: &str) -> Option<(&'a str, u8)> {
    emoji
        .shortcodes()
        .filter(|s| is_subsequence(query, s))
        .map(|s| {
            let tier = if s == query {
                0
            } else if s.starts_with(query) {
                1
            } else {
                2
            };
            (s, tier)
        })
        .min_by_key(|&(s, tier)| (tier, s.len()))
}

fn search(query: &str) -> Vec<&'static emojis::Emoji> {
    let mut results: Vec<(&'static emojis::Emoji, u8, usize)> = emojis::iter()
        .filter_map(|e| {
            let (s, tier) = best_shortcode(e, query)?;
            Some((e, tier, s.len()))
        })
        .collect();
    results.sort_by_key(|&(_, tier, len)| (tier, len));
    results
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(e, _, _)| e)
        .collect()
}

fn matching_shortcode<'a>(emoji: &'a emojis::Emoji, query: &str) -> &'a str {
    best_shortcode(emoji, query)
        .map(|(s, _)| s)
        .or_else(|| emoji.shortcodes().next())
        .unwrap_or("")
}

impl Editor {
    pub fn show_emoji_completions(&mut self, ui: &mut Ui) {
        if self.readonly || !self.emoji_completions.active {
            return;
        }

        let Some((colon_offset, replace_end)) = detect_query(&self.buffer) else {
            return;
        };
        let Some(query) = query_from_range(&self.buffer, (colon_offset, replace_end)) else {
            return;
        };

        if self.emoji_completions.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }

        let results = search(&query);
        if results.is_empty() {
            return;
        }

        // Clamp selection in case results shrank (e.g. the user typed more characters).
        self.emoji_completions.selected = self.emoji_completions.selected.min(results.len() - 1);

        // Anchor the popup at the opening colon so it doesn't shift right as the user types.
        let Some([cursor_top, cursor_bot]) = self.cursor_line(colon_offset) else {
            return;
        };

        // Escape is checked outside the focused guard so it always fires.
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.emoji_completions.suppressed = Some(query);
            return;
        }

        if self.focused(ui.ctx()) {
            // Up/Down were swallowed by translate_egui_keyboard_event so the document
            // cursor didn't move; read them here to update the highlighted row.
            ui.input(|i| {
                if i.key_pressed(Key::ArrowUp) && self.emoji_completions.selected > 0 {
                    self.emoji_completions.selected -= 1;
                }
                if i.key_pressed(Key::ArrowDown)
                    && self.emoji_completions.selected + 1 < results.len()
                {
                    self.emoji_completions.selected += 1;
                }
            });

            // Enter picks the highlighted row (also swallowed by translate_egui_keyboard_event).
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                let idx = self.emoji_completions.selected;
                self.apply_emoji_completion(
                    colon_offset,
                    replace_end,
                    matching_shortcode(results[idx], &query),
                );
                return;
            }

            let mut chosen = None;
            ui.input(|i| {
                let modifier =
                    if cfg!(target_os = "macos") { i.modifiers.command } else { i.modifiers.ctrl };
                for (idx, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5]
                    .iter()
                    .enumerate()
                    .take(results.len())
                {
                    if i.key_pressed(*key) && modifier {
                        chosen = Some(idx);
                    }
                }
            });
            if let Some(idx) = chosen {
                self.apply_emoji_completion(
                    colon_offset,
                    replace_end,
                    matching_shortcode(results[idx], &query),
                );
                return;
            }
        }

        let fmt = sans_fmt();
        let ppi = ui.ctx().pixels_per_point();

        // Precompute labels once — reused for measurement, background highlights, and rendering.
        let labels: Vec<(&str, String)> = results
            .iter()
            .map(|emoji| {
                let shortcode = matching_shortcode(emoji, &query);
                let label = format!("{} :{shortcode}:", emoji.as_str());
                (shortcode, label)
            })
            .collect();

        // Measure each label at an unconstrained width to get its natural (unwrapped) size,
        // then derive popup_width from the widest result.
        let max_label_width = labels
            .iter()
            .map(|(_, label)| {
                let buf = self.upsert_glyphon_buffer(
                    label,
                    COMPLETION_FONT_SIZE,
                    COMPLETION_LINE_HEIGHT,
                    COMPLETION_MEASURE_WIDTH,
                    &fmt,
                );
                let buf = buf.read().unwrap();
                buf.shaped_size(ppi).x
            })
            .fold(0.0_f32, f32::max);
        let popup_width = max_label_width + POPUP_CHROME;

        let popup_height = results.len() as f32 * COMPLETION_ROW_HEIGHT;
        let screen_rect = ui.ctx().screen_rect();
        let popup_y = if cursor_top.y - popup_height >= screen_rect.min.y {
            cursor_top.y - popup_height // above cursor
        } else {
            cursor_bot.y // below cursor
        };
        let popup_rect = Rect::from_min_size(
            Pos2::new(cursor_top.x, popup_y),
            Vec2::new(popup_width, popup_height),
        );

        // All rows are uniform height since the popup is sized to fit without wrapping.
        let row_rects: Vec<Rect> = (0..results.len())
            .map(|i| {
                Rect::from_min_size(
                    Pos2::new(
                        popup_rect.min.x,
                        popup_rect.min.y + i as f32 * COMPLETION_ROW_HEIGHT,
                    ),
                    Vec2::new(popup_width, COMPLETION_ROW_HEIGHT),
                )
            })
            .collect();

        // Allocate interaction rects before drawing — click detection doesn't depend on draw order.
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked: Option<usize> = None;
        for (idx, _) in results.iter().enumerate() {
            let resp = ui.interact(row_rects[idx], Id::new("emoji_item").with(idx), Sense::click());
            if resp.clicked() {
                clicked = Some(idx);
            }
        }

        // Draw frame and row backgrounds using the egui painter (plain shapes, no text).
        draw_completion_popup(
            ui.painter(),
            popup_rect,
            &row_rects,
            self.emoji_completions.selected,
            hover_pos,
            ui.visuals().extreme_bg_color,
            ui.visuals().widgets.hovered.bg_fill,
            ui.visuals().selection.bg_fill.gamma_multiply(0.3),
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        );

        // Build glyphon text buffers. Glyphon (not egui's text system) is required
        // so that emoji codepoints are shaped and rendered correctly.
        let text_color = ui.visuals().text_color();
        let hint_color = ui.visuals().weak_text_color();
        let clip_rect = ui.clip_rect();

        let hint_fmt = Format {
            family: FontFamily::Sans,
            bold: false,
            italic: false,
            color: hint_color,
            underline: false,
            strikethrough: false,
            background: egui::Color32::TRANSPARENT,
            border: egui::Color32::TRANSPARENT,
            spoiler: false,
            superscript: false,
            subscript: false,
        };

        // Measure hint width once — all "^N" strings are the same width.
        let hint_text_width = {
            let sample = format!("^{}", results.len());
            let buf = self.upsert_glyphon_buffer(
                &sample,
                COMPLETION_FONT_SIZE - 2.0,
                COMPLETION_LINE_HEIGHT,
                COMPLETION_MEASURE_WIDTH,
                &hint_fmt,
            );
            let buf = buf.read().unwrap();
            buf.shaped_size(ppi).x
        };

        let normal_color = to_glyphon(text_color);

        let mut text_areas: Vec<TextBufferArea> = Vec::new();

        for (idx, (shortcode, _label)) in labels.iter().enumerate() {
            let rect = row_rects[idx];
            let text_top = rect.min.y + 4.0;

            let label_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, text_top),
                Vec2::new(max_label_width, COMPLETION_LINE_HEIGHT),
            );

            // Build a rich-text buffer with bold shortcode text.
            let spans = build_label_spans(results[idx].as_str(), shortcode, &query);
            let font_size_px = COMPLETION_FONT_SIZE * ppi;
            let line_height_px = COMPLETION_LINE_HEIGHT * ppi;
            let mut b = GlyphonBuffer::new(
                &mut self.font_system.lock().unwrap(),
                Metrics::new(font_size_px, line_height_px),
            );
            b.set_size(
                &mut self.font_system.lock().unwrap(),
                Some(COMPLETION_MEASURE_WIDTH * ppi),
                None,
            );
            let default_attrs = base_attrs();
            b.set_rich_text(
                &mut self.font_system.lock().unwrap(),
                spans.iter().map(|(text, bold)| {
                    let attrs = if *bold {
                        base_attrs().color(normal_color).weight(Weight::BOLD)
                    } else {
                        base_attrs().color(normal_color)
                    };
                    (text.as_str(), attrs)
                }),
                &default_attrs,
                Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(&mut self.font_system.lock().unwrap(), false);
            let label_buf = Arc::new(RwLock::new(b));

            text_areas.push(TextBufferArea::new(
                label_buf,
                label_rect,
                normal_color,
                ui.ctx(),
                clip_rect,
            ));

            let hint = format!("{}{}", if cfg!(target_os = "macos") { "⌘" } else { "^" }, idx + 1);
            // Position rect so its right edge sits at popup right - 8, making the
            // left-aligned glyphon text appear right-aligned within the popup.
            let hint_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - hint_text_width - 8.0, text_top),
                Vec2::new(hint_text_width, COMPLETION_LINE_HEIGHT),
            );
            let hint_buf = self.upsert_glyphon_buffer(
                &hint,
                COMPLETION_FONT_SIZE - 2.0,
                COMPLETION_LINE_HEIGHT,
                COMPLETION_MEASURE_WIDTH,
                &hint_fmt,
            );
            text_areas.push(TextBufferArea::new(
                hint_buf,
                hint_rect,
                to_glyphon(hint_color),
                ui.ctx(),
                clip_rect,
            ));
        }

        // Submit as a second callback after the editor's main text callback so the
        // popup text composites on top of the document text.
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.max_rect(),
                crate::GlyphonRendererCallback::new(text_areas),
            ));

        if let Some(idx) = clicked {
            self.apply_emoji_completion(colon_offset, replace_end, labels[idx].0);
        }
    }

    fn apply_emoji_completion(
        &mut self, colon_offset: DocCharOffset, cursor: DocCharOffset, shortcode: &str,
    ) {
        self.event.internal_events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::DocCharOffset(colon_offset),
                end: Location::DocCharOffset(cursor),
            },
            text: format!(":{}:", shortcode),
            advance_cursor: true,
        });
        self.emoji_completions.selected = 0;
        self.emoji_completions.suppressed = None;
    }
}
