use similar::{algorithms::DiffHook, DiffableStr as _, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::buffer::{Operation, Replace};
use crate::tab::markdown_editor::offset_types::DocCharOffset;

// implementation note: this works because similar uses the same grapheme definition as we do, so reported indexes can
// be interpreted as doc char offsets
pub fn patch_ops(old: &str, new: &str) -> Vec<Operation> {
    println!("\n----- merge -----");
    println!("old: {}", old);
    println!("new: {}", new);

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

    println!("out_of_editor_mutations (words): {:?}", out_of_editor_mutations);

    out_of_editor_mutations
}

// todo: cache unicode segmentation for performance
struct Hook<'a> {
    new: &'a str,
    ops: Vec<Operation>,
}

impl<'a> Hook<'a> {
    fn new(new: &'a str) -> Self {
        Self { new, ops: Vec::new() }
    }

    fn ops(self) -> Vec<Operation> {
        self.ops
    }
}

impl DiffHook for Hook<'_> {
    type Error = ();

    fn delete(
        &mut self, old_index: usize, old_len: usize, _new_index: usize,
    ) -> Result<(), Self::Error> {
        if let Some(op) = self.ops.last_mut() {
            if let Operation::Replace(Replace { range, .. }) = op {
                if range.1 == DocCharOffset(old_index) {
                    range.1 = DocCharOffset(old_index + old_len);
                    return Ok(());
                }
            } else {
                unreachable!();
            }
        }

        let op = Operation::Replace(Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index + old_len)),
            text: String::new(),
        });

        self.ops.push(op);
        Ok(())
    }

    fn insert(
        &mut self, old_index: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let new_text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)));

        if let Some(op) = self.ops.last_mut() {
            if let Operation::Replace(Replace { range, text }) = op {
                if range.1 == DocCharOffset(old_index) {
                    text.push_str(new_text);
                    return Ok(());
                }
            } else {
                unreachable!();
            }
        }

        let op = Operation::Replace(Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index)),
            text: new_text.into(),
        });

        self.ops.push(op);
        Ok(())
    }

    fn replace(
        &mut self, old_index: usize, old_len: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let new_text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)));

        if let Some(op) = self.ops.last_mut() {
            if let Operation::Replace(Replace { range, text }) = op {
                if range.1 == DocCharOffset(old_index) {
                    range.1 = DocCharOffset(old_index + old_len);
                    text.push_str(new_text);
                    return Ok(());
                }
            } else {
                unreachable!();
            }
        }

        let op = Operation::Replace(Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index + old_len)),
            text: new_text.into(),
        });

        self.ops.push(op);
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
