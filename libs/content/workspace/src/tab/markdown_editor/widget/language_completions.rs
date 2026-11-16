//! Language names offered while typing a code fence's info, like `ru` after
//! ```` ``` ````, from the languages we color.

use egui::{Context, Ui};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::syntax::{self, Lang};
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::{
    CompletionStep, completion_accepted, completion_dismissed, completion_step,
};

const MAX_RESULTS: usize = 5;

#[derive(Default)]
pub struct LanguageCompletions {
    /// A fence's info is being typed and has results.
    pub active: bool,
    /// Keyboard-highlighted result index.
    pub selected: usize,
    /// The query Escape dismissed; the popup stays away until it changes.
    suppressed: Option<String>,
}

/// The info word being typed at the caret, when the caret ends it on an
/// opening fence: its range and its text.
fn detect_query(buffer: &Buffer) -> Option<((Grapheme, Grapheme), String)> {
    let caret = buffer.current.selection;
    if !caret.is_empty() {
        return None;
    }
    let caret = caret.start();
    // the buffer takes a selection past the end of its text
    if caret > buffer.current.segs.last_cursor_position() {
        return None;
    }
    let text = &buffer.current.text;
    let at = buffer.current.segs.offset_to_byte(caret).0;
    let before = &text[text[..at].rfind('\n').map_or(0, |newline| newline + 1)..at];
    let after = &text[at..];
    if after.starts_with(|c: char| !c.is_whitespace()) {
        return None;
    }
    let fence = before.trim_start_matches([' ', '\t', '>']);
    let marker = fence.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let query = fence.trim_start_matches(marker);
    if fence.len() - query.len() < 3 || query.is_empty() || query.contains(char::is_whitespace) {
        return None;
    }
    if query.contains(['`', '~']) || Lang::from_info(query).is_some() {
        return None;
    }
    let len = query.graphemes(true).count();
    Some(((caret - len, caret), query.to_string()))
}

fn results_for(query: &str) -> Vec<&'static str> {
    syntax::complete(query)
        .into_iter()
        .take(MAX_RESULTS)
        .collect()
}

impl LanguageCompletions {
    /// Recomputes whether the popup shows, never unless the text has fences
    /// to type; runs before `handle_input`.
    pub fn update_active_state(&mut self, buffer: &Buffer, fenced: bool) {
        self.active = fenced
            && detect_query(buffer).is_some_and(|(_, query)| {
                self.suppressed.as_deref() != Some(query.as_str())
                    && !results_for(&query).is_empty()
            });
    }

    /// Takes the popup's keys, as the emoji popup does: Escape dismisses,
    /// arrows move, Enter or Tab accepts.
    pub fn handle_input(
        &mut self, ctx: &Context, buffer: &Buffer, editor_focused: bool, events: &mut Vec<Event>,
    ) {
        if !self.active {
            return;
        }
        let Some((range, query)) = detect_query(buffer) else { return };
        let results = results_for(&query);
        self.selected = self.selected.min(results.len().saturating_sub(1));
        if completion_dismissed(ctx) {
            self.suppressed = Some(query);
            return;
        }
        if !editor_focused {
            return;
        }
        match completion_step(ctx) {
            Some(CompletionStep::Up) => self.selected = self.selected.saturating_sub(1),
            Some(CompletionStep::Down) => {
                self.selected = (self.selected + 1).min(results.len().saturating_sub(1))
            }
            None => {}
        }
        if completion_accepted(ctx) {
            self.apply_completion(events, range, results[self.selected]);
        }
    }

    fn apply_completion(
        &mut self, events: &mut Vec<Event>, range: (Grapheme, Grapheme), name: &str,
    ) {
        events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(range.start()),
                end: Location::Grapheme(range.end()),
            },
            text: name.into(),
            advance_cursor: true,
        });
        self.selected = 0;
        self.suppressed = None;
    }
}

impl MdEdit {
    pub fn show_language_completions(&mut self, ui: &mut Ui) {
        if self.renderer.readonly || !self.language_completions.active {
            return;
        }
        let Some((range, query)) = detect_query(&self.renderer.buffer) else { return };
        let results = results_for(&query);
        let labels: Vec<String> = results.iter().map(|name| name.to_string()).collect();
        let highlighted = Some(self.language_completions.selected);
        let clicked = self
            .show_completion_rows(ui, range.start(), &labels, highlighted, "language_item")
            .map(|i| results[i]);
        if let Some(name) = clicked {
            self.language_completions.apply_completion(
                &mut self.event.internal_events,
                range,
                name,
            );
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod test {
    use super::detect_query;
    use lb_rs::model::text::buffer::Buffer;

    fn query(s: &str) -> Option<String> {
        let mut buffer = Buffer::from(s);
        let end = buffer.current.segs.last_cursor_position();
        buffer.current.selection = (end, end);
        detect_query(&buffer).map(|(_, query)| query)
    }

    #[test]
    fn fence_info_is_a_query() {
        assert_eq!(query("```ru").as_deref(), Some("ru"));
        assert_eq!(query("> ~~~ty").as_deref(), Some("ty"));
        assert_eq!(query("text\n```py"), None, "py is already a language");
        assert_eq!(query("``ru"), None);
        assert_eq!(query("```"), None);
        assert_eq!(query("```rust ign"), None);
    }
}
