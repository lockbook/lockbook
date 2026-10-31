//! Language names offered while typing a code fence's info, like `ru` after
//! ```` ``` ````, from the languages we color.

use egui::{Context, Id, Key, Modifiers, Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::style::{FileRow, Space, ThemeExt as _};
use crate::syntax::{self, Lang};
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::completion_popup_rect;
use crate::tab::markdown_editor::{MdEdit, TouchTarget};

const MAX_RESULTS: usize = 5;
const POPUP_WIDTH: f32 = 180.0;

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
    /// Recomputes whether the popup shows; runs before `handle_input`.
    pub fn update_active_state(&mut self, buffer: &Buffer) {
        self.active = detect_query(buffer).is_some_and(|(_, query)| {
            self.suppressed.as_deref() != Some(&query) && !results_for(&query).is_empty()
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
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.suppressed = Some(query);
            return;
        }
        if !editor_focused {
            return;
        }
        let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
        if up && self.selected > 0 {
            self.selected -= 1;
        }
        let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
        if down && self.selected + 1 < results.len() {
            self.selected += 1;
        }
        let accept = ctx.input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab)
        });
        if accept {
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
        let Some([top, bottom]) = self.cursor_line(range.start()) else { return };

        let t = ui.ctx().get_lb_theme();
        let pad = Space::Xs.pts();
        let row_h = FileRow::height_for(false);
        let size = Vec2::new(POPUP_WIDTH, 2.0 * pad + row_h * results.len() as f32);
        let popup_rect = completion_popup_rect(top, bottom, size, ui.ctx().content_rect());
        self.renderer
            .touch_targets
            .push((popup_rect, TouchTarget::Popup));
        self.renderer.draw_completion_popup(ui, popup_rect);

        let mut clicked = None;
        for (i, name) in results.iter().enumerate() {
            let row_top = popup_rect.top() + pad + row_h * i as f32;
            let rect = Rect::from_min_max(
                Pos2::new(popup_rect.left() + pad, row_top),
                Pos2::new(popup_rect.right() - pad, row_top + row_h),
            );
            let resp = FileRow::new(&t, *name)
                .icon("")
                .highlighted(i == self.language_completions.selected)
                .paint_at_hit(ui, rect, rect, Id::unique("language_item").with(i));
            if resp.clicked() {
                clicked = Some(*name);
            }
        }
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
