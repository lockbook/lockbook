use std::sync::{Arc, RwLock};

use egui::{Id, Key, Pos2, Rect, Sense, Ui, Vec2};
use glyphon::{Buffer as GlyphonBuffer, Metrics, Shaping, Weight};
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::TextBufferArea;
use crate::file_cache::FileCache;
use crate::tab::core_get_relative_path;
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{BufferExt as _, Format};
use crate::tab::markdown_editor::widget::utils::{
    COMPLETION_FONT_SIZE, COMPLETION_LINE_HEIGHT, COMPLETION_MEASURE_WIDTH, COMPLETION_ROW_HEIGHT,
    base_attrs, draw_completion_popup, sans_fmt, to_glyphon,
};

const MAX_RESULTS: usize = 7;
const POPUP_CHROME: f32 = 60.0; // 8 left + name + 8 gap + path hint + 8 right
const TARGET_POPUP_WIDTH: f32 = 320.0; // soft target per row; popup grows to fit actual content
const MIN_HINT_WIDTH: f32 = 60.0; // always leave at least this much room for the hint

#[derive(Default, Clone, Copy, PartialEq)]
pub enum CompletionMode {
    /// `[[title]]` — resolved by note title at navigation time.
    #[default]
    WikiLink,
    /// `[display text](path)` — regular markdown link, shows all files.
    Link,
    /// `![alt text](path)` — image link, shows only image files.
    ImageLink,
}

#[derive(Default)]
pub struct LinkCompletions {
    /// Set before process_events so translate_egui_keyboard_event can swallow
    /// arrow/enter keys when the popup is open.
    pub active: bool,
    /// Keyboard-highlighted result index.
    pub selected: usize,
    /// Which kind of link syntax triggered the completion.
    pub mode: CompletionMode,
    /// Suppressed query string — cleared automatically when the query changes.
    suppressed: Option<String>,
}

impl LinkCompletions {
    pub fn update_active_state(&mut self, buffer: &Buffer) {
        self.active = false;

        let Some((range, mode)) = detect_any(buffer) else { return };
        let query = query_from_range(buffer, range, mode);
        if self.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }
        // A complete token means the cursor navigated into existing syntax — don't activate.
        let raw = &buffer[range];
        let complete = match mode {
            CompletionMode::WikiLink => raw.ends_with("]]"),
            CompletionMode::Link | CompletionMode::ImageLink => raw.ends_with(')'),
        };
        if complete {
            return;
        }
        self.mode = mode;
        self.active = true;
    }
}

/// Tries all detection strategies, returning the first match with its mode.
/// WikiLink (`[[`) takes priority over plain Link (`[`).
fn detect_any(buffer: &Buffer) -> Option<((DocCharOffset, DocCharOffset), CompletionMode)> {
    if let Some(range) = detect_wikilink(buffer) {
        return Some((range, CompletionMode::WikiLink));
    }
    if let Some((range, is_image)) = detect_link(buffer) {
        let mode = if is_image { CompletionMode::ImageLink } else { CompletionMode::Link };
        return Some((range, mode));
    }
    None
}

/// Returns the range of a `[[...]]` wikilink token under the cursor.
fn detect_wikilink(buffer: &Buffer) -> Option<(DocCharOffset, DocCharOffset)> {
    let selection = buffer.current.selection;
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let text = buffer.current.text.to_string();
    let chars: Vec<char> = text.chars().collect();

    let mut i = cursor_idx;
    let bracket_start;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let c = chars[i];
        if c == '\n' || c == ']' {
            return None;
        }
        if c == '[' {
            if i > 0 && chars[i - 1] == '[' {
                bracket_start = i - 1;
                break;
            }
            return None; // single '[' — not a wikilink
        }
        if cursor_idx - i > 200 {
            return None;
        }
    }

    let mut j = cursor_idx;
    while j < chars.len() {
        if chars[j] == '\n' {
            break;
        }
        if chars[j] == ']' && j + 1 < chars.len() && chars[j + 1] == ']' {
            j += 2;
            break;
        }
        if j - cursor_idx > 200 {
            break;
        }
        j += 1;
    }

    Some((DocCharOffset(bracket_start), DocCharOffset(j)))
}

/// Returns the range of a `[text](path)` or `![text](path)` link under the cursor,
/// plus whether it's an image link. The cursor must be in the display-text field
/// (between `[` and `]`); if `](...)` already exists it's included in the range
/// so the whole link is replaced when a result is picked.
fn detect_link(buffer: &Buffer) -> Option<((DocCharOffset, DocCharOffset), bool)> {
    let selection = buffer.current.selection;
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let text = buffer.current.text.to_string();
    let chars: Vec<char> = text.chars().collect();

    // Scan backward for a single '[' that is NOT preceded by '[' (wikilink).
    // Stop at newlines, existing ']', '(' or ')' — we're outside the text field.
    let mut i = cursor_idx;
    let open_bracket;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let c = chars[i];
        if c == '\n' || c == ']' || c == '(' || c == ')' {
            return None;
        }
        if c == '[' {
            if i > 0 && chars[i - 1] == '[' {
                return None; // wikilink — handled separately
            }
            open_bracket = i;
            break;
        }
        if cursor_idx - i > 200 {
            return None;
        }
    }

    let is_image = open_bracket > 0 && chars[open_bracket - 1] == '!';
    let start = if is_image { open_bracket - 1 } else { open_bracket };

    // Scan forward from cursor. If `](...)` follows, include it so the whole
    // link is replaced when the user picks a result.
    let mut j = cursor_idx;
    while j < chars.len() && chars[j] != '\n' {
        if chars[j] == ']' {
            j += 1;
            if j < chars.len() && chars[j] == '(' {
                j += 1;
                while j < chars.len() && chars[j] != ')' && chars[j] != '\n' {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ')' {
                    j += 1;
                }
            }
            break;
        }
        j += 1;
    }

    Some(((DocCharOffset(start), DocCharOffset(j)), is_image))
}

/// Extracts the search query from a detected token range, stripping link syntax.
fn query_from_range(
    buffer: &Buffer, range: (DocCharOffset, DocCharOffset), mode: CompletionMode,
) -> String {
    let raw = &buffer[range];
    match mode {
        CompletionMode::WikiLink => raw
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string(),
        CompletionMode::Link | CompletionMode::ImageLink => {
            // Strip leading `![` or `[`, then take only text before the first `]`.
            let s = raw.trim_start_matches('!').trim_start_matches('[');
            s.split(']').next().unwrap_or("").to_string()
        }
    }
}

struct FileResult {
    /// Display name without .md extension.
    name: String,
    /// Full relative path from current file (with .md), used as hint and for disambiguation.
    rel_path: String,
    /// What to insert: bare title if unique, minimal partial path if conflicting.
    insert: String,
}

fn search(
    core: &lb_rs::blocking::Lb, cache: &Option<FileCache>, file_id: Uuid, query: &str,
    mode: CompletionMode,
) -> Vec<FileResult> {
    let Some(cache) = cache else { return Vec::new() };
    let files = &cache.files;
    // Paths in markdown are relative to the parent folder of the current file,
    // matching how the image cache and existing link insertion resolve them.
    let from_id = core
        .get_file_by_id(file_id)
        .map(|f| f.parent)
        .unwrap_or(file_id);
    let lq = query.trim_end_matches(".md").to_lowercase();

    // For image links, match against the full filename (including extension).
    // For wikilinks and regular links, match against the title without .md.
    let file_matches = |name: &str| -> bool {
        let search_name =
            if mode == CompletionMode::ImageLink { name } else { name.trim_end_matches(".md") };
        let ls = search_name.to_lowercase();
        if lq.is_empty() {
            return true;
        }
        ls == lq || ls.starts_with(&lq) || is_subsequence(&lq, &ls)
    };

    let file_allowed = |name: &str| -> bool {
        match mode {
            CompletionMode::ImageLink => name
                .rsplit('.')
                .next()
                .map(is_supported_image_fmt)
                .unwrap_or(false),
            CompletionMode::WikiLink | CompletionMode::Link => true,
        }
    };

    if query.is_empty() {
        if mode == CompletionMode::ImageLink {
            // Images are rarely in the suggested list (they aren't opened like notes),
            // so for image links show all image files sorted by last_modified desc.
            let mut image_files: Vec<_> = files
                .iter()
                .filter(|f| f.is_document() && f.id != file_id && file_allowed(&f.name))
                .collect();
            image_files.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
            let mut results: Vec<FileResult> = image_files
                .into_iter()
                .take(MAX_RESULTS)
                .map(|f| {
                    let rel_path = core_get_relative_path(core, file_id, f.id);
                    let rel_path = rel_path.strip_prefix("./").unwrap_or(&rel_path).to_string();
                    FileResult { name: f.name.clone(), rel_path, insert: String::new() }
                })
                .collect();
            populate_insert(files, &mut results, mode);
            return results;
        }

        // For WikiLink/Link: return suggested docs in activity-weighted order.
        let mut results = Vec::new();
        for &id in &cache.suggested {
            if id == file_id {
                continue;
            }
            let Some(f) = files.iter().find(|f| f.id == id && f.is_document()) else {
                continue;
            };
            let display_name = f.name.trim_end_matches(".md").to_string();
            let rel_path = core_get_relative_path(core, from_id, f.id);
            let rel_path = rel_path.strip_prefix("./").unwrap_or(&rel_path).to_string();
            results.push(FileResult { name: display_name, rel_path, insert: String::new() });
            if results.len() == MAX_RESULTS {
                break;
            }
        }
        populate_insert(files, &mut results, mode);
        return results;
    }

    let mut scored: Vec<(FileResult, u8, usize)> = files
        .iter()
        .filter(|f| f.is_document() && f.id != file_id && file_allowed(&f.name))
        .filter_map(|f| {
            if !file_matches(&f.name) {
                return None;
            }
            let display_name = if mode == CompletionMode::ImageLink {
                f.name.clone()
            } else {
                f.name.trim_end_matches(".md").to_string()
            };
            let search_name = display_name.to_lowercase();
            let tier = if search_name == lq {
                0u8
            } else if search_name.starts_with(&lq) {
                1
            } else {
                2
            };
            let rel_path = core_get_relative_path(core, from_id, f.id);
            let rel_path = rel_path.strip_prefix("./").unwrap_or(&rel_path).to_string();
            Some((
                FileResult { name: display_name.clone(), rel_path, insert: String::new() },
                tier,
                display_name.len(),
            ))
        })
        .collect();

    // Sort: by tier asc, then by title length asc within tier.
    scored.sort_by_key(|(_, tier, len)| (*tier, *len));

    let mut results: Vec<FileResult> = scored
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(r, _, _)| r)
        .collect();

    populate_insert(files, &mut results, mode);
    results
}

/// Fills `result.insert` for each entry.
/// - WikiLink: bare title when unique, minimal partial path when ambiguous.
/// - Link/ImageLink: always the full relative path (no ambiguity since path is explicit).
fn populate_insert(
    all_files: &[lb_rs::model::file::File], results: &mut Vec<FileResult>, mode: CompletionMode,
) {
    if mode != CompletionMode::WikiLink {
        // For regular and image links, insert = rel_path directly (path is the canonical ref).
        for result in results.iter_mut() {
            result.insert = result.rel_path.clone();
        }
        return;
    }
    let all_titles: Vec<&str> = all_files
        .iter()
        .filter(|f| f.is_document())
        .map(|f| f.name.trim_end_matches(".md"))
        .collect();

    for result in results {
        let count = all_titles
            .iter()
            .filter(|t| t.eq_ignore_ascii_case(&result.name))
            .count();
        result.insert = if count <= 1 {
            result.name.clone()
        } else {
            let parts: Vec<&str> = result
                .rel_path
                .trim_end_matches(".md")
                .rsplitn(2, '/')
                .collect();
            if parts.len() == 2 {
                format!("{}/{}", parts[1], parts[0])
            } else {
                result.rel_path.trim_end_matches(".md").to_string()
            }
        };
    }
}

/// Returns a bool per filename character: true if consumed by the subsequence match.
fn match_positions(query: &str, name: &str) -> Vec<bool> {
    let mut result = vec![false; name.chars().count()];
    let mut qi = query.chars().peekable();
    for (i, nc) in name.chars().enumerate() {
        if qi.peek() == Some(&nc) {
            result[i] = true;
            qi.next();
        }
    }
    result
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut hc = haystack.chars();
    'outer: for nc in needle.chars() {
        for h in &mut hc {
            if h == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

/// Abbreviates folder components in a path from outermost to innermost until
/// the path fits within `max_width`. The filename (last component) is never
/// shortened. `..` segments are left as-is since they're already minimal.
///
/// Example: `projects/work/clients/note` → `p/work/clients/note` → `p/w/clients/note` → …
fn abbreviate_path(path: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(path) <= max_width {
        return path.to_string();
    }
    let mut parts: Vec<String> = path.split('/').map(|s| s.to_string()).collect();
    let n = parts.len();
    // Iterate over all components except the last (filename).
    for i in 0..n.saturating_sub(1) {
        if parts[i] == ".." || parts[i] == "." || parts[i].chars().count() <= 1 {
            continue;
        }
        parts[i] = parts[i].chars().next().unwrap().to_string();
        if measure(&parts.join("/")) <= max_width {
            break;
        }
    }
    parts.join("/")
}

impl Editor {
    pub fn show_link_completions(&mut self, ui: &mut Ui) {
        if self.readonly || !self.link_completions.active {
            return;
        }

        let Some(((bracket_start, replace_end), mode)) = detect_any(&self.buffer) else {
            return;
        };
        let query = query_from_range(&self.buffer, (bracket_start, replace_end), mode);

        if self.link_completions.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }

        let cache = self.files.read().unwrap();
        let results = search(&self.core, &cache, self.file_id, &query, mode);
        drop(cache);
        if results.is_empty() {
            return;
        }

        self.link_completions.selected = self.link_completions.selected.min(results.len() - 1);

        let Some([cursor_top, cursor_bot]) = self.cursor_line(bracket_start) else {
            return;
        };

        // Escape is checked outside the focused guard so it always fires.
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.link_completions.suppressed = Some(query.clone());
            return;
        }

        if self.focused(ui.ctx()) {
            ui.input(|i| {
                if i.key_pressed(Key::ArrowUp) && self.link_completions.selected > 0 {
                    self.link_completions.selected -= 1;
                }
                if i.key_pressed(Key::ArrowDown)
                    && self.link_completions.selected + 1 < results.len()
                {
                    self.link_completions.selected += 1;
                }
            });

            if ui.input(|i| i.key_pressed(Key::Enter)) {
                let idx = self.link_completions.selected;
                self.apply_link_completion(
                    bracket_start,
                    replace_end,
                    &results[idx].name,
                    &results[idx].insert,
                    mode,
                );
                return;
            }

            let mut chosen = None;
            ui.input(|i| {
                for (idx, key) in
                    [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7]
                        .iter()
                        .enumerate()
                        .take(results.len())
                {
                    if i.key_pressed(*key) && i.modifiers.ctrl {
                        chosen = Some(idx);
                    }
                }
            });
            if let Some(idx) = chosen {
                self.apply_link_completion(
                    bracket_start,
                    replace_end,
                    &results[idx].name,
                    &results[idx].insert,
                    mode,
                );
                return;
            }
        }

        let ppi = ui.ctx().pixels_per_point();
        let hint_fmt = Format { color: ui.visuals().weak_text_color(), ..sans_fmt() };

        let lq = query.trim_end_matches(".md").to_lowercase();

        // Measure each row's name width individually so hint budgets are per-row.
        let name_widths: Vec<f32> = results
            .iter()
            .map(|r| {
                let buf = self.upsert_glyphon_buffer(
                    &r.name,
                    COMPLETION_FONT_SIZE,
                    COMPLETION_LINE_HEIGHT,
                    COMPLETION_MEASURE_WIDTH,
                    &sans_fmt(),
                );
                let w = buf.read().unwrap().shaped_size(ppi).x;
                w
            })
            .collect();

        // Measure ctrl hint width once — all "^N" strings are the same width.
        let ctrl_hint_width = {
            let sample = format!("^{}", results.len());
            let buf = self.upsert_glyphon_buffer(
                &sample,
                COMPLETION_FONT_SIZE - 2.0,
                COMPLETION_LINE_HEIGHT,
                COMPLETION_MEASURE_WIDTH,
                &hint_fmt,
            );
            let w = buf.read().unwrap().shaped_size(ppi).x;
            w
        };
        let ctrl_hint_gap = ctrl_hint_width + 8.0; // space reserved on the right for ^N

        // Abbreviate each hint to fit within TARGET_POPUP_WIDTH minus that row's name width.
        // This ensures name and hint never overlap regardless of which row is widest.
        let hints: Vec<String> = results
            .iter()
            .zip(name_widths.iter())
            .map(|(r, &nw)| {
                let budget =
                    (TARGET_POPUP_WIDTH - nw - POPUP_CHROME - ctrl_hint_gap).max(MIN_HINT_WIDTH);
                abbreviate_path(&r.rel_path, budget, |text| {
                    let buf = self.upsert_glyphon_buffer(
                        text,
                        COMPLETION_FONT_SIZE - 2.0,
                        COMPLETION_LINE_HEIGHT,
                        COMPLETION_MEASURE_WIDTH,
                        &hint_fmt,
                    );
                    let w = buf.read().unwrap().shaped_size(ppi).x;
                    w
                })
            })
            .collect();

        // Popup width = max per-row total; each row fits exactly, no overlap, no hard cap.
        let popup_width = name_widths
            .iter()
            .zip(hints.iter())
            .map(|(&nw, h)| {
                let buf = self.upsert_glyphon_buffer(
                    h,
                    COMPLETION_FONT_SIZE - 2.0,
                    COMPLETION_LINE_HEIGHT,
                    COMPLETION_MEASURE_WIDTH,
                    &hint_fmt,
                );
                let hw = buf.read().unwrap().shaped_size(ppi).x;
                nw + hw + ctrl_hint_gap + POPUP_CHROME
            })
            .fold(0.0_f32, f32::max);
        let popup_height = results.len() as f32 * COMPLETION_ROW_HEIGHT;
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

        // Interaction.
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked = None;
        for (idx, _) in results.iter().enumerate() {
            let resp = ui.interact(row_rects[idx], Id::new("link_item").with(idx), Sense::click());
            if resp.clicked() {
                clicked = Some(idx);
            }
        }

        // Draw frame and row backgrounds.
        draw_completion_popup(
            ui.painter(),
            popup_rect,
            &row_rects,
            self.link_completions.selected,
            hover_pos,
            ui.visuals().extreme_bg_color,
            ui.visuals().widgets.hovered.bg_fill,
            ui.visuals().selection.bg_fill.gamma_multiply(0.3),
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        );

        let text_color = ui.visuals().text_color();
        let hint_color = ui.visuals().weak_text_color();
        let clip_rect = ui.clip_rect();
        let normal_color = to_glyphon(text_color);

        let mut text_areas: Vec<TextBufferArea> = Vec::new();

        for (idx, result) in results.iter().enumerate() {
            let rect = row_rects[idx];
            let text_top = rect.min.y + 4.0;

            // Name with bold matched characters. Match against title (no .md).
            let flags = match_positions(&lq, &result.name.to_lowercase());
            let mut spans: Vec<(String, bool)> = Vec::new();
            let mut cur = String::new();
            let mut cur_bold = false;
            for (ch, &matched) in result.name.chars().zip(flags.iter()) {
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
            let label_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, text_top),
                Vec2::new(name_widths[idx], COMPLETION_LINE_HEIGHT),
            );
            text_areas.push(TextBufferArea::new(
                Arc::new(RwLock::new(b)),
                label_rect,
                normal_color,
                ui.ctx(),
                clip_rect,
            ));

            // Ctrl+N shortcut hint at the far right.
            let ctrl_hint = format!("^{}", idx + 1);
            let ctrl_buf = self.upsert_glyphon_buffer(
                &ctrl_hint,
                COMPLETION_FONT_SIZE - 2.0,
                COMPLETION_LINE_HEIGHT,
                COMPLETION_MEASURE_WIDTH,
                &hint_fmt,
            );
            let ctrl_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - ctrl_hint_width - 8.0, text_top),
                Vec2::new(ctrl_hint_width, COMPLETION_LINE_HEIGHT),
            );
            text_areas.push(TextBufferArea::new(
                ctrl_buf,
                ctrl_rect,
                to_glyphon(hint_color),
                ui.ctx(),
                clip_rect,
            ));

            // Relative path hint, right-aligned (already abbreviated to fit), left of ^N.
            let hint_buf = self.upsert_glyphon_buffer(
                &hints[idx],
                COMPLETION_FONT_SIZE - 2.0,
                COMPLETION_LINE_HEIGHT,
                COMPLETION_MEASURE_WIDTH,
                &hint_fmt,
            );
            let hint_width = hint_buf.read().unwrap().shaped_size(ppi).x;
            let hint_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - hint_width - ctrl_hint_gap - 8.0, text_top),
                Vec2::new(hint_width, COMPLETION_LINE_HEIGHT),
            );
            text_areas.push(TextBufferArea::new(
                hint_buf,
                hint_rect,
                to_glyphon(hint_color),
                ui.ctx(),
                clip_rect,
            ));
        }

        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.max_rect(),
                crate::GlyphonRendererCallback::new(text_areas),
            ));

        if let Some(idx) = clicked {
            self.apply_link_completion(
                bracket_start,
                replace_end,
                &results[idx].name,
                &results[idx].insert,
                mode,
            );
        }
    }

    fn apply_link_completion(
        &mut self, bracket_start: DocCharOffset, replace_end: DocCharOffset, display: &str,
        path: &str, mode: CompletionMode,
    ) {
        let text = match mode {
            CompletionMode::WikiLink => format!("[[{}]]", path),
            CompletionMode::Link => format!("[{}]({})", display, path),
            CompletionMode::ImageLink => format!("![{}]({})", display, path),
        };
        self.event.internal_events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::DocCharOffset(bracket_start),
                end: Location::DocCharOffset(replace_end),
            },
            text,
            advance_cursor: true,
        });
        self.link_completions.selected = 0;
        self.link_completions.suppressed = None;
    }
}
