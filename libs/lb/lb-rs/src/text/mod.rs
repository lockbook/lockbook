pub mod buffer;
pub mod offset_types;
pub mod operation_types;
pub mod unicode_segs;

use operation_types::Replace;

use similar::{algorithms::DiffHook, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

pub fn diff(from: &str, to: &str) -> Vec<Replace> {
    let mut hook = Hook::new(to);

    let mut from_words: Vec<_> = from
        .split_word_bound_indices()
        .map(|(idx, _)| idx)
        .collect();
    from_words.push(from.len());
    let mut to_words: Vec<_> = to.split_word_bound_indices().map(|(idx, _)| idx).collect();
    to_words.push(to.len());

    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .diff_unicode_words(from.as_diffable_str(), to.as_diffable_str());

    for diff_op in diff.ops().iter().cloned() {
        println!("processing diff op: {:?}", diff_op);
        match diff_op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete { old_index, old_len, new_index } => {
                let old_len = from_words[old_index + old_len] - from_words[old_index];
                let old_index = from_words[old_index];
                let new_index = to_words[new_index];
                hook.delete(old_index, old_len, new_index).unwrap();
            }
            similar::DiffOp::Insert { old_index, new_index, new_len } => {
                let old_index = from_words[old_index];
                let new_len = to_words[new_index + new_len] - to_words[new_index];
                let new_index = to_words[new_index];
                hook.insert(old_index, new_index, new_len).unwrap()
            }
            similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                let old_len = from_words[old_index + old_len] - from_words[old_index];
                let old_index = from_words[old_index];
                let new_len = to_words[new_index + new_len] - to_words[new_index];
                let new_index = to_words[new_index];
                hook.replace(old_index, old_len, new_index, new_len)
                    .unwrap()
            }
        }
    }
    hook.ops()
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
