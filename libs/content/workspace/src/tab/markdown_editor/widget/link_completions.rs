use egui::{Context, Id, Key, Modifiers, Pos2, Rect, Sense, Ui, Vec2};
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use std::sync::{Arc, RwLock};

use lb_rs::model::file::File;

use crate::TextBufferArea;
use crate::file_cache::{
    FileCache, FilesExt as _, Reach, link_id, relative_path, split_fragment, strip_ext, web_link,
    wiki_fragment, wiki_title,
};
use crate::style::{
    Space, ThemeExt as _, TypeRole, crumb_label, ellipsize_path, ellipsize_path_tail, parent_crumbs,
};
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::bounds::{Paragraphs, RangesExt as _};
use crate::tab::markdown_editor::fragment::{Heading, crumbs, document_headings, wiki_token};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::{
    completion_chrome_w, completion_font, completion_line_h, completion_line_rects,
    completion_path_font, completion_popup_rect, completion_popup_size, completion_row_h,
    completion_text_rect,
};
use crate::widgets::GlyphonLabel;
use crate::widgets::glyphon_label::TextOverflow;

const MAX_RESULTS: usize = 7;
const TARGET_POPUP_WIDTH: f32 = 320.0; // soft target per row; popup grows to fit actual content
const MIN_HINT_WIDTH: f32 = 60.0; // always leave at least this much room for the hint
const OUTSIDE_DOT: f32 = 3.0; // radius of the dot before the path of a file outside the note's scope
const HEADINGS_SLOT: f32 = 32.0; // the `#` button at the end of a note's row
const HEADINGS_SLOT_TOUCH: f32 = 44.0; // the same, a thumb wide

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
    /// The buffer state (`Snapshot::seq`) and query Escape dismissed the
    /// popup in; an edit brings it back (seq alone repeats after an undo).
    suppressed: Option<(usize, String)>,
    /// The files the note could link to in this mode, with what ranking
    /// them takes. Rebuilt when the file tree, the note, or the mode changes.
    candidates: Vec<Candidate>,
    candidates_for: Option<(u64, Uuid, CompletionMode)>,
    /// Last `search` result this frame / keystroke.
    cached: Option<(u64, Uuid, CompletionMode, String, Vec<FileResult>)>,
    /// Where this account's external file links point, for links to files
    /// outside the note's scope; `app.lockbook.net` when unknown.
    open_origin: Option<String>,
    /// This note's headings, for `[[#`, with the text they were read from.
    own_headings: Option<(String, Arc<Vec<Heading>>)>,
}

impl LinkCompletions {
    pub fn update_active_state(
        &mut self, buffer: &Buffer, inline_paragraphs: &Paragraphs, files: &Arc<RwLock<FileCache>>,
        file_id: Uuid,
    ) {
        let note = &buffer.current.text;
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
        if self.is_suppressed(buffer, query) {
            return;
        }
        // `[` is the markdown-link opener (`[title](path)`), but an empty
        // query is also a footnote (`[^`), a checkbox (`- [`), the first
        // keystroke of `[[`, or a pause before typing a label. Wait for a
        // character, unless the whole empty link is there, as the toolbar
        // writes it. `[[` and `](` still open on empty.
        let fresh = match mode {
            CompletionMode::Link => &buffer[range] == "[]()",
            CompletionMode::ImageLink => &buffer[range] == "![]()",
            _ => false,
        };
        if mode == CompletionMode::Link && query.is_empty() && !fresh {
            return;
        }
        // `- [` is far more likely a task item than a link: while the bracket
        // content could still be a checkbox, hold the popup.
        if mode == CompletionMode::Link
            && !fresh
            && (query.is_empty() || query == " " || query.eq_ignore_ascii_case("x"))
            && follows_list_marker(buffer, range.0)
        {
            return;
        }
        // a web URL isn't a file path, and an id link already names one
        // until a `#` asks for its headings
        if dest
            && (web_link(query) || link_id(query).is_some())
            && split_fragment(query).1.is_none()
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
        if complete && !fresh {
            // cursor navigated into existing syntax
            return;
        }

        // Only activate if there are actual results to show — and not when
        // the destination already is one of them (cursor parked on a valid
        // link rather than mid-edit).
        let cache = files.read().unwrap();
        let results = self.search(&cache, file_id, query, mode, note);
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
        if self.is_suppressed(buffer, &query) {
            return;
        }

        let cache = files.read().unwrap();
        let results = self.search(&cache, file_id, &query, mode, &buffer.current.text);
        drop(cache);
        if results.is_empty() {
            return;
        }
        self.selected = self.selected.min(results.len() - 1);

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.suppressed = Some((buffer.current.seq, query));
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

    fn is_suppressed(&self, buffer: &Buffer, query: &str) -> bool {
        self.suppressed
            .as_ref()
            .is_some_and(|(seq, q)| *seq == buffer.current.seq && q == query)
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
pub(crate) fn detect_destination(buffer: &Buffer) -> Option<((Grapheme, Grapheme), bool)> {
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
pub(crate) fn detect_wikilink(buffer: &Buffer) -> Option<(Grapheme, Grapheme)> {
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
pub(crate) fn detect_link(buffer: &Buffer) -> Option<((Grapheme, Grapheme), bool)> {
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

#[derive(Clone)]
struct FileResult {
    /// The file's UUID.
    id: Uuid,
    /// Display name without .md extension (images keep the extension).
    name: String,
    /// What to insert: a wikilink title, or a link destination.
    insert: String,
    /// True if this file is outside the scope of the current file, so the
    /// link won't open the same file for every reader.
    outside: bool,
    /// How `insert` reaches the file for this reader: past the scope, or not
    /// at all. None inside the scope.
    reach: Option<Option<Reach>>,
    /// The file's path, for its folder crumbs.
    path: String,
    /// A heading's own crumbs, the headings above it, shown in place of
    /// the folders.
    crumbs: Option<String>,
    /// A note whose headings the row's `#` button lists.
    headings: bool,
    /// For a heading, the note it is in, which heads its group of rows.
    group: Option<String>,
}

impl FileResult {
    fn crumbs(&self) -> String {
        self.crumbs
            .clone()
            .unwrap_or_else(|| parent_crumbs(&self.path))
    }
}

/// A file a completion could link to, with what ranking it takes.
struct Candidate {
    id: Uuid,
    /// Outside the note's scope, where only an id link reaches it.
    outside: bool,
    /// Folders between the note and the file.
    distance: usize,
    /// Lowercased name, name less its extension, and path.
    name: String,
    stem: String,
    path: String,
    /// The way from the note to the file, lowercased: its path past the
    /// folders they share. A file outside the scope has no such way, and
    /// keeps its whole path.
    way: String,
}

/// How a query matches a file, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Match {
    /// The name starts with the query.
    Prefix,
    /// The name has the query in it.
    Contains,
    /// The way to the file has each word of the query's letters, in order.
    Fuzzy,
}

impl Candidate {
    fn matches(&self, query: &str) -> Option<Match> {
        let scattered = |word: &str| {
            let mut way = self.way.chars();
            word.chars().all(|c| way.any(|w| w == c))
        };
        if self.name.starts_with(query) {
            Some(Match::Prefix)
        } else if self.name.contains(query) {
            Some(Match::Contains)
        } else {
            query
                .split_whitespace()
                .all(scattered)
                .then_some(Match::Fuzzy)
        }
    }
}

/// A wikilink title as comrak reads it back: [`crate::file_cache::escape_wiki_title`] undone.
fn unescape_wiki_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut chars = title.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('\\', Some('[' | ']' | '|' | '\\')) => {}
            _ => out.push(c),
        }
    }
    out
}

/// A query as it is matched: lowercased, and for one typed as a destination,
/// percent-escapes decoded and the `./` and `../` a path leads with dropped.
fn normalized(query: &str, mode: CompletionMode) -> String {
    if !matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest) {
        return query.to_lowercase();
    }
    let query = urlencoding::decode(query).map_or(query.into(), |q| q.into_owned());
    let mut query = query.as_str();
    while let Some(rest) = query.strip_prefix("./").or(query.strip_prefix("../")) {
        query = rest;
    }
    query.to_lowercase()
}

impl LinkCompletions {
    pub fn new(open_origin: Option<String>) -> Self {
        Self { open_origin, ..Default::default() }
    }

    /// Reads other notes for their headings with `core`, repainting `ctx`
    /// as each arrives.
    fn ensure_candidates(&mut self, cache: &FileCache, file_id: Uuid, mode: CompletionMode) {
        let key = Some((cache.last_modified, file_id, mode));
        if self.candidates_for == key {
            return;
        }
        let image = matches!(mode, CompletionMode::ImageLink | CompletionMode::ImageLinkDest);
        let scope = cache.scope_top(file_id);
        let from = cache.get_by_id(file_id).map_or(file_id, |f| f.parent);
        let from_path = cache.path(from);
        self.candidates = cache
            .all_files()
            .filter(|f| f.is_document() && f.id != file_id)
            .filter(|f| {
                !image
                    || f.name
                        .rsplit('.')
                        .next()
                        .is_some_and(is_supported_image_fmt)
            })
            .map(|f| {
                let outside = f.id == scope || !cache.in_scope(scope, f.id);
                let name = f.name.to_lowercase();
                let path = cache.path(f.id);
                let way = match outside {
                    true => path.clone(),
                    false => relative_path(&from_path, &path).replace("../", ""),
                };
                Candidate {
                    id: f.id,
                    outside,
                    distance: cache.folder_distance(from, f.id),
                    stem: strip_ext(&name).to_string(),
                    name,
                    path: path.to_lowercase(),
                    way: way.to_lowercase(),
                }
            })
            .collect();
        self.candidates_for = key;
    }

    /// Files in the note's scope first, then the rest, which only an id link
    /// reaches. Each is ranked by how the query matches, then by folders from
    /// the note, then by name — an order the query takes no part in, so a
    /// name on top by its first letters stays there as the rest are typed.
    fn search(
        &mut self, cache: &FileCache, file_id: Uuid, query: &str, mode: CompletionMode, note: &str,
    ) -> Vec<FileResult> {
        let heading_query = match mode {
            CompletionMode::WikiLink => wiki_fragment(query),
            CompletionMode::LinkDest => split_fragment(query),
            _ => (query, None),
        };
        let heading_query = heading_query.1.map(|needle| (heading_query.0, needle));
        if let Some((file_part, needle)) = heading_query {
            return self.heading_results(cache, file_id, file_part, needle, mode, note);
        }
        if let Some((m, id, mode_c, q, results)) = &self.cached {
            if *m == cache.last_modified && *id == file_id && *mode_c == mode && q == query {
                return results.clone();
            }
        }

        self.ensure_candidates(cache, file_id, mode);
        let normalized = normalized(query, mode);
        let mut ranked: Vec<(Match, &Candidate)> = self
            .candidates
            .iter()
            .filter_map(|c| Some((c.matches(&normalized)?, c)))
            .collect();
        ranked.sort_by(|(a_match, a), (b_match, b)| {
            (a.outside, a_match, a.distance, &a.stem, &a.name, &a.path)
                .cmp(&(b.outside, b_match, b.distance, &b.stem, &b.name, &b.path))
        });
        ranked.truncate(MAX_RESULTS);

        let image = matches!(mode, CompletionMode::ImageLink | CompletionMode::ImageLinkDest);
        let origin = self
            .open_origin
            .as_deref()
            .unwrap_or("https://app.lockbook.net");
        let results: Vec<FileResult> = ranked
            .into_iter()
            .filter_map(|(_, candidate)| {
                let f = cache.get_by_id(candidate.id)?;
                let name = if image { f.name.clone() } else { strip_ext(&f.name).to_string() };
                // a wikilink title, or a link destination: a relative path in
                // the scope, the external URL out of it, never an absolute
                // path; a title past the scope is unique in the reader's tree
                let (insert, located) = if mode == CompletionMode::WikiLink {
                    let title = cache
                        .wikilink_title(f.id, file_id)
                        .or_else(|| cache.wikilink_title_in(f.id, cache.tree_root(file_id)))
                        .unwrap_or_else(|| wiki_title(&f.name).to_string());
                    // resolved as the note will read it, escapes undone
                    let located = cache.locate_wikilink(&unescape_wiki_title(&title), file_id);
                    (title, located)
                } else {
                    let destination = match cache.link_destination(f.id, file_id) {
                        Some(path) => path,
                        None => format!("{origin}/open/{}", f.id),
                    };
                    let located = cache.locate_link(&destination, file_id);
                    (destination, located)
                };
                let (outside, path) = (candidate.outside, cache.path(f.id));
                // the dot is the color the link will draw in: none where it
                // opens this file for everyone, red where it opens no file or
                // another one
                let reach = match located.filter(|(id, _)| *id == f.id) {
                    Some((_, Reach::Scope)) => None,
                    located => outside.then_some(located.map(|(_, reach)| reach)),
                };
                let headings = !image && f.name.to_lowercase().ends_with(".md");
                Some(FileResult {
                    id: f.id,
                    name,
                    insert,
                    outside,
                    reach,
                    path,
                    crumbs: None,
                    headings,
                    group: None,
                })
            })
            .collect();

        self.cached =
            Some((cache.last_modified, file_id, mode, query.to_string(), results.clone()));
        results
    }

    /// Rows for a query that names a heading, `note#heading`: up to seven
    /// pairs of a note the part before the `#` names and a heading of it that
    /// has what follows. The note is the one that part resolves to, or else
    /// every note whose name has it in it, in the file search's order; `#`
    /// alone is this note. The best seven by headings starting with the
    /// query, then the notes' order, then document order, shown grouped by
    /// note under its name.
    fn heading_results(
        &mut self, cache: &FileCache, file_id: Uuid, file_part: &str, needle: &str,
        mode: CompletionMode, note: &str,
    ) -> Vec<FileResult> {
        let wiki = mode == CompletionMode::WikiLink;
        let markdown = |f: &File| f.name.to_lowercase().ends_with(".md");
        // the notes, each with how a link writes it
        let mut notes: Vec<(Uuid, String)> = vec![];
        if file_part.is_empty() {
            notes.push((file_id, String::new()));
        } else {
            let located = match wiki {
                true => cache.locate_wikilink(&unescape_wiki_title(file_part), file_id),
                false => cache.locate_link(file_part, file_id),
            };
            match located
                .and_then(|(id, _)| cache.get_by_id(id))
                .filter(|f| markdown(f))
            {
                Some(f) => notes.push((f.id, file_part.to_string())),
                None => {
                    let typed = normalized(file_part, mode);
                    let typed = strip_ext(typed.rsplit('/').next().unwrap_or(&typed)).to_string();
                    for r in self.search(cache, file_id, file_part, mode, note) {
                        if r.headings && r.name.to_lowercase().contains(&typed) {
                            notes.push((r.id, r.insert));
                        }
                    }
                }
            }
        }
        // a note's name, or its shortest title where two names are the same
        let stem = |id: Uuid| {
            cache
                .get_by_id(id)
                .map_or(String::new(), |f| strip_ext(&f.name).to_string())
        };
        let labels: Vec<String> = notes
            .iter()
            .map(|(id, _)| {
                let name = stem(*id);
                let shared = notes
                    .iter()
                    .filter(|(other, _)| stem(*other) == name)
                    .count()
                    > 1;
                let title = shared
                    .then(|| {
                        cache
                            .wikilink_title(*id, file_id)
                            .or_else(|| cache.wikilink_title_in(*id, cache.tree_root(file_id)))
                    })
                    .flatten();
                title.map_or(name, |t| unescape_wiki_title(&t))
            })
            .collect();

        let needle = needle.trim().to_lowercase();
        let tier = |h: &Heading| {
            let text = h.text.to_lowercase();
            if text.starts_with(&needle) {
                Some(0)
            } else if text.contains(&needle) || h.slug.contains(&needle) {
                Some(1)
            } else {
                None
            }
        };
        let per_note: Vec<Arc<Vec<Heading>>> = notes
            .iter()
            .map(|(id, _)| match *id == file_id {
                true => self.own_headings(note),
                false => cache.notes.headings(*id).unwrap_or_default(),
            })
            .collect();
        let mut hits: Vec<(u8, usize, usize)> = vec![];
        for (rank, headings) in per_note.iter().enumerate() {
            for (i, h) in headings.iter().enumerate() {
                if let Some(t) = tier(h) {
                    hits.push((t, rank, i));
                }
            }
        }
        hits.sort();
        hits.truncate(MAX_RESULTS);
        // grouped by note, groups in the order their best rows came
        let mut groups: Vec<usize> = vec![];
        for (_, rank, _) in &hits {
            if !groups.contains(rank) {
                groups.push(*rank);
            }
        }
        hits.sort_by_key(|(_, rank, _)| groups.iter().position(|g| g == rank));
        hits.into_iter()
            .map(|(_, rank, i)| {
                let (id, insert) = &notes[rank];
                let headings = &per_note[rank];
                let h = &headings[i];
                let token = if wiki { wiki_token(h) } else { h.slug.clone() };
                // the slug names it, repeats included; the headings above
                // place it; its note heads the group
                FileResult {
                    id: *id,
                    name: h.slug.clone(),
                    insert: format!("{insert}#{token}"),
                    outside: false,
                    reach: None,
                    path: String::new(),
                    crumbs: Some(crumbs(headings, i)),
                    headings: false,
                    group: Some(labels[rank].clone()),
                }
            })
            .collect()
    }

    /// This note's headings, read again only once its text changes.
    fn own_headings(&mut self, note: &str) -> Arc<Vec<Heading>> {
        if let Some((text, headings)) = &self.own_headings {
            if text == note {
                return headings.clone();
            }
        }
        let headings = Arc::new(document_headings(note));
        self.own_headings = Some((note.to_string(), headings.clone()));
        headings
    }
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

        if self
            .link_completions
            .is_suppressed(&self.renderer.buffer, &query)
        {
            return;
        }

        let cache = self.renderer.files.read().unwrap();
        let note = &self.renderer.buffer.current.text;
        let results = self
            .link_completions
            .search(&cache, self.file_id, &query, mode, note);
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
        // what the row names was typed after the `#` for a heading
        let lq = match query.split_once('#') {
            Some((_, needle)) if results[0].group.is_some() => {
                crate::tab::markdown_editor::fragment::slugify(needle)
            }
            _ => strip_ext(&query).to_lowercase(),
        };

        let shortcuts: Vec<String> = if self.phone_mode {
            Vec::new()
        } else {
            (0..results.len())
                .map(|i| format!("{}{}", modifier, i + 1))
                .collect()
        };

        let measure = |text: &str, size: f32, line_height: f32| -> f32 {
            GlyphonLabel::new(text, hint_color)
                .font_size(size)
                .line_height(line_height)
                .measure(ui)
                .x
        };
        let measure_path =
            |text: &str| measure(text, completion_path_font(), TypeRole::Mono.line_height());
        let name_widths: Vec<f32> = results
            .iter()
            .map(|r| measure(&r.name, completion_font(), completion_line_h()))
            .collect();
        // the shortcut at the row's right edge, and the gap before it
        let tails: Vec<f32> = (0..results.len())
            .map(|i| {
                shortcuts.get(i).map_or(0.0, |s| {
                    measure(s, completion_path_font(), completion_line_h()) + Space::Xs.pts()
                })
            })
            .collect();
        let path_gap = Space::Sm.pts();
        // the dot and the gap after it, before the path of a file outside
        // the scope: the color the link will be drawn in
        let lead =
            |r: &FileResult| if r.outside { 2.0 * OUTSIDE_DOT + Space::Xs.pts() } else { 0.0 };
        // a note's `#` button, past the shortcut
        let can_list_headings = matches!(
            mode,
            CompletionMode::WikiLink | CompletionMode::Link | CompletionMode::LinkDest
        );
        let touch = self.renderer.touch_mode;
        let slot_w = if touch { HEADINGS_SLOT_TOUCH } else { HEADINGS_SLOT };
        let headings_slot = |r: &FileResult| {
            if can_list_headings && r.headings { slot_w } else { 0.0 }
        };
        let text_rect = |row: Rect, r: &FileResult| {
            let rect = completion_text_rect(row);
            rect.with_max_x(rect.max.x - headings_slot(r))
        };

        // Folders shorten in the middle (as Files and search do); the headings
        // above a heading from the outside in, its nearest parent kept.
        let path_hint = |r: &FileResult, budget: f32| -> String {
            match r.group {
                Some(_) => ellipsize_path_tail(ui, &r.crumbs(), budget.max(1.0)),
                None => ellipsize_path(ui, &r.crumbs(), budget.max(1.0)),
            }
        };
        // a heading's note heads its group of rows
        let headers: Vec<(usize, &str)> = results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let group = r.group.as_deref()?;
                let first = i == 0 || results[i - 1].group.as_deref() != Some(group);
                first.then_some((i, group))
            })
            .collect();
        let path_hints: Vec<String> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let taken = name_widths[i]
                    + path_gap
                    + lead(r)
                    + tails[i]
                    + headings_slot(r)
                    + completion_chrome_w();
                path_hint(r, (TARGET_POPUP_WIDTH - taken).max(MIN_HINT_WIDTH))
            })
            .collect();

        // -- Position popup --------------------------------------------------------
        // Width = the widest row: name, gap, dot, path hint, shortcut, padding.
        let popup_width = path_hints
            .iter()
            .zip(&results)
            .enumerate()
            .map(|(i, (hint, r))| {
                name_widths[i]
                    + path_gap
                    + lead(r)
                    + measure_path(hint)
                    + tails[i]
                    + headings_slot(r)
                    + completion_chrome_w()
            })
            .chain(
                headers
                    .iter()
                    .map(|(_, group)| measure_path(group) + completion_chrome_w()),
            )
            .fold(0.0_f32, f32::max);

        // a header is a short line, its label low, so it sits with the rows
        // it names; the rule above it parts it from the group before
        let header_h = TypeRole::Mono.line_height() + Space::Xs.pts();
        let mut size = completion_popup_size(popup_width - completion_chrome_w(), results.len());
        size.y += headers.len() as f32 * header_h;
        let popup_rect =
            completion_popup_rect(cursor_top, cursor_bot, size, ui.ctx().content_rect());
        self.renderer
            .touch_targets
            .push((popup_rect, TouchTarget::Popup));

        // each header takes a line above its group's first row
        let heights: Vec<f32> = (0..results.len())
            .flat_map(|i| {
                let header = headers.iter().any(|(at, _)| *at == i);
                header
                    .then_some(header_h)
                    .into_iter()
                    .chain([completion_row_h()])
            })
            .collect();
        let mut lines = completion_line_rects(popup_rect, &heights).into_iter();
        let mut row_rects = Vec::with_capacity(results.len());
        let mut header_rects: Vec<(Rect, &str)> = vec![];
        for i in 0..results.len() {
            if let Some((_, group)) = headers.iter().find(|(at, _)| *at == i) {
                header_rects.push((lines.next().unwrap_or(Rect::NOTHING), group));
            }
            row_rects.push(lines.next().unwrap_or(Rect::NOTHING));
        }

        // Re-ellipsize crumbs against the clamped row so name / path / shortcut
        // don't overlap when the window is narrower than the target popup.
        let mut row_fit: Vec<(String, f32, bool)> = Vec::with_capacity(results.len());
        for (idx, result) in results.iter().enumerate() {
            let content_rect = text_rect(row_rects[idx], result);
            let available = (content_rect.width() - tails[idx] - lead(result)).max(1.0);
            let name_w = name_widths[idx];
            let path_budget = if name_w + MIN_HINT_WIDTH + path_gap <= available {
                available - name_w - path_gap
            } else {
                MIN_HINT_WIDTH
            };
            let path = path_hint(result, path_budget.max(1.0));
            let path_w = measure_path(&path);
            let name_max = (available - path_w - path_gap).max(40.0);
            row_fit.push((path, name_max, name_w > name_max + 0.5));
        }

        // -- Interaction -----------------------------------------------------------
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked = None;
        for (idx, result) in results.iter().enumerate() {
            let resp = ui.interact(
                row_rects[idx],
                Id::unique("link_item").with((result.id, idx)),
                Sense::click(),
            );
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                clicked = Some(idx);
            }
        }
        let mut list_headings = None;
        let mut hash_buttons = vec![];
        for (idx, result) in results.iter().enumerate() {
            if headings_slot(result) == 0.0 {
                continue;
            }
            let row = row_rects[idx];
            let rect = Rect::from_min_max(Pos2::new(row.right() - slot_w, row.top()), row.max);
            let id = Id::unique("link_item_headings").with((result.id, idx));
            let resp = ui.interact(rect, id, Sense::click());
            crate::style::tip_text(ui.ctx(), &resp, "Headings");
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                list_headings = Some(idx);
                clicked = None;
            }
            // a mouse finds it by hovering; on touch it shows while pressed
            let active = if touch { resp.is_pointer_button_down_on() } else { resp.hovered() };
            hash_buttons.push((rect, active));
        }

        // -- Draw backgrounds ------------------------------------------------------
        self.renderer.draw_completion_popup(
            ui,
            popup_rect,
            &row_rects,
            self.link_completions.selected,
            hover_pos,
        );

        // a rule above the first file outside the note's scope
        if let Some(idx) = results.iter().position(|r| r.outside).filter(|i| *i > 0) {
            let row = row_rects[idx];
            ui.painter().hline(
                row.x_range(),
                row.top(),
                egui::Stroke::new(crate::style::STROKE_HAIRLINE, theme.neutral()),
            );
        }

        // On touch the `#` is its own cell, a hairline between it and the
        // row, in text ink so it doesn't read as the last crumb.
        for (rect, active) in hash_buttons {
            if active {
                let wash = theme.wash_toward_neutral_fg(theme.neutral_bg(), crate::style::FG_HOVER);
                ui.painter()
                    .rect_filled(rect.shrink(2.0), crate::style::Radius::Sm.corner(), wash);
            }
            if touch {
                let inset = rect.height() * 0.25;
                ui.painter().vline(
                    rect.left(),
                    (rect.top() + inset)..=(rect.bottom() - inset),
                    egui::Stroke::new(crate::style::STROKE_HAIRLINE, theme.neutral()),
                );
            }
            let ink = if active || touch { text_color } else { hint_color };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                crate::style::phosphor::HASH,
                crate::style::phosphor_font_id(completion_font()),
                ink,
            );
        }

        // -- Render text -----------------------------------------------------------
        let clip_rect = ui.clip_rect();
        let mut text_areas: Vec<TextBufferArea> = Vec::new();

        // group headers: the note's name, quiet, a rule above all but the first
        for (idx, (rect, group)) in header_rects.iter().enumerate() {
            if idx > 0 {
                ui.painter().hline(
                    rect.x_range(),
                    rect.top(),
                    egui::Stroke::new(crate::style::STROKE_HAIRLINE, theme.neutral()),
                );
            }
            let label_h = TypeRole::Mono.line_height();
            let content_rect = completion_text_rect(*rect);
            let content_rect = Rect::from_min_max(
                Pos2::new(content_rect.left(), rect.bottom() - label_h),
                Pos2::new(content_rect.right(), rect.bottom()),
            );
            let shaped = GlyphonLabel::new(group, hint_color)
                .font_size(completion_path_font())
                .line_height(label_h)
                .max_width(content_rect.width())
                .text_overflow(TextOverflow::EndEllipsis)
                .build(ui.ctx());
            text_areas.extend(shaped.text_areas(content_rect, ui.ctx(), clip_rect));
        }

        for (idx, result) in results.iter().enumerate() {
            let content_rect = text_rect(row_rects[idx], result);
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
            let tail = shaped.hint_size().map_or(0.0, |s| s.x + Space::Xs.pts());
            text_areas.extend(shaped.text_areas(content_rect, ui.ctx(), clip_rect));

            let shaped = crumb_label(path, hint_color, theme.accent())
                .font_size(completion_path_font())
                .line_height(TypeRole::Mono.line_height())
                .build(ui.ctx());
            let path_h = TypeRole::Mono.line_height();
            let path_rect = Rect::from_min_size(
                Pos2::new(
                    content_rect.max.x - tail - shaped.size.x,
                    content_rect.center().y - path_h / 2.0,
                ),
                Vec2::new(shaped.size.x, path_h),
            );
            text_areas.push(shaped.text_area(path_rect, ui.ctx(), clip_rect));
            if let Some(reach) = result.reach {
                let center = Pos2::new(
                    path_rect.min.x - Space::Xs.pts() - OUTSIDE_DOT,
                    path_rect.center().y,
                );
                let color = if reach.is_some() { theme.fg().yellow } else { theme.fg().red };
                ui.painter().circle_filled(center, OUTSIDE_DOT, color);
            }
        }

        // Submit after the editor's main text callback so the popup composites on top.
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.max_rect(),
                crate::GlyphonRendererCallback::new(text_areas),
            ));

        // -- Apply clicked result --------------------------------------------------
        // `#` on a note's row: its headings, the picker kept open
        if let Some(idx) = list_headings {
            let r = &results[idx];
            let events = &mut self.event.internal_events;
            if mode == CompletionMode::Link {
                let text = format!("[{}]({}#)", r.name, r.insert);
                let caret = Grapheme(bracket_start.0 + text.graphemes(true).count() - 1);
                events.push(Event::Replace {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(bracket_start),
                        end: Location::Grapheme(replace_end),
                    },
                    text,
                    advance_cursor: true,
                });
                events.push(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(caret),
                        end: Location::Grapheme(caret),
                    },
                });
            } else {
                events.push(Event::Replace {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(qr.0),
                        end: Location::Grapheme(qr.1),
                    },
                    text: format!("{}#", r.insert),
                    advance_cursor: true,
                });
            }
            self.link_completions.selected = 0;
        }

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

    use super::{CompletionMode, LinkCompletions};
    use crate::file_cache::{FileCache, FilesExt as _, Reach, ResolvedLink};
    use crate::test_utils::files::{at, cache, tree};

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

    /// After a `#`, the rows pair a note the part before it names with a
    /// heading that has what follows: named by slug, placed by the headings
    /// above, grouped under the note, inserting what resolves back (the text
    /// in a wiki link, the slug in a destination or for a repeat). A part
    /// that resolves names that note alone; a partial title or a tie, every
    /// note that has it, told apart by folder where names match.
    #[test]
    fn a_hash_lists_headings() {
        let (mut cache, editing) = shared_project();
        let note = "# Launch\n\n## Goals\n\n### Open\n\n## Goals\n";
        let mut lc = LinkCompletions::default();
        for (path, md) in [
            ("/project/Floor plan.md", "# Kitchen"),
            ("/project/design/Plan.md", "# Goals"),
            ("/project/research/Plan.md", "# Goals\n## Gone"),
        ] {
            let id = at(&cache, path);
            let headings = super::document_headings(md);
            cache.notes.insert(id, headings);
        }
        let mut rows = |query: &str, mode| {
            lc.search(&cache, editing, query, mode, note)
                .into_iter()
                .map(|r| {
                    let group = r.group.unwrap_or_default();
                    (group, r.name, r.insert, r.crumbs.unwrap_or_default())
                })
                .collect::<Vec<_>>()
        };
        let row = |group: &str, slug: &str, insert: &str, above: &str| {
            (group.to_string(), slug.to_string(), insert.to_string(), above.to_string())
        };
        assert_eq!(
            rows("#", CompletionMode::WikiLink),
            [
                row("editing", "launch", "#Launch", ""),
                row("editing", "goals", "#Goals", "Launch"),
                row("editing", "open", "#Open", "Launch / Goals"),
                row("editing", "goals-1", "#goals-1", "Launch"),
            ]
        );
        assert_eq!(
            rows("editing.md#op", CompletionMode::LinkDest),
            [row("editing", "open", "editing.md#open", "Launch / Goals")]
        );
        assert_eq!(
            rows("floo#", CompletionMode::WikiLink),
            [row("Floor plan", "kitchen", "Floor plan#Kitchen", "")]
        );
        assert_eq!(
            rows("Plan#go", CompletionMode::WikiLink),
            [
                row("design/Plan", "goals", "design/Plan#Goals", ""),
                row("research/Plan", "goals", "research/Plan#Goals", ""),
                row("research/Plan", "gone", "research/Plan#Gone", "Goals"),
            ],
            "a tie lists both notes' headings"
        );
        assert!(rows("Roadmap#", CompletionMode::WikiLink).is_empty(), "not read yet");
        assert!(rows("Nowhere#x", CompletionMode::WikiLink).is_empty());
    }

    /// Alice's tree, with `project/` shared; the note being edited is
    /// `/project/editing.md`.
    fn shared_project() -> (FileCache, Uuid) {
        let files = tree(
            "alice",
            &[
                "/Plan.md",
                "/Planning.md",
                "/project/ @bob",
                "/project/editing.md",
                "/project/Planning.md",
                "/project/Roadmap.md",
                "/project/Floor plan.md",
                "/project/design/Plan.md",
                "/project/design/Planner.md",
                "/project/design/Roadmap.md",
                "/project/research/Plan.md",
                "/project/planets/notes.md",
            ],
        );
        let editing = at(&files, "/project/editing.md");
        (cache(files), editing)
    }

    fn rows(cache: &FileCache, editing: Uuid, query: &str, mode: CompletionMode) -> Vec<String> {
        LinkCompletions::default()
            .search(cache, editing, query, mode, "")
            .iter()
            .map(|r| format!("{}{}", if r.outside { "outside:" } else { "" }, cache.path(r.id)))
            .collect()
    }

    /// The folders a note and a file share say nothing about the file, so
    /// their letters match nothing.
    #[test]
    fn shared_folders_take_no_part_in_a_match() {
        let files = tree(
            "alice",
            &[
                "/lockbook/maintainers/editing.md",
                "/lockbook/maintainers/design.md",
                "/lockbook/marketing/launch.md",
            ],
        );
        let editing = at(&files, "/lockbook/maintainers/editing.md");
        let cache = cache(files);
        assert!(rows(&cache, editing, "lk", CompletionMode::Link).is_empty());
        assert_eq!(
            rows(&cache, editing, "mark", CompletionMode::Link),
            ["/lockbook/marketing/launch.md"]
        );
    }

    /// A name on top by its first letters stays on top as the rest of it is
    /// typed.
    #[test]
    fn top_holds_while_its_name_is_typed() {
        let (cache, editing) = shared_project();
        for target in cache.all_files().filter(|f| f.is_document()) {
            let name = target.name.to_lowercase();
            let mut held = false;
            for end in 1..=name.len() {
                let query = &name[..end];
                let results = LinkCompletions::default().search(
                    &cache,
                    editing,
                    query,
                    CompletionMode::Link,
                    "",
                );
                let on_top = results.first().is_some_and(|r| r.id == target.id);
                assert!(
                    on_top || !held,
                    "{} fell from the top at {query:?}",
                    cache.path(target.id)
                );
                held = on_top;
            }
        }
    }

    /// Every completion offered resolves back to the file it was for, in the
    /// scope as a relative path or a wiki title, out of it as the external
    /// URL or a title unique in the reader's tree, and never as an absolute
    /// path. Out of the scope it reaches as [`Reach::Reader`], and says so.
    #[test]
    fn inserts_resolve_back_and_wiki_titles_are_shortest() {
        let (cache, editing) = shared_project();
        let mut lc = LinkCompletions::new(Some("http://localhost:8100".into()));
        for mode in [CompletionMode::WikiLink, CompletionMode::Link] {
            for query in ["plan", "road", "p", ""] {
                for r in lc.search(&cache, editing, query, mode, "") {
                    assert_eq!(r.reach, r.outside.then_some(Some(Reach::Reader)), "{:?}", r.insert);
                    let resolved = if mode == CompletionMode::WikiLink {
                        cache.resolve_wikilink(&r.insert, editing)
                    } else {
                        assert!(!r.insert.starts_with('/'), "absolute insert {:?}", r.insert);
                        assert_eq!(r.insert.starts_with("http://localhost:8100/open/"), r.outside);
                        match cache.resolve_link(&r.insert, editing) {
                            Some(ResolvedLink::File(id)) => Some(id),
                            _ => None,
                        }
                    };
                    assert_eq!(resolved, Some(r.id), "{query:?} inserted {:?}", r.insert);
                }
            }
        }

        let mut insert = |query: &str, path: &str| {
            let id = at(&cache, path);
            let results = lc.search(&cache, editing, query, CompletionMode::WikiLink, "");
            results.into_iter().find(|r| r.id == id).unwrap().insert
        };
        assert_eq!(insert("planner", "/project/design/Planner.md"), "Planner");
        assert_eq!(insert("plan", "/project/design/Plan.md"), "design/Plan");
        assert_eq!(insert("road", "/project/design/Roadmap.md"), "design/Roadmap");
        // every trailing path of the one at the top also ends the other's;
        // its whole path, from the scope's top, is its own
        assert_eq!(insert("road", "/project/Roadmap.md"), "/Roadmap");
    }

    /// A link to a file outside the note's scope inserts the external URL,
    /// a wiki title enough of its folders to be unique in the reader's tree;
    /// both resolve back to the file, marked as differing by reader. A file
    /// in another tree has no wiki title that reaches it: red.
    #[test]
    fn out_of_scope_rows_say_how_they_reach() {
        let (cache, [note, _, own_budget, ..]) = crate::file_cache::tests::alice();
        let mut lc = LinkCompletions::new(Some("http://localhost:8100".into()));
        let results = lc.search(&cache, note, "budget", CompletionMode::Link, "");
        let own = results
            .iter()
            .find(|r| r.id == own_budget)
            .expect("offered");
        assert_eq!(own.insert, format!("http://localhost:8100/open/{own_budget}"));
        assert_eq!(own.reach, Some(Some(Reach::Reader)));
        assert_eq!(cache.locate_link(&own.insert, note), Some((own_budget, Reach::Reader)));

        let wiki = lc.search(&cache, note, "budget", CompletionMode::WikiLink, "");
        let own = wiki.iter().find(|r| r.id == own_budget).expect("offered");
        assert_eq!(own.insert, "personal/Budget");
        assert_eq!(own.reach, Some(Some(Reach::Reader)));
        assert_eq!(cache.locate_wikilink(&own.insert, note), Some((own_budget, Reach::Reader)));

        // bob's memo, shared with alice and still pending: its own tree
        let root = cache.root().clone();
        let rows: Vec<File> = cache.all_files().cloned().collect();
        let memo_file = File {
            id: Uuid::from_u128(77),
            parent: Uuid::from_u128(78),
            owner: "bob".into(),
            ..tree("bob", &["/Memo.md"]).pop().unwrap()
        };
        let cache = FileCache::from_owned_and_shared(root, rows, [memo_file]);
        let mut lc = LinkCompletions::default();
        let wiki = lc.search(&cache, note, "memo", CompletionMode::WikiLink, "");
        assert_eq!(
            wiki.iter()
                .map(|r| (r.insert.as_str(), r.reach))
                .collect::<Vec<_>>(),
            [("Memo", Some(None))]
        );
        let link = lc.search(&cache, note, "memo", CompletionMode::Link, "");
        assert_eq!(link[0].reach, Some(Some(Reach::Reader)));

        // every title for /notes/Plan.md also names /project/notes/Plan.md,
        // which the note reaches first: the pick would open another file
        let files = tree(
            "alice",
            &["/notes/Plan.md", "/project/ @bob", "/project/x.md", "/project/notes/Plan.md"],
        );
        let (x, outside) = (at(&files, "/project/x.md"), at(&files, "/notes/Plan.md"));
        let files = crate::test_utils::files::cache(files);
        let wiki =
            LinkCompletions::default().search(&files, x, "plan", CompletionMode::WikiLink, "");
        let row = wiki.iter().find(|r| r.id == outside).expect("offered");
        assert_eq!(row.reach, Some(None), "{} opens another file", row.insert);
    }

    /// Filename search matches the full path; completions should too
    /// (`Buildinga` → `building/Android`, #4962).
    #[test]
    fn path_query_matches_folder_and_leaf() {
        let files = tree("alice", &["/editing.md", "/building/Android.md"]);
        let (editing, android) = (at(&files, "/editing.md"), at(&files, "/building/Android.md"));
        let cache = cache(files);
        // case never counts (#5204)
        for query in ["buildinga", "BuildingA"] {
            let results = LinkCompletions::default().search(
                &cache,
                editing,
                query,
                CompletionMode::WikiLink,
                "",
            );
            assert!(
                results.iter().any(|r| r.id == android),
                "{query} should suggest building/Android, got {:?}",
                results.iter().map(|r| &r.name).collect::<Vec<_>>(),
            );
        }
    }
}
