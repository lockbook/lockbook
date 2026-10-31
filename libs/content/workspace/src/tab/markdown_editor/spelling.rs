//! Spelling marks: which words of the prose a wrap unit lays out are
//! misspelled, and the dotted rule under each.

use egui::Ui;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::spelling::checkable;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    DecoLine, Fragment, FragmentContent,
};
use crate::theme::palette_v2::ThemeExt as _;

impl MdRender {
    /// Whether spelling is marked: on, editable, and not the drag card.
    pub fn spelling_marked(&self) -> bool {
        self.spell_check && !self.readonly && !self.painting_drag_float
    }

    pub fn note_typing(&mut self, text_updated: bool, selection_moved: bool) {
        self.typing = text_updated || (self.typing && !selection_moved);
    }

    /// The misspelled words in `prose`. A word being typed or composed is
    /// checked once it is left.
    pub fn misspelled_words(&self, prose: &[(Grapheme, Grapheme)]) -> Vec<(Grapheme, Grapheme)> {
        if prose.is_empty() || !self.spelling_marked() {
            return Vec::new();
        }
        let Some(spelling) = crate::spelling::get(&self.ctx) else { return Vec::new() };
        let (start, end) = self.buffer.current.selection;
        let caret = (self.typing && start == end).then_some(start);
        // a keyboard also marks the word a placed caret lands in; that is
        // not input in progress
        let composing = self
            .typing
            .then(|| {
                self.ime_marked
                    .as_ref()
                    .map(|m| m.range)
                    .or(self.platform_marked)
            })
            .flatten();
        let words = &self.bounds.words;
        let mut misspelled = Vec::new();
        for &range in prose {
            let first = words.partition_point(|w| w.end() <= range.start());
            for &word in words[first..]
                .iter()
                .take_while(|w| w.start() < range.end())
            {
                let inside = range.start() <= word.start() && word.end() <= range.end();
                let typing = caret.is_some_and(|c| word.start() <= c && c <= word.end());
                let composed =
                    composing.is_some_and(|c| c.start() < word.end() && word.start() < c.end());
                if !inside || typing || composed {
                    continue;
                }
                let text = &self.buffer[word];
                if checkable(text, self.before_tag_run(word.start()))
                    && spelling.check(text) == Some(false)
                {
                    misspelled.push(word);
                }
            }
        }
        misspelled
    }

    /// The character before the run of tag characters that ends at `start`,
    /// so `#to-read` and `#project/plan` count as tags in every part.
    fn before_tag_run(&self, start: Grapheme) -> Option<char> {
        let mut at = start;
        while at.0 > 0 {
            let c = self.buffer[(at - 1, at)].chars().last()?;
            if !(c.is_alphanumeric() || matches!(c, '-' | '_' | '/')) {
                return Some(c);
            }
            at -= 1;
        }
        None
    }

    /// The word that holds `offset`.
    pub fn word_at(&self, offset: Grapheme) -> Option<(Grapheme, Grapheme)> {
        self.bounds
            .words
            .iter()
            .copied()
            .find(|w| w.start() <= offset && offset <= w.end())
    }

    /// The word marked misspelled last frame that holds `offset`.
    pub fn misspelled_at(&self, offset: Grapheme) -> Option<(Grapheme, Grapheme)> {
        self.misspelled
            .iter()
            .copied()
            .find(|w| w.start() <= offset && offset <= w.end())
    }

    /// The word marked misspelled last frame that holds the selection.
    pub fn selection_misspelling(&self) -> Option<(Grapheme, Grapheme)> {
        let (start, end) = self.buffer.current.selection;
        self.misspelled
            .iter()
            .copied()
            .find(|w| w.start() <= start && end <= w.end())
    }

    /// Dotted rules under the parts of `misspelled` in `fragment`, which is
    /// in screen space.
    pub fn mark_misspellings(
        &mut self, ui: &Ui, fragment: &Fragment, misspelled: &[(Grapheme, Grapheme)],
        row_height: f32,
    ) {
        if fragment.atomic || !matches!(fragment.content, FragmentContent::Glyphs { .. }) {
            return;
        }
        let clip = ui.clip_rect().x_range();
        let y = fragment.rect.min.y + fragment.content_inset.top + row_height + 1.0;
        let color = self.ctx.get_lb_theme().danger();
        let source = fragment.source_range;
        for &word in misspelled {
            let (start, end) = (word.start().max(source.start()), word.end().min(source.end()));
            if start >= end {
                continue;
            }
            let left = self.fragment_x(fragment, start).max(clip.min);
            let right = self.fragment_x(fragment, end).min(clip.max);
            if left < right {
                self.deco_lines
                    .push(DecoLine { x: left..=right, y, color, dotted: true });
            }
        }
    }
}
