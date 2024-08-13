use similar::{algorithms::DiffHook, DiffableStr as _, DiffableStrRef as _};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::tab::markdown_editor::{buffer::Operation, offset_types::DocCharOffset};

// implementation note: this works because similar uses the same grapheme definition as we do, so reported indexes can
// be interpreted as doc char offsets
pub fn patch_ops(base: &str, remote: &str) -> Vec<Operation> {
    println!("\n----- merge -----");
    println!("base: {}", base);
    println!("remote: {}", remote);

    let out_of_editor_mutations = {
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
        hook.ops()
    };

    println!("out_of_editor_mutations: {:?}", out_of_editor_mutations);

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
        let op = Operation::Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index + old_len)),
            text: String::new(),
        };

        self.ops.push(op);
        Ok(())
    }

    fn insert(
        &mut self, old_index: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)))
            .to_string();
        let op = Operation::Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index)),
            text,
        };

        self.ops.push(op);
        Ok(())
    }

    fn replace(
        &mut self, old_index: usize, old_len: usize, new_index: usize, new_len: usize,
    ) -> Result<(), Self::Error> {
        let text = self
            .new
            .grapheme_index((DocCharOffset(new_index), DocCharOffset(new_index + new_len)))
            .to_string();
        let op = Operation::Replace {
            range: (DocCharOffset(old_index), DocCharOffset(old_index + old_len)),
            text,
        };

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
