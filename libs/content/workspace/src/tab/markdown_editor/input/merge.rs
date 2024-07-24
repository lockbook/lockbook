use similar::{algorithms::DiffHook, DiffableStr as _, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::offset_types::DocCharOffset;

use super::canonical::{Location, Modification, Region};

/// Merge merges changes made outside the editor with changes made inside the editor by evaluating the diff between
/// the base and new content and fuzzy-patching the diff onto the editor's content.
// implementation note: this works because similar uses the same grapheme definition as we do, so reported indexes can
// be interpreted as doc char offsets
pub fn merge(base: &str, new: &str) -> Vec<Modification> {
    let mut hook = Hook::new(new);
    similar::algorithms::diff(
        similar::Algorithm::Myers,
        &mut hook,
        &base.as_diffable_str().tokenize_graphemes(),
        0..base.len(),
        &new.as_diffable_str().tokenize_graphemes(),
        0..new.len(),
    )
    .expect("unexpected error (DiffHook does not emit errors)");
    hook.events()
}

struct Hook<'a> {
    new: &'a str,
    events: Vec<Modification>,
}

impl<'a> Hook<'a> {
    fn new(new: &'a str) -> Self {
        Self { new, events: Vec::new() }
    }

    fn events(self) -> Vec<Modification> {
        self.events
    }
}

impl DiffHook for Hook<'_> {
    type Error = ();

    fn delete(
        &mut self, old_index: usize, old_len: usize, _new_index: usize,
    ) -> Result<(), Self::Error> {
        let start = Location::DocCharOffset(old_index.into());
        let end = Location::DocCharOffset((old_index + old_len).into());
        let region = Region::BetweenLocations { start, end };
        let text = String::new();
        let event = Modification::Replace { region, text };
        self.events.push(event);
        Ok(())
    }

    fn insert(
        &mut self, old_index: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let location = Location::DocCharOffset(old_index.into());
        let region = Region::Location(location);
        let text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)))
            .to_string();
        let event = Modification::Replace { region, text };
        self.events.push(event);
        Ok(())
    }

    fn replace(
        &mut self, old_index: usize, old_len: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let start = Location::DocCharOffset(old_index.into());
        let end = Location::DocCharOffset((old_index + old_len).into());
        let region = Region::BetweenLocations { start, end };
        let text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)))
            .to_string();
        let event = Modification::Replace { region, text };
        self.events.push(event);
        Ok(())
    }
}

trait GraphemeIndex {
    type Output: ?Sized;

    fn grapheme_index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output;
}

impl GraphemeIndex for str {
    type Output = str;

    fn grapheme_index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        let start = self.grapheme_indices(true).nth(index.0 .0).unwrap().0;
        let end = self.grapheme_indices(true).nth(index.1 .0).unwrap().0;
        &self[start..end]
    }
}
