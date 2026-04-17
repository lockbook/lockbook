use egui::{Id, Key, Pos2, Rect, Sense, Ui, Vec2};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::TextBufferArea;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::{Paragraphs, RangesExt as _};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::widgets::GlyphonLabel;

const MAX_RESULTS: usize = 5;
const MIN_QUERY_LEN: usize = 2;
const POPUP_PADDING: f32 = 24.0; // 8 left + 8 gap + 8 right

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
    pub fn update_active_state(&mut self, buffer: &Buffer, inline_paragraphs: &Paragraphs) {
        self.active = false;
        self.search_term_range = None;

        if inline_paragraphs
            .find_containing(buffer.current.selection.1, true, true)
            .is_empty()
        {
            // not in an inline paragraph; wherever the cursor is rn, inlines do not apply
            return;
        }

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
            if has_results { Some((range.start() + 1, range.end())) } else { None };
    }
}

/// Returns the range `(start, end)` of the shortcode token under the cursor,
/// including the opening `:` and the closing `:` if present. For example,
/// with the cursor anywhere inside `:smile:` this returns the range covering
/// that entire token. The caller reads `buffer[range]` and strips colons to
/// get the search query.
/// Returns the grapheme `&str` at the given char offset.
fn grapheme_at(buffer: &Buffer, i: usize) -> &str {
    &buffer[(DocCharOffset(i), DocCharOffset(i + 1))]
}

fn detect_query(buffer: &Buffer) -> Option<(DocCharOffset, DocCharOffset)> {
    let selection = buffer.current.selection;

    // Only trigger for a collapsed cursor — an active selection means the user
    // is not in the middle of typing a shortcode.
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let len = buffer.current.segs.last_cursor_position().0;

    // Scan backward to find the opening ':'.
    let mut i = cursor_idx;
    let colon_idx;
    loop {
        if i == 0 {
            // Walked all the way to the document start without finding ':'.
            return None;
        }
        i -= 1;

        let g = grapheme_at(buffer, i);

        if g == ":" {
            // Don't trigger when ':' immediately follows a word character, so
            // tokens like "http://", "e.g.:", or "v1.0:" are ignored.
            if i > 0
                && grapheme_at(buffer, i - 1)
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric())
            {
                return None;
            }
            colon_idx = i;
            break;
        }

        // Hit whitespace before finding ':' — not inside a shortcode token.
        if g.chars().next().is_some_and(|c| c.is_whitespace()) {
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
    while j < len
        && !grapheme_at(buffer, j)
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace())
        && j - cursor_idx <= 30
    {
        if grapheme_at(buffer, j) == ":" {
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

        let Some((colon_offset, replace_end)) = detect_query(&self.renderer.buffer) else {
            return;
        };
        let Some(query) = query_from_range(&self.renderer.buffer, (colon_offset, replace_end))
        else {
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
                let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) {
                    i.modifiers.command
                } else {
                    i.modifiers.ctrl
                };
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

        // -- Measure content -------------------------------------------------------
        let text_color = ui.visuals().text_color();
        let hint_color = ui.visuals().weak_text_color();
        let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) { "⌘" } else { "^" };

        let shortcodes: Vec<&str> = results
            .iter()
            .map(|emoji| matching_shortcode(emoji, &query))
            .collect();

        let hints: Vec<String> = if self.phone_mode {
            Vec::new()
        } else {
            (0..results.len())
                .map(|i| format!("{}{}", modifier, i + 1))
                .collect()
        };

        let spans: Vec<Vec<(String, bool)>> = results
            .iter()
            .zip(shortcodes.iter())
            .map(|(emoji, &shortcode)| build_label_spans(emoji.as_str(), shortcode, &query))
            .collect();

        let max_width = spans
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let span_refs: Vec<(&str, bool)> =
                    s.iter().map(|(t, b)| (t.as_str(), *b)).collect();
                let mut label = GlyphonLabel::new_rich(span_refs, text_color)
                    .font_size(self.renderer.layout.completion_font_size)
                    .line_height(self.renderer.layout.completion_line_height);
                if let Some(hint) = hints.get(i) {
                    label = label.hint(hint, hint_color);
                }
                label.measure(ui).x
            })
            .fold(0.0_f32, f32::max);

        // -- Position popup --------------------------------------------------------
        let popup_width = max_width + POPUP_PADDING;
        let popup_height = results.len() as f32 * self.renderer.layout.completion_row_height;
        let screen_rect = ui.ctx().screen_rect();
        let popup_y = if cursor_top.y - popup_height >= screen_rect.min.y {
            cursor_top.y - popup_height
        } else {
            cursor_bot.y
        };
        let popup_rect = Rect::from_min_size(
            Pos2::new(cursor_top.x, popup_y),
            Vec2::new(popup_width, popup_height),
        );
        self.renderer.touch_consuming_rects.push(popup_rect);

        let row_rects: Vec<Rect> = (0..results.len())
            .map(|i| {
                Rect::from_min_size(
                    Pos2::new(
                        popup_rect.min.x,
                        popup_rect.min.y + i as f32 * self.renderer.layout.completion_row_height,
                    ),
                    Vec2::new(popup_width, self.renderer.layout.completion_row_height),
                )
            })
            .collect();

        // -- Interaction -----------------------------------------------------------
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked: Option<usize> = None;
        for (idx, _) in results.iter().enumerate() {
            let resp = ui.interact(row_rects[idx], Id::new("emoji_item").with(idx), Sense::click());
            if resp.clicked() {
                clicked = Some(idx);
            }
        }

        // -- Draw backgrounds ------------------------------------------------------
        self.renderer.draw_completion_popup(
            ui,
            popup_rect,
            &row_rects,
            self.emoji_completions.selected,
            hover_pos,
        );

        // -- Render text -----------------------------------------------------------
        let clip_rect = ui.clip_rect();
        let mut text_areas: Vec<TextBufferArea> = Vec::new();

        for (idx, spans) in spans.iter().enumerate() {
            let rect = row_rects[idx];
            let text_top = rect.min.y + 4.0;
            let content_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, text_top),
                Vec2::new(popup_width - 16.0, self.renderer.layout.completion_line_height),
            );

            let span_refs: Vec<(&str, bool)> =
                spans.iter().map(|(t, b)| (t.as_str(), *b)).collect();
            let mut label = GlyphonLabel::new_rich(span_refs, text_color)
                .font_size(self.renderer.layout.completion_font_size)
                .line_height(self.renderer.layout.completion_line_height);
            if let Some(hint) = hints.get(idx) {
                label = label.hint(hint, hint_color);
            }
            let shaped = label.build(ui.ctx());
            text_areas.extend(shaped.text_areas(content_rect, ui.ctx(), clip_rect));
        }

        // Submit after the editor's main text callback so the popup composites on top.
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.max_rect(),
                crate::GlyphonRendererCallback::new(text_areas),
            ));

        // -- Apply clicked result --------------------------------------------------
        if let Some(idx) = clicked {
            self.apply_emoji_completion(colon_offset, replace_end, shortcodes[idx]);
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
