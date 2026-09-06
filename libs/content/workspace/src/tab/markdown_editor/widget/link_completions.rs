use egui::{Context, Id, Key, Modifiers, Pos2, Rect, Sense, Ui, Vec2};
use lb_rs::Uuid;
use lb_rs::model::file::File;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use std::sync::{Arc, RwLock};

use crate::TextBufferArea;
use crate::file_cache::{FileCache, FilesExt as _, relative_path, strip_ext};
use crate::style::{Space, ThemeExt as _, TypeRole, ellipsize_path, parent_crumbs};
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::bounds::{Paragraphs, RangesExt as _};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::{
    completion_chrome_w, completion_font, completion_line_h, completion_path_font,
    completion_popup_rect, completion_popup_size, completion_row_rects, completion_text_rect,
};
use crate::widgets::GlyphonLabel;
use crate::widgets::glyphon_label::TextOverflow;

const MAX_RESULTS: usize = 7;
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
    /// `[display text](des...` — cursor in the destination; completes file
    /// paths while the display text is kept as-is.
    LinkDest,
    /// `![alt text](des...` — destination of an image link.
    ImageLinkDest,
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
    /// Nucleo path index (same matcher as filename search). Rebuilt when the
    /// file cache's `last_modified` advances.
    searcher: Option<lb_rs::search::PathSearcher>,
    index_mod: u64,
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
        let dest = matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest);
        let qr = query_range(buffer, range, mode);
        let query = &buffer[qr];
        if self.suppressed.as_deref() == Some(query) {
            return;
        }
        // `[` is the markdown-link opener (`[title](path)`), but an empty
        // query is also a footnote (`[^`), a checkbox (`- [`), the first
        // keystroke of `[[`, or a pause before typing a label. Wait for a
        // character. `[[` and `](` still open on empty.
        if mode == CompletionMode::Link && query.is_empty() {
            return;
        }
        // `- [` is far more likely a task item than a link: while the bracket
        // content could still be a checkbox, hold the popup.
        if mode == CompletionMode::Link
            && (query.is_empty() || query == " " || query.eq_ignore_ascii_case("x"))
            && follows_list_marker(buffer, range.0)
        {
            return;
        }
        // External and already-resolved destinations aren't file paths;
        // anchors aren't completed (yet).
        if dest
            && (query.starts_with("http://")
                || query.starts_with("https://")
                || query.starts_with("lb://")
                || query.starts_with('#'))
        {
            return;
        }

        let raw = &buffer[range];
        let complete = match mode {
            CompletionMode::WikiLink => raw.ends_with("]]"),
            CompletionMode::Link | CompletionMode::ImageLink => raw.ends_with(')'),
            // being inside complete syntax is the destination context
            CompletionMode::LinkDest | CompletionMode::ImageLinkDest => false,
        };
        if complete {
            // cursor navigated into existing syntax
            return;
        }

        // Only activate if there are actual results to show — and not when
        // the destination already is one of them (cursor parked on a valid
        // link rather than mid-edit).
        let cache = files.read().unwrap();
        let results = self.search(&cache, file_id, query, mode);
        if results.is_empty() || (dest && results.iter().any(|r| r.insert == query)) {
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
        let results = self.search(&cache, file_id, &query, mode);
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

        let display_for = |r: &FileResult| -> String {
            if matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest) {
                existing_title(&buffer[(bracket_start, replace_end)], mode)
            } else {
                r.name.clone()
            }
        };

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            let idx = self.selected;
            let r = &results[idx];
            self.apply_completion(
                events,
                bracket_start,
                replace_end,
                &display_for(r),
                &r.insert,
                mode,
            );
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
                self.apply_completion(
                    events,
                    bracket_start,
                    replace_end,
                    &display_for(r),
                    &r.insert,
                    mode,
                );
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
            CompletionMode::Link | CompletionMode::LinkDest => {
                format!("[{}]({})", display, path)
            }
            CompletionMode::ImageLink | CompletionMode::ImageLinkDest => {
                format!("![{}]({})", display, path)
            }
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
    if let Some((range, is_image)) = detect_destination(buffer) {
        let mode = if is_image { CompletionMode::ImageLinkDest } else { CompletionMode::LinkDest };
        return Some((range, mode));
    }
    None
}

/// Returns the range of a `[text](path...` link whose *destination* contains
/// the cursor, plus whether it's an image link. The range spans the whole
/// link (through the closing `)` when present) so a picked result replaces
/// it wholesale, keeping the display text (#4893).
fn detect_destination(buffer: &Buffer) -> Option<((Grapheme, Grapheme), bool)> {
    let selection = buffer.current.selection;
    if selection.0 != selection.1 {
        return None;
    }

    let cursor_idx = selection.1.0;
    let len = buffer.current.segs.last_cursor_position().0;

    // Scan backward for the `(` that opened this destination; it must be
    // preceded by `]` (i.e. we're in `](...`, not a bare paren).
    let mut i = cursor_idx;
    let open_paren;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let g = grapheme_at(buffer, i);
        if g == "\n" || g == ")" || g == "[" || g == "]" {
            return None;
        }
        if g == "(" {
            if i == 0 || grapheme_at(buffer, i - 1) != "]" {
                return None;
            }
            open_paren = i;
            break;
        }
        if cursor_idx - i > 200 {
            return None;
        }
    }

    // Scan backward from `](` for the display text's `[`.
    let mut i = open_paren - 1; // at `]`
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
                return None;
            }
            open_bracket = i;
            break;
        }
        if open_paren - i > 200 {
            return None;
        }
    }

    let is_image = open_bracket > 0 && grapheme_at(buffer, open_bracket - 1) == "!";
    let start = if is_image { open_bracket - 1 } else { open_bracket };

    // Forward to the closing `)`; a space or newline ends the destination.
    let mut j = cursor_idx;
    while j < len {
        let g = grapheme_at(buffer, j);
        if g == ")" {
            j += 1;
            break;
        }
        if g == "\n" || g == " " || j - cursor_idx > 200 {
            break;
        }
        j += 1;
    }

    Some(((Grapheme(start), Grapheme(j)), is_image))
}

/// The display text of the link under completion — preserved when a
/// destination completion replaces the whole link.
fn existing_title(raw: &str, mode: CompletionMode) -> String {
    let start = match mode {
        CompletionMode::ImageLink | CompletionMode::ImageLinkDest => 2,
        _ => 1,
    };
    raw[start..]
        .split(']')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Returns the grapheme `&str` at the given char offset.
fn grapheme_at(buffer: &Buffer, i: usize) -> &str {
    &buffer[(Grapheme(i), Grapheme(i + 1))]
}

/// Whether the `[` at `bracket` is the first content of a list item (after
/// optional indentation and blockquote markers) — where it far more likely
/// starts a task checkbox than a link.
fn follows_list_marker(buffer: &Buffer, bracket: Grapheme) -> bool {
    let mut i = bracket.0;
    let mut prefix = String::new();
    while i > 0 {
        let g = grapheme_at(buffer, i - 1);
        if g == "\n" {
            break;
        }
        prefix.insert_str(0, g);
        if prefix.len() > 40 {
            return false; // marker prefixes are short
        }
        i -= 1;
    }

    let mut s = prefix.trim_start();
    while let Some(rest) = s.strip_prefix('>') {
        s = rest.trim_start();
    }
    let s = if let Some(rest) = s.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            return false;
        }
        match s[digits..].strip_prefix(['.', ')']) {
            Some(rest) => rest,
            None => return false,
        }
    };
    // the marker needs trailing whitespace, and nothing else before the `[`
    !s.is_empty() && s.chars().all(|c| c == ' ' || c == '\t')
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
    let raw_full = &buffer[range];
    if matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest) {
        // query = the destination text: after `](`, before any closing `)`
        let dest_byte_start = raw_full.find("](").map(|i| i + 2).unwrap_or(raw_full.len());
        let start = Grapheme(range.0.0 + raw_full[..dest_byte_start].graphemes(true).count());
        let dest = raw_full[dest_byte_start..].trim_end_matches(')');
        let end = Grapheme(start.0 + dest.graphemes(true).count());
        return (start, end);
    }

    let prefix_len = match mode {
        CompletionMode::WikiLink => 2,  // [[
        CompletionMode::Link => 1,      // [
        CompletionMode::ImageLink => 2, // ![
        CompletionMode::LinkDest | CompletionMode::ImageLinkDest => unreachable!(),
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
        CompletionMode::LinkDest | CompletionMode::ImageLinkDest => unreachable!(),
    };

    (start, end)
}

struct FileResult {
    /// The file's UUID.
    id: Uuid,
    /// Display name without .md extension (images keep the extension).
    name: String,
    /// Full relative path from current file (with .md), used for insert.
    rel_path: String,
    /// What to insert: bare title if unique, minimal partial path if colliding.
    insert: String,
    /// True if this file is in a different tree than the current file.
    cross_tree: bool,
    /// Parent path for crumbs (`parent_crumbs` / `ellipsize_path`).
    parent_path: String,
}

impl LinkCompletions {
    fn ensure_index(&mut self, cache: &FileCache) {
        if self.searcher.is_some() && self.index_mod == cache.last_modified {
            return;
        }
        self.searcher = Some(lb_rs::search::PathSearcher::from_files(cache.path_index()));
        self.index_mod = cache.last_modified;
    }

    /// Same nucleo path search as ⌘O. Empty query → recents (mtime), except
    /// image links which recents among image files only.
    fn search(
        &mut self, cache: &FileCache, file_id: Uuid, query: &str, mode: CompletionMode,
    ) -> Vec<FileResult> {
        let image = matches!(mode, CompletionMode::ImageLink | CompletionMode::ImageLinkDest);
        let wiki = mode == CompletionMode::WikiLink;

        let hits: Vec<Uuid> = if query.is_empty() && image {
            let mut images: Vec<&File> = cache
                .all_files()
                .filter(|f| {
                    f.is_document()
                        && f.id != file_id
                        && f.name
                            .rsplit('.')
                            .next()
                            .map(is_supported_image_fmt)
                            .unwrap_or(false)
                })
                .collect();
            images.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
            images.into_iter().take(MAX_RESULTS).map(|f| f.id).collect()
        } else {
            self.ensure_index(cache);
            let searcher = self.searcher.as_mut().unwrap();
            searcher.query(query);
            searcher
                .results()
                .iter()
                .filter(|r| {
                    if r.id == file_id || r.is_folder {
                        return false;
                    }
                    if wiki && !cache.same_tree(file_id, r.id) {
                        return false;
                    }
                    if image {
                        return r
                            .filename
                            .rsplit('.')
                            .next()
                            .map(is_supported_image_fmt)
                            .unwrap_or(false);
                    }
                    true
                })
                .take(MAX_RESULTS)
                .map(|r| r.id)
                .collect()
        };

        let from_id = cache
            .get_by_id(file_id)
            .map(|f| f.parent)
            .unwrap_or(file_id);
        let from_path = cache.path(from_id);

        let mut results: Vec<FileResult> = hits
            .into_iter()
            .filter_map(|id| {
                let f = cache.get_by_id(id)?;
                let cross_tree = !cache.same_tree(file_id, f.id);
                let rel_path = if cross_tree {
                    cache.path(f.id)
                } else {
                    let rp = relative_path(&from_path, &cache.path(f.id));
                    rp.strip_prefix("./").unwrap_or(&rp).to_string()
                };
                let parent_path = cache
                    .get_by_id(f.parent)
                    .map(|p| cache.path(p.id))
                    .unwrap_or_default();
                let name = if image { f.name.clone() } else { strip_ext(&f.name).to_string() };
                Some(FileResult {
                    id: f.id,
                    name,
                    rel_path,
                    insert: String::new(),
                    cross_tree,
                    parent_path,
                })
            })
            .collect();

        populate_insert(cache, &mut results, mode);
        results
    }
}

/// Fills `result.insert` for each entry.
/// - Link/ImageLink: cross-tree uses `lb://uuid` (relative paths don't work
///   across trees); same-tree uses the encoded relative path.
/// - WikiLink: the shortest reference that resolves unambiguously — bare stem,
///   else full name (extension disambiguates), else a relative path. Cross-tree
///   results are filtered upstream (wikilinks are same-tree only).
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

    let docs: Vec<&File> = cache.all_files().filter(|f| f.is_document()).collect();
    let unique = |pred: &dyn Fn(&str) -> bool| docs.iter().filter(|d| pred(&d.name)).count() <= 1;

    for result in results.iter_mut() {
        let Some(f) = cache.get_by_id(result.id) else { continue };
        let full = f.name.clone();
        let stem = strip_ext(&full).to_string();

        if unique(&|n| strip_ext(n).eq_ignore_ascii_case(&stem)) {
            result.insert = stem;
            continue;
        }
        if unique(&|n| n.eq_ignore_ascii_case(&full)) {
            result.insert = full;
            continue;
        }

        // The relative dir locates the folder; within it, the bare stem when
        // unique among siblings, else the full name.
        let sibling_stem_unique = cache
            .children(f.parent)
            .into_iter()
            .filter(|d| d.is_document())
            .filter(|d| strip_ext(&d.name).eq_ignore_ascii_case(&stem))
            .count()
            <= 1;
        let last = if sibling_stem_unique { stem } else { full };
        result.insert = match result.rel_path.rsplit_once('/') {
            Some((dir, _)) => format!("{dir}/{last}"),
            None => last,
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
        let results = self
            .link_completions
            .search(&cache, self.file_id, &query, mode);
        drop(cache);
        if results.is_empty() {
            return;
        }

        let Some([cursor_top, cursor_bot]) = self.cursor_line(bracket_start) else {
            return;
        };

        // -- Measure content -------------------------------------------------------
        let theme = ui.ctx().get_lb_theme();
        let text_color = theme.neutral_fg();
        let hint_color = theme.neutral_fg_secondary();
        let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) { "⌘" } else { "^" };
        let lq = strip_ext(&query).to_lowercase();

        let shortcuts: Vec<String> = if self.phone_mode {
            Vec::new()
        } else {
            (0..results.len())
                .map(|i| format!("{}{}", modifier, i + 1))
                .collect()
        };

        // Measure each name+shortcut label (shortcut is the built-in hint).
        let label_widths: Vec<f32> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut label = GlyphonLabel::new(&r.name, text_color)
                    .font_size(completion_font())
                    .line_height(completion_line_h());
                if let Some(shortcut) = shortcuts.get(i) {
                    label = label.hint(shortcut, hint_color);
                }
                label.measure(ui).x
            })
            .collect();

        let measure_path = |text: &str| -> f32 {
            GlyphonLabel::new(text, hint_color)
                .font_size(completion_path_font())
                .line_height(TypeRole::Mono.line_height())
                .measure(ui)
                .x
        };

        // Shared crumbs + middle-ellipsis (same as Files / search subtitles).
        let path_hints: Vec<String> = results
            .iter()
            .zip(label_widths.iter())
            .map(|(r, &lw)| {
                let crumbs = parent_crumbs(&r.parent_path);
                let budget = (TARGET_POPUP_WIDTH - lw - completion_chrome_w()).max(MIN_HINT_WIDTH);
                ellipsize_path(ui, &crumbs, budget)
            })
            .collect();

        // -- Position popup --------------------------------------------------------
        // Width = max per-row total (name+shortcut label + gap + path hint + padding).
        let popup_width = path_hints
            .iter()
            .zip(label_widths.iter())
            .map(|(hint, &lw)| lw + measure_path(hint) + completion_chrome_w())
            .fold(0.0_f32, f32::max);

        let popup_rect = completion_popup_rect(
            cursor_top,
            cursor_bot,
            completion_popup_size(popup_width - completion_chrome_w(), results.len()),
            ui.ctx().screen_rect(),
        );
        self.renderer.touch_consuming_rects.push(popup_rect);

        let row_rects = completion_row_rects(popup_rect, results.len());

        // Re-ellipsize crumbs against the clamped row so name / path / shortcut
        // don't overlap when the window is narrower than the target popup.
        let path_gap = Space::Xs.pts();
        let mut row_fit: Vec<(String, f32, bool)> = Vec::with_capacity(results.len());
        for (idx, result) in results.iter().enumerate() {
            let content_rect = completion_text_rect(row_rects[idx]);
            let shortcut_w = shortcuts.get(idx).map_or(0.0, |s| {
                GlyphonLabel::new(s, hint_color)
                    .font_size(completion_path_font())
                    .line_height(completion_line_h())
                    .measure(ui)
                    .x
            });
            let hint_gap = if shortcut_w > 0.0 { Space::Xs.pts() } else { 0.0 };
            let available = (content_rect.width() - shortcut_w - hint_gap).max(1.0);
            let name_w = GlyphonLabel::new(&result.name, text_color)
                .font_size(completion_font())
                .line_height(completion_line_h())
                .measure(ui)
                .x;
            let crumbs = parent_crumbs(&result.parent_path);
            let path_budget = if name_w + MIN_HINT_WIDTH + path_gap <= available {
                available - name_w - path_gap
            } else {
                MIN_HINT_WIDTH
            };
            let path = ellipsize_path(ui, &crumbs, path_budget.max(1.0));
            let path_w = measure_path(&path);
            let name_max = (available - path_w - path_gap).max(40.0);
            row_fit.push((path, name_max, name_w > name_max + 0.5));
        }

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
            let content_rect = completion_text_rect(row_rects[idx]);
            let (path, name_max, ellipsize_name) = &row_fit[idx];
            let shortcut = shortcuts.get(idx).map(String::as_str);

            let flags = match_positions(&lq, &result.name.to_lowercase());
            let spans = build_bold_spans(&result.name, &flags);
            let span_refs: Vec<(&str, bool)> =
                spans.iter().map(|(t, b)| (t.as_str(), *b)).collect();
            let mut label = if *ellipsize_name {
                GlyphonLabel::new(&result.name, text_color)
                    .font_size(completion_font())
                    .line_height(completion_line_h())
                    .max_width(*name_max)
                    .text_overflow(TextOverflow::EndEllipsis)
            } else {
                GlyphonLabel::new_rich(span_refs, text_color)
                    .font_size(completion_font())
                    .line_height(completion_line_h())
            };
            if let Some(s) = shortcut {
                label = label.hint(s, hint_color);
            }
            let shaped = label.build(ui.ctx());
            let shortcut_width = shaped.hint_size().map_or(0.0, |s| s.x);
            text_areas.extend(shaped.text_areas(content_rect, ui.ctx(), clip_rect));

            let shaped = GlyphonLabel::new(path, hint_color)
                .font_size(completion_path_font())
                .line_height(TypeRole::Mono.line_height())
                .build(ui.ctx());
            let path_h = TypeRole::Mono.line_height();
            let path_rect = Rect::from_min_size(
                Pos2::new(
                    content_rect.max.x - shortcut_width - path_gap - shaped.size.x,
                    content_rect.center().y - path_h / 2.0,
                ),
                Vec2::new(shaped.size.x, path_h),
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
            let display =
                if matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest) {
                    existing_title(&self.renderer.buffer[(bracket_start, replace_end)], mode)
                } else {
                    r.name.clone()
                };
            self.link_completions.apply_completion(
                &mut self.event.internal_events,
                bracket_start,
                replace_end,
                &display,
                &r.insert,
                mode,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use lb_rs::Uuid;
    use lb_rs::model::file::File;
    use lb_rs::model::file_metadata::FileType;

    use super::{CompletionMode, LinkCompletions};
    use crate::file_cache::{FileCache, FilesExt as _};

    fn file(id: Uuid, parent: Uuid, name: &str, file_type: FileType) -> File {
        File {
            id,
            parent,
            name: name.into(),
            file_type,
            last_modified: 0,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: vec![],
            size_bytes: 0,
        }
    }

    fn build(root: File, rest: Vec<File>) -> FileCache {
        FileCache::from_owned_and_shared(root, rest, [])
    }

    /// The completions/resolver contract: every wikilink completion offered
    /// resolves back to the file it was for. Covers all three insert forms —
    /// bare stem (`todo`), full name when the stem collides (`Pancakes.md`),
    /// and a relative path when the full name collides across folders (`a/Spec`).
    #[test]
    fn list_markers_hold_the_popup() {
        use lb_rs::model::text::buffer::Buffer;
        use lb_rs::model::text::offset_types::Grapheme;

        use super::follows_list_marker;

        let at_bracket = |md: &str| {
            let buffer = Buffer::from(md);
            let bracket = md.rfind('[').unwrap();
            follows_list_marker(&buffer, Grapheme(bracket))
        };
        assert!(at_bracket("- ["));
        assert!(at_bracket("  * ["));
        assert!(at_bracket("1. ["));
        assert!(at_bracket("12) ["));
        assert!(at_bracket("> - ["));
        assert!(at_bracket("text\n- ["));
        assert!(!at_bracket("some ["));
        assert!(!at_bracket("-[")); // no space: not a list marker
        assert!(!at_bracket("a - ["));
        assert!(!at_bracket("["));
    }

    #[test]
    fn destination_context_detected_and_title_kept() {
        use lb_rs::model::text::buffer::Buffer;
        use lb_rs::model::text::offset_types::Grapheme;

        use super::{detect_any, existing_title, query_range};

        // cursor mid-destination of a complete link
        let mut buffer = Buffer::from("see [My Title](qu) after");
        buffer.current.selection = (Grapheme(17), Grapheme(17));
        let ((start, end), mode) = detect_any(&buffer).expect("destination context");
        assert!(matches!(mode, CompletionMode::LinkDest));
        assert_eq!(&buffer[(start, end)], "[My Title](qu)");
        let qr = query_range(&buffer, (start, end), mode);
        assert_eq!(&buffer[qr], "qu");
        assert_eq!(existing_title(&buffer[(start, end)], mode), "My Title");

        // image link, destination not yet closed
        let mut buffer = Buffer::from("![alt](par");
        buffer.current.selection = (Grapheme(10), Grapheme(10));
        let ((start, end), mode) = detect_any(&buffer).expect("image destination context");
        assert!(matches!(mode, CompletionMode::ImageLinkDest));
        assert_eq!(&buffer[(start, end)], "![alt](par");
        let qr = query_range(&buffer, (start, end), mode);
        assert_eq!(&buffer[qr], "par");
        assert_eq!(existing_title(&buffer[(start, end)], mode), "alt");

        // cursor in the display text is the title context, not destination
        let mut buffer = Buffer::from("[ti](dest)");
        buffer.current.selection = (Grapheme(3), Grapheme(3));
        let (_, mode) = detect_any(&buffer).expect("title context");
        assert!(matches!(mode, CompletionMode::Link));

        // a bare paren isn't a destination
        let mut buffer = Buffer::from("just (parens");
        buffer.current.selection = (Grapheme(9), Grapheme(9));
        assert!(detect_any(&buffer).is_none());
    }

    #[test]
    fn destination_completion_replaces_link_keeping_title() {
        use super::LinkCompletions;
        use crate::tab::markdown_editor::input::Event;
        use lb_rs::model::text::offset_types::Grapheme;

        let mut lc = LinkCompletions::default();
        let mut events: Vec<Event> = vec![];
        lc.apply_completion(
            &mut events,
            Grapheme(4),
            Grapheme(18),
            "My Title",
            "notes/target.md",
            CompletionMode::LinkDest,
        );
        match &events[..] {
            [Event::Replace { text, .. }] => {
                assert_eq!(text, "[My Title](notes/target.md)");
            }
            other => panic!("expected one replace, got {other:?}"),
        }
    }

    #[test]
    fn wikilink_completions_resolve_back() {
        let root_id = Uuid::new_v4();
        let root = file(root_id, root_id, "root", FileType::Folder);
        let (recipes, a, b, editing) =
            (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let doc = |parent, name| file(Uuid::new_v4(), parent, name, FileType::Document);

        let cache = build(
            root,
            vec![
                file(editing, root_id, "editing.md", FileType::Document),
                doc(root_id, "todo.txt"),
                file(recipes, root_id, "recipes", FileType::Folder),
                doc(recipes, "Pancakes.md"),
                doc(recipes, "Pancakes.svg"),
                file(a, root_id, "a", FileType::Folder),
                doc(a, "Spec.md"),
                file(b, root_id, "b", FileType::Folder),
                doc(b, "Spec.md"),
            ],
        );
        let from_id = cache.get_by_id(editing).unwrap().parent;

        let mut lc = LinkCompletions::default();
        for query in ["pan", "todo", "spec"] {
            let results = lc.search(&cache, editing, query, CompletionMode::WikiLink);
            assert!(!results.is_empty(), "query {query:?} produced no completions");
            for r in results {
                assert_eq!(
                    cache.resolve_wikilink(&r.insert, from_id),
                    Some(r.id),
                    "query {query:?}: insert {:?} did not resolve to {:?}",
                    r.insert,
                    r.name,
                );
            }
        }
    }

    /// Filename search matches the full path; completions should too
    /// (`Buildinga` → `building/Android`, #4962).
    #[test]
    fn path_query_matches_folder_and_leaf() {
        let root_id = Uuid::new_v4();
        let root = file(root_id, root_id, "root", FileType::Folder);
        let (building, android, editing) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let cache = build(
            root,
            vec![
                file(editing, root_id, "editing.md", FileType::Document),
                file(building, root_id, "building", FileType::Folder),
                file(android, building, "Android.md", FileType::Document),
            ],
        );
        let mut lc = LinkCompletions::default();
        // Lowercase: nucleo Smart case is case-insensitive (same as ⌘O).
        let results = lc.search(&cache, editing, "buildinga", CompletionMode::WikiLink);
        assert!(
            results.iter().any(|r| r.id == android),
            "buildinga should suggest building/Android, got {:?}",
            results.iter().map(|r| &r.name).collect::<Vec<_>>(),
        );
    }
}
