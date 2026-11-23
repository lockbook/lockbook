//! Shared 3×8 numbered word grid used across:
//! 1. Sign in (24 editable slots; focused visible with BIP-39 prefix ghost
//!    completion and auto-advance on unique complete words; unfocused valid
//!    cells mask to `•••••`, while invalid cells stay readable in `t.danger()`).
//! 2. Secret key backup (display all 24 masked or revealed).

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{
    Align, Align2, Color32, Context, Event, Frame, Id, Key, Layout, Modifiers, Rect, Sense, Stroke,
    StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2,
};
use lb::model::account::{PhrasePrefixMatch, is_valid_phrase_word, match_phrase_prefix};

use crate::components::{Radius, STROKE_HAIRLINE, Theme, TypeRole, ui_width};

pub const GRID_COLS: usize = 3;
pub const GRID_ROWS: usize = 8;
pub const WORD_COUNT: usize = GRID_COLS * GRID_ROWS;

const CELL_H: f32 = 34.0;
const CELL_GAP: f32 = 8.0;
const MASK_DOTS: &str = "•••••";

/// Exact height of the 3×8 word grid — deterministic ahead of layout.
pub const fn grid_height() -> f32 {
    GRID_ROWS as f32 * CELL_H + (GRID_ROWS - 1) as f32 * CELL_GAP
}

/// Outcome of pasting text into the 24-word Sign in grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasteOutcome {
    /// Text was split into `count` words starting at `start_slot`.
    Words { start_slot: usize, count: usize, all_filled: bool },
    /// Text looks like a compact base64 account key rather than a word phrase.
    CompactKey(String),
    /// Single short word (handled as normal input).
    SingleWord(String),
}

/// Split a pasted phrase (space / comma / newline separated, optional `1.` / `1)` prefixes).
pub fn parse_pasted_words(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip standalone numbering tokens like "1.", "12.", "3)", "#4".
        let stripped_prefix = trimmed.trim_start_matches('#');
        if stripped_prefix
            .trim_end_matches(['.', ')', ':'])
            .chars()
            .all(|c| c.is_ascii_digit())
            && !stripped_prefix.is_empty()
        {
            continue;
        }
        // Strip inline numbering like "1.abandon" or "12)turkey".
        let word = if let Some(idx) = trimmed.find(['.', ')', ':']) {
            let head = trimmed[..idx].trim_start_matches('#');
            if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) {
                &trimmed[idx + 1..]
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        let word = word.trim().to_ascii_lowercase();
        if !word.is_empty() {
            out.push(word);
        }
    }
    out
}

/// True when a single whitespace-stripped token looks like a compact Lockbook key
/// (base64-encoded account key, >= 40 chars, not an English BIP-39 word).
pub fn looks_like_compact_key(raw: &str) -> bool {
    let compact: String = raw.split_whitespace().collect();
    if compact.len() < 40 {
        return false;
    }
    let words = parse_pasted_words(raw);
    if words.len() >= 2 && words.iter().filter(|w| is_valid_phrase_word(w)).count() >= 2 {
        return false;
    }
    compact
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
}

/// Apply pasted text into the 24-word buffer at `focused_slot`.
pub fn apply_paste_to_words(
    words: &mut [String; WORD_COUNT], focused_slot: usize, raw: &str,
) -> PasteOutcome {
    if looks_like_compact_key(raw) {
        let compact: String = raw.split_whitespace().collect();
        return PasteOutcome::CompactKey(compact);
    }
    let parsed = parse_pasted_words(raw);
    if parsed.len() >= 2 {
        let start = if parsed.len() >= WORD_COUNT { 0 } else { focused_slot.min(WORD_COUNT - 1) };
        let mut placed = 0usize;
        for (offset, word) in parsed.into_iter().enumerate() {
            let idx = start + offset;
            if idx >= WORD_COUNT {
                break;
            }
            words[idx] = word;
            placed += 1;
        }
        let all_filled = words.iter().all(|w| !w.trim().is_empty());
        PasteOutcome::Words { start_slot: start, count: placed, all_filled }
    } else if let Some(single) = parsed.into_iter().next() {
        PasteOutcome::SingleWord(single)
    } else {
        PasteOutcome::SingleWord(String::new())
    }
}

/// Result emitted each frame by the interactive 24-word Sign in grid.
#[derive(Default)]
pub struct ImportGridResult {
    pub changed: bool,
    pub compact_key_pasted: Option<String>,
    pub pasted_full_phrase: bool,
}

fn slot_edit_id(grid_id: Id, idx: usize) -> Id {
    grid_id.with(("slot", idx))
}

fn pending_caret_key(edit_id: Id) -> Id {
    edit_id.with("pending_caret")
}

fn set_edit_caret(ctx: &Context, edit_id: Id, char_pos: usize) {
    let mut state = TextEditState::load(ctx, edit_id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(char_pos))));
    state.store(ctx, edit_id);
    ctx.data_mut(|d| d.insert_temp(pending_caret_key(edit_id), char_pos));
}

fn apply_pending_caret(ctx: &Context, edit_id: Id) {
    if let Some(pos) = ctx.data_mut(|d| d.remove_temp::<usize>(pending_caret_key(edit_id))) {
        let mut state = TextEditState::load(ctx, edit_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
        state.store(ctx, edit_id);
    }
}

/// Schedule placing the caret at the end of a Sign in phrase slot.
pub fn request_import_slot_caret_end(ctx: &Context, idx: usize, text: &str) {
    let id = slot_edit_id(Id::unique("onboard_import_grid"), idx);
    set_edit_caret(ctx, id, text.chars().count());
}

/// Consume plain `Backspace`, `Alt+Backspace`, `Cmd+Backspace`, or `Ctrl+Backspace`.
fn consume_any_backspace(ui: &Ui) -> bool {
    ui.input_mut(|i| {
        i.consume_key(Modifiers::NONE, Key::Backspace)
            || i.consume_key(Modifiers::ALT, Key::Backspace)
            || i.consume_key(Modifiers::COMMAND, Key::Backspace)
            || i.consume_key(Modifiers::CTRL, Key::Backspace)
            || i.consume_key(Modifiers::MAC_CMD, Key::Backspace)
    })
}

/// Current collapsed caret char index inside `edit_id` (None if text is selected).
fn collapsed_caret_pos(ctx: &Context, edit_id: Id) -> Option<usize> {
    let range = TextEditState::load(ctx, edit_id)?.cursor.char_range()?;
    (range.primary.index == range.secondary.index).then_some(range.primary.index.into())
}

/// True when `words` has all 24 slots filled with valid English BIP-39 words.
pub fn all_words_valid(words: &[String; WORD_COUNT]) -> bool {
    words.iter().all(|w| is_valid_phrase_word(w))
}

/// Next slot after `after` (wrapping) that is empty or not a valid BIP-39 word.
pub fn next_incomplete_or_invalid_slot(
    words: &[String; WORD_COUNT], after: usize,
) -> Option<usize> {
    for step in 1..=WORD_COUNT {
        let idx = (after + step) % WORD_COUNT;
        if !is_valid_phrase_word(&words[idx]) {
            return Some(idx);
        }
    }
    None
}

/// Interactive 3×8 Sign in grid.
///
/// - Focused cell: editable monospace input with inline BIP-39 unique-prefix
///   ghost completion, live zero-match danger border, and auto-advance once a
///   complete non-prefix BIP-39 word is typed.
/// - Unfocused valid cell: masked `•••••`. Unfocused invalid cell stays visible
///   in `t.danger()` so typos can be read and fixed at a glance.
/// - `Tab` / `Space` / `,` accept unique completion and advance; `⇧Tab` steps
///   back; `Backspace` / `Alt+Backspace` / `Cmd+Backspace` in an empty cell jump
///   to the end of the previous cell so spamming clears across slots.
/// - Arrow keys (`↑`/`↓`, and `←`/`→` at text boundaries) navigate the 3×8 grid.
/// - Multi-word or compact-key `⌘V` paste works anywhere on the screen.
pub fn show_import_grid(
    ui: &mut Ui, t: &Theme, grid_id: Id, words: &mut [String; WORD_COUNT], focused: &mut usize,
    request_focus: &mut bool,
) -> ImportGridResult {
    let mut out = ImportGridResult::default();
    let cur_focus = (*focused).min(WORD_COUNT - 1);
    let focused_id = slot_edit_id(grid_id, cur_focus);
    let has_focus = ui.memory(|m| m.has_focus(focused_id));

    // 1. Intercept multi-word or compact-key paste anywhere on the Sign in grid
    // (even if no cell currently holds focus).
    let mut intercepted_paste: Option<String> = None;
    ui.input_mut(|i| {
        let mut idx_to_remove = None;
        for (idx, ev) in i.events.iter().enumerate() {
            if let Event::Paste(text) = ev {
                if looks_like_compact_key(text) || parse_pasted_words(text).len() >= 2 {
                    intercepted_paste = Some(text.clone());
                    idx_to_remove = Some(idx);
                    break;
                }
            }
        }
        if let Some(idx) = idx_to_remove {
            i.events.remove(idx);
        }
    });

    if let Some(pasted) = intercepted_paste {
        match apply_paste_to_words(words, cur_focus, &pasted) {
            PasteOutcome::CompactKey(k) => {
                out.compact_key_pasted = Some(k);
                out.changed = true;
            }
            PasteOutcome::Words { start_slot, count, all_filled } => {
                let next = (start_slot + count).min(WORD_COUNT - 1);
                *focused = next;
                set_edit_caret(ui.ctx(), slot_edit_id(grid_id, next), words[next].chars().count());
                *request_focus = true;
                out.changed = true;
                out.pasted_full_phrase = all_filled && all_words_valid(words);
            }
            PasteOutcome::SingleWord(w) => {
                words[cur_focus] = w;
                out.changed = true;
            }
        }
    }

    // 2. Intercept navigation & spam-backspace keys when a slot holds focus.
    if has_focus {
        let cur_len = words[cur_focus].chars().count();
        let caret = collapsed_caret_pos(ui.ctx(), focused_id);

        let backspace_empty =
            words[cur_focus].is_empty() && cur_focus > 0 && consume_any_backspace(ui);

        if backspace_empty {
            let prev = cur_focus - 1;
            *focused = prev;
            set_edit_caret(ui.ctx(), slot_edit_id(grid_id, prev), words[prev].chars().count());
            *request_focus = true;
        } else {
            let (shift_tab, tab, up, down, left_edge, right_edge) = ui.input_mut(|i| {
                let st = i.consume_key(Modifiers::SHIFT, Key::Tab);
                let tb = !st && i.consume_key(Modifiers::NONE, Key::Tab);
                let u = cur_focus >= GRID_COLS && i.consume_key(Modifiers::NONE, Key::ArrowUp);
                let d = cur_focus + GRID_COLS < WORD_COUNT
                    && i.consume_key(Modifiers::NONE, Key::ArrowDown);
                let l = cur_focus > 0
                    && caret == Some(0)
                    && i.consume_key(Modifiers::NONE, Key::ArrowLeft);
                let r = cur_focus + 1 < WORD_COUNT
                    && caret == Some(cur_len)
                    && i.consume_key(Modifiers::NONE, Key::ArrowRight);
                (st, tb, u, d, l, r)
            });

            if shift_tab {
                let prev = (cur_focus + WORD_COUNT - 1) % WORD_COUNT;
                *focused = prev;
                set_edit_caret(ui.ctx(), slot_edit_id(grid_id, prev), words[prev].chars().count());
                *request_focus = true;
            } else if tab {
                if let PhrasePrefixMatch::Unique(full) = match_phrase_prefix(&words[cur_focus]) {
                    if words[cur_focus] != full {
                        words[cur_focus] = full.to_owned();
                        out.changed = true;
                    }
                }
                let next = (cur_focus + 1) % WORD_COUNT;
                *focused = next;
                set_edit_caret(ui.ctx(), slot_edit_id(grid_id, next), words[next].chars().count());
                *request_focus = true;
            } else if up {
                let target = cur_focus - GRID_COLS;
                *focused = target;
                set_edit_caret(
                    ui.ctx(),
                    slot_edit_id(grid_id, target),
                    words[target].chars().count(),
                );
                *request_focus = true;
            } else if down {
                let target = cur_focus + GRID_COLS;
                *focused = target;
                set_edit_caret(
                    ui.ctx(),
                    slot_edit_id(grid_id, target),
                    words[target].chars().count(),
                );
                *request_focus = true;
            } else if left_edge {
                let prev = cur_focus - 1;
                *focused = prev;
                set_edit_caret(ui.ctx(), slot_edit_id(grid_id, prev), words[prev].chars().count());
                *request_focus = true;
            } else if right_edge {
                let next = cur_focus + 1;
                *focused = next;
                set_edit_caret(ui.ctx(), slot_edit_id(grid_id, next), 0);
                *request_focus = true;
            }
        }
    }

    let total_w = ui_width(ui).max(1.0);
    let cell_w = ((total_w - CELL_GAP * (GRID_COLS as f32 - 1.0)) / GRID_COLS as f32).max(60.0);
    let total_h = GRID_ROWS as f32 * CELL_H + (GRID_ROWS as f32 - 1.0) * CELL_GAP;
    let (grid_rect, _) = ui.allocate_exact_size(vec2(total_w, total_h), Sense::hover());

    let mut next_focus: Option<(usize, usize)> = None;

    for row in 0..GRID_ROWS {
        for col in 0..GRID_COLS {
            let idx = row * GRID_COLS + col;
            let x = grid_rect.left() + col as f32 * (cell_w + CELL_GAP);
            let y = grid_rect.top() + row as f32 * (CELL_H + CELL_GAP);
            let cell = Rect::from_min_size(pos2(x, y), vec2(cell_w, CELL_H));
            let edit_id = slot_edit_id(grid_id, idx);

            let is_active = *focused == idx;
            let trimmed = words[idx].trim();
            let is_filled = !trimmed.is_empty();
            let prefix_match = match_phrase_prefix(trimmed);
            let is_invalid = if is_active {
                matches!(prefix_match, PhrasePrefixMatch::None)
            } else {
                is_filled && !is_valid_phrase_word(trimmed)
            };

            let (fill, stroke_color, stroke_w) = if is_active && is_invalid {
                (t.neutral_bg(), t.danger(), 1.5)
            } else if is_active {
                (t.neutral_bg(), t.accent(), 1.5)
            } else if is_invalid {
                (t.neutral_bg_secondary(), t.danger(), 1.25)
            } else {
                (t.neutral_bg_secondary(), t.neutral(), STROKE_HAIRLINE)
            };

            ui.painter().rect(
                cell,
                Radius::Control.corner(),
                fill,
                Stroke::new(stroke_w, stroke_color),
                StrokeKind::Inside,
            );

            // Number prefix (right-aligned in 18px column).
            let num_color = if is_invalid {
                t.danger()
            } else if is_active {
                t.accent()
            } else {
                t.neutral_fg_secondary()
            };
            let num_rect =
                Rect::from_min_size(pos2(cell.left() + 8.0, cell.top()), vec2(18.0, CELL_H));
            ui.painter().text(
                pos2(num_rect.right(), cell.center().y),
                Align2::RIGHT_CENTER,
                format!("{}", idx + 1),
                TypeRole::Mono.font_id(),
                num_color,
            );

            let text_rect = Rect::from_min_max(
                pos2(num_rect.right() + 8.0, cell.top()),
                pos2(cell.right() - 8.0, cell.bottom()),
            );

            if is_active {
                apply_pending_caret(ui.ctx(), edit_id);
                let before = words[idx].clone();
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(text_rect)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                let resp = child.add(
                    TextEdit::singleline(&mut words[idx])
                        .id(edit_id)
                        .frame(Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .min_size(text_rect.size())
                        .align(Align2::LEFT_CENTER)
                        .font(TypeRole::Mono.font_id())
                        .text_color(t.neutral_fg())
                        .desired_width(text_rect.width())
                        .lock_focus(true),
                );

                // Inline ghost completion when typed prefix uniquely identifies one BIP-39 word.
                if let PhrasePrefixMatch::Unique(full) = match_phrase_prefix(&words[idx]) {
                    let typed_len = words[idx].len();
                    if full.len() > typed_len && full.starts_with(words[idx].as_str()) {
                        let suffix = &full[typed_len..];
                        let typed_w = ui
                            .painter()
                            .layout_no_wrap(
                                words[idx].clone(),
                                TypeRole::Mono.font_id(),
                                Color32::PLACEHOLDER,
                            )
                            .size()
                            .x;
                        let gx = text_rect.left() + typed_w;
                        if gx < text_rect.right() - 4.0 {
                            ui.painter()
                                .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                                .text(
                                    pos2(gx, cell.center().y),
                                    Align2::LEFT_CENTER,
                                    suffix,
                                    TypeRole::Mono.font_id(),
                                    t.neutral_fg_secondary().gamma_multiply(0.55),
                                );
                        }
                    }
                }

                if *request_focus {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                    if ui.memory(|m| m.has_focus(edit_id)) {
                        *request_focus = false;
                    }
                }

                if words[idx] != before {
                    if looks_like_compact_key(&words[idx]) {
                        let compact: String = words[idx].split_whitespace().collect();
                        words[idx].clear();
                        out.compact_key_pasted = Some(compact);
                        out.changed = true;
                    } else if parse_pasted_words(&words[idx]).len() >= 2 {
                        let raw = std::mem::take(&mut words[idx]);
                        if let PasteOutcome::Words { start_slot, count, all_filled } =
                            apply_paste_to_words(words, idx, &raw)
                        {
                            let next = (start_slot + count).min(WORD_COUNT - 1);
                            next_focus = Some((next, words[next].chars().count()));
                            out.pasted_full_phrase = all_filled && all_words_valid(words);
                            out.changed = true;
                        }
                    } else if words[idx].contains([' ', ',', '\t', '\n']) {
                        // Space/comma confirms (accepting any unique prefix completion) and advances.
                        let clean: String = words[idx]
                            .chars()
                            .filter(|c| c.is_ascii_alphabetic())
                            .map(|c| c.to_ascii_lowercase())
                            .collect();
                        let finalized = match match_phrase_prefix(&clean) {
                            PhrasePrefixMatch::Unique(full) => full.to_owned(),
                            _ => clean,
                        };
                        words[idx] = finalized;
                        if !words[idx].is_empty() && idx + 1 < WORD_COUNT {
                            next_focus = Some((idx + 1, words[idx + 1].chars().count()));
                        }
                        out.changed = true;
                    } else {
                        // Keep only lowercase ASCII letters (BIP-39 alphabet).
                        let clean: String = words[idx]
                            .chars()
                            .filter(|c| c.is_ascii_alphabetic())
                            .map(|c| c.to_ascii_lowercase())
                            .collect();
                        let grew = clean.len() > before.len();
                        words[idx] = clean;
                        out.changed = true;

                        // Auto-advance only on insertion of a complete non-prefix BIP-39 word
                        // (e.g. "turkey" advances; "art" waits because "artist" also starts with "art").
                        if grew && idx + 1 < WORD_COUNT {
                            if let PhrasePrefixMatch::Unique(full) =
                                match_phrase_prefix(&words[idx])
                            {
                                if words[idx] == full {
                                    next_focus = Some((idx + 1, words[idx + 1].chars().count()));
                                }
                            }
                        }
                    }
                }

                // Clicking outside the text inner area but inside the cell still keeps focus.
                let cell_resp = ui.interact(cell, grid_id.with(("cell_hit", idx)), Sense::click());
                if cell_resp.clicked() && !resp.has_focus() {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                }
            } else {
                // Unfocused cell: mask valid words as •••••; keep invalid words readable in danger ink.
                if is_filled {
                    let (shown, color) = if is_invalid {
                        (trimmed, t.danger())
                    } else {
                        (MASK_DOTS, t.neutral_fg())
                    };
                    ui.painter()
                        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                        .text(
                            pos2(text_rect.left(), cell.center().y),
                            Align2::LEFT_CENTER,
                            shown,
                            TypeRole::Mono.font_id(),
                            color,
                        );
                }
                let cell_resp = ui.interact(cell, grid_id.with(("cell_hit", idx)), Sense::click());
                if cell_resp.clicked() {
                    next_focus = Some((idx, words[idx].chars().count()));
                }
            }
        }
    }

    if let Some((nf, caret_pos)) = next_focus {
        *focused = nf;
        set_edit_caret(ui.ctx(), slot_edit_id(grid_id, nf), caret_pos);
        *request_focus = true;
    }

    out
}

/// Read-only 3×8 phrase display grid (used in `Modal::BackupKey`).
/// When `revealed` is false, all 24 cells render `•••••`.
pub fn show_display_grid(ui: &mut Ui, t: &Theme, words: &[&str], revealed: bool) -> Rect {
    let total_w = ui_width(ui).max(1.0);
    let cell_w = ((total_w - CELL_GAP * (GRID_COLS as f32 - 1.0)) / GRID_COLS as f32).max(56.0);
    let total_h = GRID_ROWS as f32 * CELL_H + (GRID_ROWS as f32 - 1.0) * CELL_GAP;
    let (grid_rect, _) = ui.allocate_exact_size(vec2(total_w, total_h), Sense::hover());

    for row in 0..GRID_ROWS {
        for col in 0..GRID_COLS {
            let idx = row * GRID_COLS + col;
            let x = grid_rect.left() + col as f32 * (cell_w + CELL_GAP);
            let y = grid_rect.top() + row as f32 * (CELL_H + CELL_GAP);
            let cell = Rect::from_min_size(pos2(x, y), vec2(cell_w, CELL_H));

            ui.painter().rect(
                cell,
                Radius::Control.corner(),
                t.neutral_bg_secondary(),
                Stroke::new(STROKE_HAIRLINE, t.neutral()),
                StrokeKind::Inside,
            );

            let num_right = cell.left() + 26.0;
            ui.painter().text(
                pos2(num_right, cell.center().y),
                Align2::RIGHT_CENTER,
                format!("{}", idx + 1),
                TypeRole::Mono.font_id(),
                t.neutral_fg_secondary(),
            );

            let word = if revealed { words.get(idx).copied().unwrap_or("") } else { MASK_DOTS };
            let ink = if revealed { t.neutral_fg() } else { t.neutral_fg_secondary() };
            ui.painter().text(
                pos2(num_right + 8.0, cell.center().y),
                Align2::LEFT_CENTER,
                word,
                TypeRole::Mono.font_id(),
                ink,
            );
        }
    }

    grid_rect
}

/// Join the 24 phrase slots into a normalized lowercase space-separated string.
pub fn words_joined(words: &[String; WORD_COUNT]) -> String {
    words
        .iter()
        .map(|w| w.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn all_words_filled(words: &[String; WORD_COUNT]) -> bool {
    words.iter().all(|w| !w.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ThemeExt, install};
    use egui::{Pos2, RawInput};

    #[test]
    fn backspace_alt_and_cmd_backspace_in_empty_cell_jump_to_end_of_prior_slot() {
        for (case_idx, mods) in [Modifiers::NONE, Modifiers::ALT, Modifiers::COMMAND]
            .into_iter()
            .enumerate()
        {
            let ctx = Context::default();
            let grid_id = Id::unique(("test_bs_grid", case_idx));
            let mut words: [String; 24] = Default::default();
            words[0] = "turkey".into();
            words[1] = String::new();
            let mut focused = 1usize;
            let mut req_focus = true;

            let step = |events: Vec<Event>,
                        words: &mut [String; 24],
                        focused: &mut usize,
                        req_focus: &mut bool| {
                let input = RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 500.0))),
                    events,
                    ..Default::default()
                };
                ctx.run_pass(input, |ctx| {
                    install(ctx);
                    let t = ctx.get_lb_theme();
                    ctx.root_ui(|ui| {
                        show_import_grid(ui, &t, grid_id, words, focused, req_focus);
                    });
                })
                .textures_delta
                .clear();
            };

            // Establish focus on empty slot 1.
            step(vec![], &mut words, &mut focused, &mut req_focus);
            step(vec![], &mut words, &mut focused, &mut req_focus);
            assert_eq!(focused, 1);

            // Press Backspace / Alt+Backspace / Cmd+Backspace in empty slot 1.
            step(
                vec![Event::Key {
                    key: Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: mods,
                }],
                &mut words,
                &mut focused,
                &mut req_focus,
            );
            assert_eq!(focused, 0);
            // Caret is placed at the end of "turkey" (6 chars).
            assert_eq!(collapsed_caret_pos(&ctx, slot_edit_id(grid_id, 0)), Some("turkey".len()));
        }
    }

    #[test]
    fn import_grid_auto_advances_on_complete_unique_word_but_not_prefix_word() {
        let ctx = Context::default();
        let grid_id = Id::unique("test_auto_adv_grid");
        let mut words: [String; 24] = Default::default();
        words[0] = "turke".into();
        let mut focused = 0usize;
        let mut req_focus = true;

        let step = |events: Vec<Event>,
                    words: &mut [String; 24],
                    focused: &mut usize,
                    req_focus: &mut bool| {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 500.0))),
                events,
                ..Default::default()
            };
            ctx.run_pass(input, |ctx| {
                install(ctx);
                let t = ctx.get_lb_theme();
                ctx.root_ui(|ui| {
                    show_import_grid(ui, &t, grid_id, words, focused, req_focus);
                });
            })
            .textures_delta
            .clear();
        };

        // Focus slot 0 and put caret at end of "turke".
        set_edit_caret(&ctx, slot_edit_id(grid_id, 0), 5);
        step(vec![], &mut words, &mut focused, &mut req_focus);
        step(vec![], &mut words, &mut focused, &mut req_focus);

        // Typing 'y' finishes "turkey" (unique complete BIP-39 word) → auto-advances to slot 1.
        step(vec![Event::Text("y".into())], &mut words, &mut focused, &mut req_focus);
        assert_eq!(words[0], "turkey");
        assert_eq!(focused, 1);

        // In slot 1, typing "art" must NOT auto-advance because "artist" also starts with "art".
        step(vec![], &mut words, &mut focused, &mut req_focus);
        step(vec![Event::Text("art".into())], &mut words, &mut focused, &mut req_focus);
        assert_eq!(words[1], "art");
        assert_eq!(focused, 1);
    }
}
