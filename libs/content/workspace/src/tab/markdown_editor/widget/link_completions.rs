use egui::{Context, Id, Key, Modifiers, Pos2, Rect, Sense, Ui, Vec2};
use lb_rs::Uuid;
use lb_rs::model::file::File;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use std::sync::{Arc, RwLock};

use crate::TextBufferArea;
use crate::file_cache::{FileCache, FilesExt as _, relative_path};
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::bounds::{Paragraphs, RangesExt as _};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::GlyphonLabel;

const MAX_RESULTS: usize = 7;
const MIN_QUERY_LEN: usize = 2;
const POPUP_PADDING: f32 = 24.0; // 8 left + 8 gap + 8 right
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
    /// True when a valid link query is being typed and has results.
    /// Read by the editor to gate rendering; also gates `handle_input`.
    pub active: bool,
    /// Keyboard-highlighted result index.
    pub selected: usize,
    /// Which kind of link syntax triggered the completion.
    pub mode: CompletionMode,
    /// The search term range in the document (the query text only, excluding
    /// brackets/syntax). Set when active so show_text() can highlight it.
    pub search_term_range: Option<(Grapheme, Grapheme)>,
    /// Suppressed query string — cleared automatically when the query changes.
    suppressed: Option<String>,
}

impl LinkCompletions {
    pub fn update_active_state(
        &mut self, buffer: &Buffer, inline_paragraphs: &Paragraphs, files: &Arc<RwLock<FileCache>>,
        file_id: Uuid,
    ) {
        self.active = false;
        self.search_term_range = None;

        if inline_paragraphs
            .find_containing(buffer.current.selection.1, true, true)
            .is_empty()
        {
            // not in an inline paragraph; wherever the cursor is rn, inlines do not apply
            return;
        }

        let Some((range, mode)) = detect_any(buffer) else { return };
        let qr = query_range(buffer, range, mode);
        let query = &buffer[qr];
        if query.len() < MIN_QUERY_LEN {
            return;
        }
        if self.suppressed.as_deref() == Some(query) {
            return;
        }

        let raw = &buffer[range];
        let complete = match mode {
            CompletionMode::WikiLink => raw.ends_with("]]"),
            CompletionMode::Link | CompletionMode::ImageLink => raw.ends_with(')'),
        };
        if complete {
            // cursor navigated into existing syntax
            return;
        }

        // Only activate if there are actual results to show.
        let cache = files.read().unwrap();
        let has_results = !search(&cache, file_id, query, mode).is_empty();
        if !has_results {
            return;
        }

        self.mode = mode;
        self.active = true;
        self.search_term_range = Some(qr);
    }

    /// Consume (or observe) keyboard events targeting the popup and update
    /// state accordingly. Emitted replacements are pushed onto `events`.
    ///
    /// Must run before the editor's `process_events`. Escape is observed
    /// (not consumed) and fires regardless of editor focus; nav keys are
    /// consumed and focus-gated. See `EmojiCompletions::handle_input` for the
    /// rationale.
    pub fn handle_input(
        &mut self, ctx: &Context, buffer: &Buffer, files: &Arc<RwLock<FileCache>>, file_id: Uuid,
        editor_focused: bool, events: &mut Vec<Event>,
    ) {
        if !self.active {
            return;
        }
        let Some(((bracket_start, replace_end), mode)) = detect_any(buffer) else { return };
        let qr = query_range(buffer, (bracket_start, replace_end), mode);
        let query = buffer[qr].to_string();
        if self.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }

        let cache = files.read().unwrap();
        let results = search(&cache, file_id, &query, mode);
        drop(cache);
        if results.is_empty() {
            return;
        }
        self.selected = self.selected.min(results.len() - 1);

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.suppressed = Some(query);
            return;
        }

        if !editor_focused {
            return;
        }

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) && self.selected > 0 {
            self.selected -= 1;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
            && self.selected + 1 < results.len()
        {
            self.selected += 1;
        }

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            let idx = self.selected;
            let r = &results[idx];
            self.apply_completion(events, bracket_start, replace_end, &r.name, &r.insert, mode);
            return;
        }

        let num_modifier = if cfg!(any(target_os = "macos", target_os = "ios")) {
            Modifiers::COMMAND
        } else {
            Modifiers::CTRL
        };
        for (idx, key) in
            [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7]
                .iter()
                .enumerate()
                .take(results.len())
        {
            if ctx.input_mut(|i| i.consume_key(num_modifier, *key)) {
                let r = &results[idx];
                self.apply_completion(events, bracket_start, replace_end, &r.name, &r.insert, mode);
                return;
            }
        }
    }

    /// Push the replacement event for the current query and reset popup state.
    /// Shared between `handle_input` and the click path in `show_link_completions`.
    pub fn apply_completion(
        &mut self, events: &mut Vec<Event>, bracket_start: Grapheme, replace_end: Grapheme,
        display: &str, path: &str, mode: CompletionMode,
    ) {
        let text = match mode {
            CompletionMode::WikiLink => format!("[[{}]]", path),
            CompletionMode::Link => format!("[{}]({})", display, path),
            CompletionMode::ImageLink => format!("![{}]({})", display, path),
        };
        events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(bracket_start),
                end: Location::Grapheme(replace_end),
            },
            text,
            advance_cursor: true,
        });
        self.selected = 0;
        self.suppressed = None;
    }
}

/// Tries all detection strategies, returning the first match with its mode.
/// WikiLink (`[[`) takes priority over plain Link (`[`).
fn detect_any(buffer: &Buffer) -> Option<((Grapheme, Grapheme), CompletionMode)> {
    if let Some(range) = detect_wikilink(buffer) {
        return Some((range, CompletionMode::WikiLink));
    }
    if let Some((range, is_image)) = detect_link(buffer) {
        let mode = if is_image { CompletionMode::ImageLink } else { CompletionMode::Link };
        return Some((range, mode));
    }
    None
}

/// Returns the grapheme `&str` at the given char offset.
fn grapheme_at(buffer: &Buffer, i: usize) -> &str {
    &buffer[(Grapheme(i), Grapheme(i + 1))]
}

/// Returns the range of a `[[...]]` wikilink token under the cursor.
fn detect_wikilink(buffer: &Buffer) -> Option<(Grapheme, Grapheme)> {
    let selection = buffer.current.selection;
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let len = buffer.current.segs.last_cursor_position().0;

    let mut i = cursor_idx;
    let bracket_start;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let g = grapheme_at(buffer, i);
        if g == "\n" || g == "]" {
            return None;
        }
        if g == "[" {
            if i > 0 && grapheme_at(buffer, i - 1) == "[" {
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
    while j < len {
        if grapheme_at(buffer, j) == "\n" {
            break;
        }
        if grapheme_at(buffer, j) == "]" && j + 1 < len && grapheme_at(buffer, j + 1) == "]" {
            j += 2;
            break;
        }
        if j - cursor_idx > 200 {
            break;
        }
        j += 1;
    }

    Some((Grapheme(bracket_start), Grapheme(j)))
}

/// Returns the range of a `[text](path)` or `![text](path)` link under the cursor,
/// plus whether it's an image link. The cursor must be in the display-text field
/// (between `[` and `]`); if `](...)` already exists it's included in the range
/// so the whole link is replaced when a result is picked.
fn detect_link(buffer: &Buffer) -> Option<((Grapheme, Grapheme), bool)> {
    let selection = buffer.current.selection;
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let len = buffer.current.segs.last_cursor_position().0;

    // Scan backward for a single '[' that is NOT preceded by '[' (wikilink).
    // Stop at newlines, existing ']', '(' or ')' — we're outside the text field.
    let mut i = cursor_idx;
    let open_bracket;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let g = grapheme_at(buffer, i);
        if g == "\n" || g == "]" || g == "(" || g == ")" {
            return None;
        }
        if g == "[" {
            if i > 0 && grapheme_at(buffer, i - 1) == "[" {
                return None; // wikilink — handled separately
            }
            open_bracket = i;
            break;
        }
        if cursor_idx - i > 200 {
            return None;
        }
    }

    let is_image = open_bracket > 0 && grapheme_at(buffer, open_bracket - 1) == "!";
    let start = if is_image { open_bracket - 1 } else { open_bracket };

    // Scan forward from cursor. If `](...)` follows, include it so the whole
    // link is replaced when the user picks a result.
    let mut j = cursor_idx;
    while j < len && grapheme_at(buffer, j) != "\n" {
        if grapheme_at(buffer, j) == "]" {
            j += 1;
            if j < len && grapheme_at(buffer, j) == "(" {
                j += 1;
                while j < len && grapheme_at(buffer, j) != ")" && grapheme_at(buffer, j) != "\n" {
                    j += 1;
                }
                if j < len && grapheme_at(buffer, j) == ")" {
                    j += 1;
                }
            }
            break;
        }
        j += 1;
    }

    Some(((Grapheme(start), Grapheme(j)), is_image))
}

/// Returns the sub-range of `range` covering just the query text, with syntax stripped.
fn query_range(
    buffer: &Buffer, range: (Grapheme, Grapheme), mode: CompletionMode,
) -> (Grapheme, Grapheme) {
    let prefix_len = match mode {
        CompletionMode::WikiLink => 2,  // [[
        CompletionMode::Link => 1,      // [
        CompletionMode::ImageLink => 2, // ![
    };
    let start = Grapheme(range.0.0 + prefix_len);

    // Convert byte lengths to grapheme counts before adding to a `Grapheme`;
    // multi-byte clusters (Devanagari, emoji) overshoot otherwise.
    let raw = &buffer[range];
    let end = match mode {
        CompletionMode::WikiLink => {
            let trimmed = raw.trim_end_matches(']');
            Grapheme(range.0.0 + trimmed.graphemes(true).count())
        }
        CompletionMode::Link | CompletionMode::ImageLink => {
            let after_prefix = &raw[prefix_len..];
            let text_byte_len = after_prefix.find(']').unwrap_or(after_prefix.len());
            let text_grapheme_count = after_prefix[..text_byte_len].graphemes(true).count();
            Grapheme(start.0 + text_grapheme_count)
        }
    };

    (start, end)
}

struct FileResult {
    /// The file's UUID.
    id: Uuid,
    /// Display name without .md extension.
    name: String,
    /// Full relative path from current file (with .md), used as hint and for disambiguation.
    rel_path: String,
    /// What to insert: bare title if unique, minimal partial path if conflicting.
    insert: String,
    /// True if this file is in a different tree than the current file.
    cross_tree: bool,
    /// Path segments with per-segment share-recipient flag, for colored rendering.
    path_segments: Vec<(String, bool)>,
}

fn search(cache: &FileCache, file_id: Uuid, query: &str, mode: CompletionMode) -> Vec<FileResult> {
    // Paths in markdown are relative to the parent folder of the current file,
    // matching how the image cache and existing link insertion resolve them.
    let from_id = cache
        .get_by_id(file_id)
        .map(|f| f.parent)
        .unwrap_or(file_id);
    let from_path = cache.path(from_id);
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

    // Path distance: number of `../` segments in the relative path, or u16::MAX for cross-tree.
    let path_distance = |f_id: Uuid, cross_tree: bool| -> u16 {
        if cross_tree {
            return u16::MAX;
        }
        let rel = relative_path(&from_path, &cache.path(f_id));
        rel.matches("../").count() as u16
    };

    // Build a FileResult for a file, computing cross-tree status and rel_path.
    let make_result = |f: &File, display_name: String| -> FileResult {
        let cross_tree = !cache.same_tree(file_id, f.id);
        let rel_path = if cross_tree {
            // Show path within the shared tree as hint
            cache.path(f.id)
        } else {
            let rp = relative_path(&from_path, &cache.path(f.id));
            rp.strip_prefix("./").unwrap_or(&rp).to_string()
        };
        let path_segments = cache.path_segments(f.id);
        FileResult {
            id: f.id,
            name: display_name,
            rel_path,
            insert: String::new(),
            cross_tree,
            path_segments,
        }
    };

    if query.is_empty() {
        if mode == CompletionMode::ImageLink {
            // Images are rarely in the suggested list (they aren't opened like notes),
            // so for image links show all image files sorted by last_modified desc.
            let mut image_files: Vec<_> = cache
                .all_files()
                .filter(|f| f.is_document() && f.id != file_id && file_allowed(&f.name))
                .collect();
            image_files.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
            let mut results: Vec<FileResult> = image_files
                .into_iter()
                .take(MAX_RESULTS)
                .map(|f| make_result(f, f.name.clone()))
                .collect();
            populate_insert(cache, &mut results, mode);
            return results;
        }

        // For WikiLink/Link: return suggested docs in activity-weighted order.
        let mut results = Vec::new();
        for &id in &cache.suggested {
            if id == file_id {
                continue;
            }
            let Some(f) = cache.get_by_id(id).filter(|f| f.is_document()) else {
                continue;
            };
            let display_name = f.name.trim_end_matches(".md").to_string();
            results.push(make_result(f, display_name));
            if results.len() == MAX_RESULTS {
                break;
            }
        }
        populate_insert(cache, &mut results, mode);
        return results;
    }

    // scored: (result, tier, cross_tree, path_distance, name_len)
    let mut scored: Vec<(FileResult, u8, bool, u16, usize)> = cache
        .all_files()
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
            let result = make_result(f, display_name.clone());
            let cross = result.cross_tree;
            let dist = path_distance(f.id, cross);
            Some((result, tier, cross, dist, display_name.len()))
        })
        .collect();

    // Sort: tier, then same-tree before cross-tree, then proximity, then name length.
    scored.sort_by_key(|(_, tier, cross, dist, len)| (*tier, *cross, *dist, *len));

    let mut results: Vec<FileResult> = scored
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(r, _, _, _, _)| r)
        .collect();

    populate_insert(cache, &mut results, mode);
    results
}

/// Fills `result.insert` for each entry.
/// - Cross-tree files always use `lb://uuid` (relative paths don't work across trees).
/// - WikiLink: bare title when unique, minimal partial path when ambiguous.
/// - Link/ImageLink: always the full relative path (no ambiguity since path is explicit).
fn populate_insert(cache: &FileCache, results: &mut [FileResult], mode: CompletionMode) {
    if mode != CompletionMode::WikiLink {
        for result in results.iter_mut() {
            if result.cross_tree {
                result.insert = format!("lb://{}", result.id);
            } else {
                result.insert = encode_link_path(&result.rel_path);
            }
        }
        return;
    }

    // For wikilinks: cross-tree files use lb://uuid; same-tree files use title or partial path.
    let all_titles: Vec<&str> = cache
        .all_files()
        .filter(|f| f.is_document())
        .map(|f| f.name.trim_end_matches(".md"))
        .collect();

    for result in results.iter_mut() {
        if result.cross_tree {
            result.insert = format!("lb://{}", result.id);
            continue;
        }
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

/// Percent-encodes characters that are invalid in CommonMark bare link destinations.
fn encode_link_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            _ => out.push(c),
        }
    }
    out
}

/// Groups characters into `(text, bold)` spans based on match flags.
fn build_bold_spans(text: &str, flags: &[bool]) -> Vec<(String, bool)> {
    let mut spans = Vec::new();
    let mut cur = String::new();
    let mut cur_bold = false;
    for (ch, &matched) in text.chars().zip(flags.iter()) {
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
    spans
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
/// Abbreviates path segments in-place: shortens intermediate directory names to
/// their first character until the full path fits within `max_width`. Separator
/// segments (`/`) and the final name segment (filename) are never shortened.
fn abbreviate_segments(
    segments: &mut [(String, bool)], max_width: f32, measure: &impl Fn(&str) -> f32,
) {
    let full: String = segments.iter().map(|(t, _)| t.as_str()).collect();
    if measure(&full) <= max_width {
        return;
    }
    // Find indices of name segments (non-"/" entries), excluding the last (filename).
    let name_indices: Vec<usize> = segments
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| t != "/")
        .map(|(i, _)| i)
        .collect();
    // Shorten all except the last name segment (the filename).
    let dir_indices: Vec<usize> = name_indices.iter().rev().skip(1).copied().rev().collect();
    for i in dir_indices {
        let text = &segments[i].0;
        if text == ".." || text == "." || text.chars().count() <= 1 {
            continue;
        }
        segments[i].0 = text.chars().next().unwrap().to_string();
        let full: String = segments.iter().map(|(t, _)| t.as_str()).collect();
        if measure(&full) <= max_width {
            break;
        }
    }
}

impl MdEdit {
    pub fn show_link_completions(&mut self, ui: &mut Ui) {
        if self.renderer.readonly || !self.link_completions.active {
            return;
        }

        let Some(((bracket_start, replace_end), mode)) = detect_any(&self.renderer.buffer) else {
            return;
        };
        let qr = query_range(&self.renderer.buffer, (bracket_start, replace_end), mode);
        let query = self.renderer.buffer[qr].to_string();

        if self.link_completions.suppressed.as_deref() == Some(query.as_str()) {
            return;
        }

        let cache = self.renderer.files.read().unwrap();
        let mut results = search(&cache, self.file_id, &query, mode);
        drop(cache);
        if results.is_empty() {
            return;
        }

        let Some([cursor_top, cursor_bot]) = self.cursor_line(bracket_start) else {
            return;
        };

        // -- Measure content -------------------------------------------------------
        let text_color = ui.visuals().text_color();
        let hint_color = ui.visuals().weak_text_color();
        let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) { "⌘" } else { "^" };
        let lq = query.trim_end_matches(".md").to_lowercase();

        let shortcuts: Vec<String> = if self.phone_mode {
            Vec::new()
        } else {
            (0..results.len())
                .map(|i| format!("{}{}", modifier, i + 1))
                .collect()
        };

        let theme = ui.ctx().get_lb_theme();
        let share_recipient_color = theme.fg().get_color(theme.prefs().secondary);

        // Measure each name+shortcut label (shortcut is the built-in hint).
        let label_widths: Vec<f32> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut label = GlyphonLabel::new(&r.name, text_color)
                    .font_size(self.renderer.layout.completion_font_size)
                    .line_height(self.renderer.layout.completion_line_height);
                if let Some(shortcut) = shortcuts.get(i) {
                    label = label.hint(shortcut, hint_color);
                }
                label.measure(ui).x
            })
            .collect();

        let measure_path = |text: &str| -> f32 {
            GlyphonLabel::new(text, hint_color)
                .font_size(self.renderer.layout.completion_font_size - 2.0)
                .line_height(self.renderer.layout.completion_line_height)
                .measure(ui)
                .x
        };

        // Abbreviate path segments to fit within a per-row budget so names and
        // hints never overlap regardless of which row is widest.
        for (r, &lw) in results.iter_mut().zip(label_widths.iter()) {
            let budget = (TARGET_POPUP_WIDTH - lw - POPUP_PADDING).max(MIN_HINT_WIDTH);
            abbreviate_segments(&mut r.path_segments, budget, &measure_path);
        }

        // -- Position popup --------------------------------------------------------
        // Width = max per-row total (name+shortcut label + gap + path hint + padding).
        let popup_width = results
            .iter()
            .zip(label_widths.iter())
            .map(|(r, &lw)| {
                let hint: String = r.path_segments.iter().map(|(t, _)| t.as_str()).collect();
                let hw = measure_path(&hint);
                lw + hw + POPUP_PADDING
            })
            .fold(0.0_f32, f32::max);

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
        let mut clicked = None;
        for (idx, _) in results.iter().enumerate() {
            let resp = ui.interact(row_rects[idx], Id::new("link_item").with(idx), Sense::click());
            if resp.clicked() {
                clicked = Some(idx);
            }
        }

        // -- Draw backgrounds ------------------------------------------------------
        self.renderer.draw_completion_popup(
            ui,
            popup_rect,
            &row_rects,
            self.link_completions.selected,
            hover_pos,
        );

        // -- Render text -----------------------------------------------------------
        let clip_rect = ui.clip_rect();
        let mut text_areas: Vec<TextBufferArea> = Vec::new();

        for (idx, result) in results.iter().enumerate() {
            let rect = row_rects[idx];
            let text_top = rect.min.y + 4.0;
            let content_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 8.0, text_top),
                Vec2::new(popup_width - 16.0, self.renderer.layout.completion_line_height),
            );

            // Name (bold on matched chars) + shortcut hint (e.g. ⌘1).
            let flags = match_positions(&lq, &result.name.to_lowercase());
            let spans = build_bold_spans(&result.name, &flags);
            let span_refs: Vec<(&str, bool)> =
                spans.iter().map(|(t, b)| (t.as_str(), *b)).collect();
            let mut label = GlyphonLabel::new_rich(span_refs, text_color)
                .font_size(self.renderer.layout.completion_font_size)
                .line_height(self.renderer.layout.completion_line_height);
            if let Some(shortcut) = shortcuts.get(idx) {
                label = label.hint(shortcut, hint_color);
            }
            let shaped = label.build(ui.ctx());
            let shortcut_width = shaped.hint_size().map_or(0.0, |s| s.x);
            text_areas.extend(shaped.text_areas(content_rect, ui.ctx(), clip_rect));

            // Path hint, right-aligned between name and shortcut.
            let share_color = share_recipient_color;
            let colored_spans: Vec<(&str, Option<egui::Color32>)> = result
                .path_segments
                .iter()
                .map(|(text, shared)| {
                    (text.as_str(), if *shared { Some(share_color) } else { None })
                })
                .collect();
            let shaped = GlyphonLabel::new_colored(colored_spans, hint_color)
                .font_size(self.renderer.layout.completion_font_size - 2.0)
                .line_height(self.renderer.layout.completion_line_height)
                .build(ui.ctx());
            let path_rect = Rect::from_min_size(
                Pos2::new(content_rect.max.x - shortcut_width - 8.0 - shaped.size.x, text_top),
                Vec2::new(shaped.size.x, self.renderer.layout.completion_line_height),
            );
            text_areas.push(shaped.text_area(path_rect, ui.ctx(), clip_rect));
        }

        // Submit after the editor's main text callback so the popup composites on top.
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.max_rect(),
                crate::GlyphonRendererCallback::new(text_areas),
            ));

        // -- Apply clicked result --------------------------------------------------
        if let Some(idx) = clicked {
            let r = &results[idx];
            self.link_completions.apply_completion(
                &mut self.event.internal_events,
                bracket_start,
                replace_end,
                &r.name,
                &r.insert,
                mode,
            );
        }
    }
}
