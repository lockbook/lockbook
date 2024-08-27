use similar::{algorithms::DiffHook, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::buffer::Replace;

// implementation note: this works because similar uses the same grapheme definition as we do, so reported indexes can
// be interpreted as doc char offsets
pub fn patch_ops(old: &str, new: &str) -> Vec<Replace> {
    let out_of_editor_mutations = {
        let mut hook = Hook::new(new);

        let old_words: Vec<_> = old.unicode_word_indices().collect();
        let new_words: Vec<_> = new.unicode_word_indices().collect();
        let diff = similar::TextDiff::configure()
            .algorithm(similar::Algorithm::Myers)
            .diff_unicode_words(old.as_diffable_str(), new.as_diffable_str());

        for diff_op in diff.ops().iter().cloned() {
            match diff_op {
                similar::DiffOp::Equal { .. } => {}
                similar::DiffOp::Delete { old_index, old_len, new_index } => {
                    let old_index = old_words[old_index].0;
                    let old_len = old_words[old_index + old_len].0 - old_index;
                    let new_index = new_words[new_index].0;
                    hook.delete(old_index, old_len, new_index).unwrap();
                }
                similar::DiffOp::Insert { old_index, new_index, new_len } => {
                    let old_index = old_words[old_index].0;
                    let new_index = new_words[new_index].0;
                    let new_len = new_words[new_index + new_len].0 - new_index;
                    hook.insert(old_index, new_index, new_len).unwrap()
                }
                similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                    let old_index = old_words[old_index].0;
                    let old_len = old_words[old_index + old_len].0 - old_index;
                    let new_index = new_words[new_index].0;
                    let new_len = new_words[new_index + new_len].0 - new_index;
                    hook.replace(old_index, old_len, new_index, new_len)
                        .unwrap()
                }
            }
        }
        hook.ops()
    };

    out_of_editor_mutations
}

struct Hook<'a> {
    new: &'a str,
    segs: Vec<usize>,
    ops: Vec<Replace>,
}

impl<'a> Hook<'a> {
    fn new(new: &'a str) -> Self {
        let mut segs: Vec<_> = new.grapheme_indices(true).map(|(idx, _)| idx).collect();
        segs.push(new.len());
        Self { new, ops: Vec::new(), segs }
    }

    fn ops(self) -> Vec<Replace> {
        self.ops
    }

    fn grapheme_index(&self, index: (usize, usize)) -> &str {
        let (start, end) = index;
        &self.new[self.segs[start]..self.segs[end]]
    }
}

impl DiffHook for Hook<'_> {
    type Error = ();

    fn delete(
        &mut self, old_index: usize, old_len: usize, _new_index: usize,
    ) -> Result<(), Self::Error> {
        if let Some(op) = self.ops.last_mut() {
            let Replace { range, .. } = op;
            if range.1 == old_index {
                range.1 = (old_index + old_len).into();
                return Ok(());
            }
        }

        let op = Replace {
            range: (old_index.into(), (old_index + old_len).into()),
            text: String::new(),
        };

        self.ops.push(op);
        Ok(())
    }

    fn insert(
        &mut self, old_index: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let new_text = self
            .grapheme_index((new_index, new_index + new_len))
            .to_string();

        if let Some(op) = self.ops.last_mut() {
            let Replace { range, text } = op;
            if range.1 == old_index {
                text.push_str(&new_text);
                return Ok(());
            }
        }

        let op = Replace { range: (old_index.into(), old_index.into()), text: new_text };

        self.ops.push(op);
        Ok(())
    }

    fn replace(
        &mut self, old_index: usize, old_len: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let new_text = self
            .grapheme_index((new_index, new_index + new_len))
            .to_string();

        if let Some(op) = self.ops.last_mut() {
            let Replace { range, text } = op;
            if range.1 == old_index {
                range.1 = (old_index + old_len).into();
                text.push_str(&new_text);
                return Ok(());
            }
        }

        let op =
            Replace { range: (old_index.into(), (old_index + old_len).into()), text: new_text };

        self.ops.push(op);
        Ok(())
    }
}
