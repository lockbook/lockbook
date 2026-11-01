use egui::{Context, Id, Key, Modifiers, Pos2, Rect, Ui, Vec2};
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use std::sync::{Arc, RwLock};

use lb_rs::model::file::File;

use crate::file_cache::{
    FileCache, FilesExt as _, link_id, relative_path, split_fragment, strip_ext, web_link,
    wiki_fragment, wiki_title,
};
use crate::style::tree_metrics::{INDENT_BASE, INDENT_STEP};
use crate::style::{FileRow, Space, ThemeExt as _, TypeRole, file_row_icon, parent_crumbs};
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::bounds::{Paragraphs, RangesExt as _};
use crate::tab::markdown_editor::fragment::{Heading, document_headings, wiki_token};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::completion_popup_rect;
use crate::widgets::GlyphonLabel;
use crate::widgets::glyphon_label::TextOverflow;

const MAX_RESULTS: usize = 7;
/// The popup's width, narrower where the window is.
const POPUP_WIDTH: f32 = 360.0;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum CompletionMode {
    /// `[[title]]` — resolved by note title at navigation time.
    #[default]
    WikiLink,
    /// `[display text](path)` — regular markdown link, shows all files.
    Link,
    /// `![alt text](path)` — image link, shows only image files.
    ImageLink,
    /// `![[title]]` — wiki embed, shows only image files.
    WikiEmbed,
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
        // past a wikilink's `|` is its display text, which nothing completes
        if mode == CompletionMode::WikiLink && query.contains('|') {
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
            CompletionMode::WikiLink | CompletionMode::WikiEmbed => raw.ends_with("]]"),
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

        let up = ctx.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::ArrowUp) || i.consume_key(Modifiers::CTRL, Key::P)
        });
        if up && self.selected > 0 {
            self.selected -= 1;
        }
        let down = ctx.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::ArrowDown) || i.consume_key(Modifiers::CTRL, Key::N)
        });
        if down && self.selected + 1 < results.len() {
            self.selected += 1;
        }

        let r = &results[self.selected];
        let accept = ctx.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab)
        });
        if accept {
            let display =
                if matches!(mode, CompletionMode::LinkDest | CompletionMode::ImageLinkDest) {
                    existing_title(&buffer[(bracket_start, replace_end)], mode)
                } else {
                    r.name.clone()
                };
            let r = r.clone();
            self.apply_completion(events, bracket_start, replace_end, &display, &r.insert, mode);
            return;
        }

        // `#` and `|` take the highlighted row and go on: to its note's
        // headings, or to the link's display text
        let next =
            if !query.contains('#') && r.headings && r.group.is_none() { Some('#') } else { None };
        let display = (mode == CompletionMode::WikiLink).then_some('|');
        for key in [next, display].into_iter().flatten() {
            let typed = |e: &egui::Event| matches!(e, egui::Event::Text(t) if t.len() == 1 && t.starts_with(key));
            if ctx.input(|i| i.events.iter().any(typed)) {
                ctx.input_mut(|i| i.events.retain(|e| !typed(e)));
                let r = r.clone();
                self.continue_with(events, (bracket_start, replace_end), qr, &r, mode, key);
                return;
            }
        }
    }

    /// Takes `r` and goes on to the next part of the link after `key`: its
    /// note's headings after `#`, the display text after `|`.
    fn continue_with(
        &mut self, events: &mut Vec<Event>, (bracket_start, replace_end): (Grapheme, Grapheme),
        qr: (Grapheme, Grapheme), r: &FileResult, mode: CompletionMode, key: char,
    ) {
        if key == '#' && mode == CompletionMode::Link {
            // the query is the link's text: write the whole link, the caret
            // in its destination after the `#`
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
                text: format!("{}{key}", r.insert),
                advance_cursor: true,
            });
        }
        self.selected = 0;
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
            CompletionMode::WikiEmbed => format!("![[{}]]", path),
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
    if let Some((start, end)) = detect_wikilink(buffer) {
        if start.0 > 0 && grapheme_at(buffer, start.0 - 1) == "!" {
            return Some(((start - 1, end), CompletionMode::WikiEmbed));
        }
        return Some(((start, end), CompletionMode::WikiLink));
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

    // an unclosed wikilink ends at the cursor; the rest of the line isn't its
    let mut end = cursor_idx;
    let mut j = cursor_idx;
    while j < len {
        if grapheme_at(buffer, j) == "\n" || grapheme_at(buffer, j) == "[" {
            break;
        }
        if grapheme_at(buffer, j) == "]" && j + 1 < len && grapheme_at(buffer, j + 1) == "]" {
            end = j + 2;
            break;
        }
        if j - cursor_idx > 200 {
            break;
        }
        j += 1;
    }

    Some((Grapheme(bracket_start), Grapheme(end)))
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
    // link is replaced when the user picks a result; an unclosed `[` ends at
    // the cursor.
    let mut end = cursor_idx;
    let mut j = cursor_idx;
    while j < len && grapheme_at(buffer, j) != "\n" && grapheme_at(buffer, j) != "[" {
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
            end = j;
            break;
        }
        j += 1;
    }

    Some(((Grapheme(start), Grapheme(end)), is_image))
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
        CompletionMode::WikiEmbed => 3, // ![[
        CompletionMode::Link => 1,      // [
        CompletionMode::ImageLink => 2, // ![
        CompletionMode::LinkDest | CompletionMode::ImageLinkDest => unreachable!(),
    };
    let start = Grapheme(range.0.0 + prefix_len);

    // Convert byte lengths to grapheme counts before adding to a `Grapheme`;
    // multi-byte clusters (Devanagari, emoji) overshoot otherwise.
    let raw = &buffer[range];
    let end = match mode {
        CompletionMode::WikiLink | CompletionMode::WikiEmbed => {
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
    /// The file's path, for its folder crumbs.
    path: String,
    /// A note whose headings the row's `#` button lists.
    headings: bool,
    /// For a heading, the note it is in, which heads its group of rows.
    group: Option<String>,
    /// A heading's level, 1 for `#`; 0 for a file.
    level: u8,
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
        let image = matches!(
            mode,
            CompletionMode::ImageLink | CompletionMode::ImageLinkDest | CompletionMode::WikiEmbed
        );
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

        let image = matches!(
            mode,
            CompletionMode::ImageLink | CompletionMode::ImageLinkDest | CompletionMode::WikiEmbed
        );
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
                let insert = if matches!(mode, CompletionMode::WikiLink | CompletionMode::WikiEmbed)
                {
                    cache
                        .wikilink_title(f.id, file_id)
                        .or_else(|| cache.wikilink_title_in(f.id, cache.tree_root(file_id)))
                        .unwrap_or_else(|| wiki_title(&f.name).to_string())
                } else {
                    match cache.link_destination(f.id, file_id) {
                        Some(path) => path,
                        None => format!("{origin}/open/{}", f.id),
                    }
                };
                let (outside, path) = (candidate.outside, cache.path(f.id));
                let headings = !image && f.name.to_lowercase().ends_with(".md");
                Some(FileResult {
                    id: f.id,
                    name,
                    insert,
                    outside,
                    path,
                    headings,
                    group: None,
                    level: 0,
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
                false => {
                    cache.notes.want();
                    cache.notes.headings(*id).unwrap_or_default()
                }
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
                // its text names it; its level indents it; its note heads
                // the group
                FileResult {
                    id: *id,
                    name: h.text.clone(),
                    insert: format!("{insert}#{token}"),
                    outside: false,
                    path: String::new(),
                    headings: false,
                    group: Some(labels[rank].clone()),
                    level: h.level,
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

/// A faint dot in each step of a heading row's indent, as an editor marks
/// whitespace, so its depth reads without a level label.
fn paint_indent_dots(ui: &Ui, row: Rect, depth: usize, theme: &crate::style::Theme) {
    let ink = theme.neutral_fg_secondary().gamma_multiply(0.45);
    for step in 0..depth {
        let x = row.left() + INDENT_BASE + (step as f32 + 0.5) * INDENT_STEP;
        ui.painter()
            .circle_filled(Pos2::new(x, row.center().y), 1.0, ink);
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
        let scope = cache.scope_top(self.file_id);
        let scope = cache
            .get_by_id(scope)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        drop(cache);
        if results.is_empty() {
            return;
        }

        let Some([cursor_top, cursor_bot]) = self.cursor_line(bracket_start) else {
            return;
        };

        let theme = ui.ctx().get_lb_theme();
        let pad = Space::Xs.pts();
        // a heading's note heads its group, when there's more than one
        let groups = results.iter().filter_map(|r| r.group.as_deref());
        let grouped = groups
            .clone()
            .any(|g| Some(g) != results[0].group.as_deref());
        // a section title sits with the rows it heads: air above it, little
        // below
        let title_above = |i: usize| if i == 0 { Space::Xs.pts() } else { Space::Sm.pts() };
        let title_h =
            |i: usize| title_above(i) + TypeRole::Caption.line_height() + Space::Xxs.pts();
        // each group's headings indent from its shallowest
        let top_level = |group: Option<&str>| {
            results
                .iter()
                .filter(|r| r.group.as_deref() == group)
                .map(|r| r.level)
                .min()
                .unwrap_or(1)
        };
        let row_h = |r: &FileResult| {
            let subtitle = r.group.is_none() && !parent_crumbs(&r.path).is_empty();
            FileRow::height_for(subtitle)
        };
        // what heads row `i`: its heading's note, or the files outside the scope
        let header = |i: usize| -> Option<String> {
            let r = &results[i];
            let first = i == 0 || results[i - 1].group != r.group;
            if grouped && first {
                return r.group.clone();
            }
            let outside = r.outside && (i == 0 || !results[i - 1].outside);
            outside.then(|| format!("Outside {scope}"))
        };
        let mut height = 2.0 * pad;
        for (i, r) in results.iter().enumerate() {
            if header(i).is_some() {
                height += title_h(i);
            }
            height += row_h(r);
        }
        let size = Vec2::new(POPUP_WIDTH, height);
        let popup_rect =
            completion_popup_rect(cursor_top, cursor_bot, size, ui.ctx().content_rect());
        self.renderer
            .touch_targets
            .push((popup_rect, TouchTarget::Popup));
        self.renderer.draw_completion_popup(ui, popup_rect);

        let mut clicked = None;
        let mut y = popup_rect.top() + pad;
        let (left, right) = (popup_rect.left() + pad, popup_rect.right() - pad);
        for (i, r) in results.iter().enumerate() {
            if let Some(label) = header(i) {
                let at = Rect::from_min_max(
                    Pos2::new(left, y + title_above(i)),
                    Pos2::new(right, y + title_h(i)),
                );
                let label = TypeRole::caps(&label);
                let label =
                    GlyphonLabel::new(&label, theme.neutral_fg_secondary()).role(TypeRole::Caption);
                let label = label
                    .max_width(at.width() - 2.0 * INDENT_BASE)
                    .text_overflow(TextOverflow::EndEllipsis);
                let size = label.measure(ui);
                let slot = Rect::from_min_size(Pos2::new(at.left() + INDENT_BASE, at.top()), size);
                ui.put(slot, label);
                y += title_h(i);
            }
            let rect = Rect::from_min_max(Pos2::new(left, y), Pos2::new(right, y + row_h(r)));
            let id = Id::unique("link_item").with((r.id, i));
            let selected = i == self.link_completions.selected;
            let resp = if r.group.is_some() {
                let depth = r.level.saturating_sub(top_level(r.group.as_deref())) as usize;
                let resp = FileRow::new(&theme, &r.name)
                    .icon("")
                    .depth(depth)
                    .highlighted(selected)
                    .paint_at_hit(ui, rect, rect, id);
                paint_indent_dots(ui, rect, depth, &theme);
                resp
            } else {
                let file = r.path.rsplit('/').next().unwrap_or(&r.name);
                FileRow::new(&theme, &r.name)
                    .icon(file_row_icon(file, false))
                    .subtitle(parent_crumbs(&r.path))
                    .highlighted(selected)
                    .paint_at_hit(ui, rect, rect, id)
            };
            if resp.clicked() {
                clicked = Some(i);
            }
            y += row_h(r);
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
    /// heading that has what follows: named by its text, at its level,
    /// grouped under the note, inserting what resolves back (the text
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
            cache.notes.insert(id, crate::links::read_note(md));
        }
        let mut rows = |query: &str, mode| {
            lc.search(&cache, editing, query, mode, note)
                .into_iter()
                .map(|r| {
                    let group = r.group.unwrap_or_default();
                    (group, r.name, r.insert, r.level)
                })
                .collect::<Vec<_>>()
        };
        let row = |group: &str, text: &str, insert: &str, level: u8| {
            (group.to_string(), text.to_string(), insert.to_string(), level)
        };
        assert_eq!(
            rows("#", CompletionMode::WikiLink),
            [
                row("editing", "Launch", "#Launch", 1),
                row("editing", "Goals", "#Goals", 2),
                row("editing", "Open", "#Open", 3),
                row("editing", "Goals", "#goals-1", 2),
            ]
        );
        assert_eq!(
            rows("editing.md#op", CompletionMode::LinkDest),
            [row("editing", "Open", "editing.md#open", 3)]
        );
        assert_eq!(
            rows("floo#", CompletionMode::WikiLink),
            [row("Floor plan", "Kitchen", "Floor plan#Kitchen", 1)]
        );
        assert_eq!(
            rows("Plan#go", CompletionMode::WikiLink),
            [
                row("design/Plan", "Goals", "design/Plan#Goals", 1),
                row("research/Plan", "Goals", "research/Plan#Goals", 1),
                row("research/Plan", "Gone", "research/Plan#Gone", 2),
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

    /// `![[` offers images, not drawings or notes, and writes a wiki embed.
    #[test]
    fn a_wiki_embed_offers_images() {
        use lb_rs::model::text::buffer::Buffer;
        use lb_rs::model::text::offset_types::Grapheme;

        use super::{detect_any, query_range};

        let mut buffer = Buffer::from("see ![[ch");
        buffer.current.selection = (Grapheme(9), Grapheme(9));
        let (range, mode) = detect_any(&buffer).unwrap();
        assert!(mode == CompletionMode::WikiEmbed);
        assert_eq!(&buffer[range], "![[ch");
        assert_eq!(&buffer[query_range(&buffer, range, mode)], "ch");

        let files = tree(
            "alice",
            &["/notes/editing.md", "/notes/chart.png", "/notes/chart sketch.svg", "/chart.md"],
        );
        let editing = at(&files, "/notes/editing.md");
        let cache = cache(files);
        assert_eq!(rows(&cache, editing, "ch", mode), ["/notes/chart.png"]);
        let mut events = vec![];
        LinkCompletions::default().apply_completion(
            &mut events,
            range.0,
            range.1,
            "",
            "chart.png",
            mode,
        );
        let replaced = matches!(&events[..], [crate::tab::markdown_editor::input::Event::Replace { text, .. }] if text == "![[chart.png]]");
        assert!(replaced, "{events:?}");
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
    /// path.
    #[test]
    fn inserts_resolve_back_and_wiki_titles_are_shortest() {
        let (cache, editing) = shared_project();
        let mut lc = LinkCompletions::new(Some("http://localhost:8100".into()));
        for mode in [CompletionMode::WikiLink, CompletionMode::Link] {
            for query in ["plan", "road", "p", ""] {
                for r in lc.search(&cache, editing, query, mode, "") {
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
    /// both resolve back to the file.
    #[test]
    fn out_of_scope_rows_insert_what_reaches_them() {
        let (cache, [note, _, own_budget, ..]) = crate::file_cache::tests::alice();
        let mut lc = LinkCompletions::new(Some("http://localhost:8100".into()));
        let results = lc.search(&cache, note, "budget", CompletionMode::Link, "");
        let own = results
            .iter()
            .find(|r| r.id == own_budget)
            .expect("offered");
        assert_eq!(own.insert, format!("http://localhost:8100/open/{own_budget}"));
        assert_eq!(cache.locate_link(&own.insert, note), Some((own_budget, Reach::Reader)));

        let wiki = lc.search(&cache, note, "budget", CompletionMode::WikiLink, "");
        let own = wiki.iter().find(|r| r.id == own_budget).expect("offered");
        assert_eq!(own.insert, "personal/Budget");
        assert_eq!(cache.locate_wikilink(&own.insert, note), Some((own_budget, Reach::Reader)));
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
