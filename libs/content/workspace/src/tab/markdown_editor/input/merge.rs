use similar::{algorithms::DiffHook, DiffableStr as _, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::{buffer::SubBuffer, offset_types::DocCharOffset};

use super::canonical::{Location, Modification, Region};

// implementation note: this works because similar uses the same grapheme definition as we do, so reported indexes can
// be interpreted as doc char offsets
pub fn merge(base: &str, local: &str, remote: &str) -> Vec<Modification> {
    println!("\n----- merge -----");
    println!("base: {}", base);
    println!("local: {}", local);
    println!("remote: {}", remote);

    let in_editor_mutations = {
        let mut hook = Hook::new(local);
        similar::algorithms::diff(
            similar::Algorithm::Myers,
            &mut hook,
            &base.as_diffable_str().tokenize_graphemes(),
            0..base.len(),
            &local.as_diffable_str().tokenize_graphemes(),
            0..local.len(),
        )
        .expect("unexpected error (DiffHook does not emit errors)");
        hook.modifications()
    };
    let mut out_of_editor_mutations = {
        let mut hook = Hook::new(remote);
        similar::algorithms::diff(
            similar::Algorithm::Myers,
            &mut hook,
            &base.as_diffable_str().tokenize_graphemes(),
            0..base.len(),
            &remote.as_diffable_str().tokenize_graphemes(),
            0..remote.len(),
        )
        .expect("unexpected error (DiffHook does not emit errors)");
        hook.modifications()
    };

    println!("in_editor_mutations: {:?}", in_editor_mutations);
    println!("out_of_editor_mutations: {:?}", out_of_editor_mutations);

    // adjust outside changes based on inside changes
    for in_mutation in in_editor_mutations {
        for out_mutation in &mut out_of_editor_mutations {
            if let (
                Modification::Replace {
                    region:
                        Region::BetweenLocations {
                            start: Location::DocCharOffset(in_start),
                            end: Location::DocCharOffset(in_end),
                        },
                    text: text_replacement,
                },
                Modification::Replace {
                    region:
                        Region::BetweenLocations {
                            start: Location::DocCharOffset(out_start),
                            end: Location::DocCharOffset(out_end),
                        },
                    text: _,
                },
            ) = (&in_mutation, out_mutation)
            {
                let text_replacement_len = text_replacement.grapheme_indices(true).count();
                let mut tmp = (out_start.clone(), out_end.clone());
                SubBuffer::adjust_subsequent_range(
                    (*in_start, *in_end),
                    text_replacement_len.into(),
                    true,
                    Some(&mut tmp),
                );
                (*out_start, *out_end) = tmp;
            } else {
                unreachable!()
            }
        }
    }

    println!("adjusted out_of_editor_mutations: {:?}", out_of_editor_mutations);

    // return out-of-editor changes "rebased" on in-editor changes
    out_of_editor_mutations
}

struct Hook<'a> {
    new: &'a str,
    modifications: Vec<Modification>,
}

impl<'a> Hook<'a> {
    fn new(new: &'a str) -> Self {
        Self { new, modifications: Vec::new() }
    }

    fn modifications(self) -> Vec<Modification> {
        self.modifications
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
        let modification = Modification::Replace { region, text };

        // println!("modification: {:?}", modification);

        self.modifications.push(modification);
        Ok(())
    }

    fn insert(
        &mut self, old_index: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let location = Location::DocCharOffset(old_index.into());
        let region = Region::BetweenLocations { start: location, end: location };
        let text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)))
            .to_string();
        let modification = Modification::Replace { region, text };

        // println!("modification: {:?}", modification);

        self.modifications.push(modification);
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
        let modification = Modification::Replace { region, text };

        // println!("modification: {:?}", modification);

        self.modifications.push(modification);
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
        let mut graphemes: Vec<_> = self.grapheme_indices(true).collect();
        graphemes.push((self.len(), ""));
        &self[graphemes[index.0 .0].0..graphemes[index.1 .0].0]
    }
}
