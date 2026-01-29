pub mod buffer;
pub mod offset_types;
pub mod operation_types;
pub mod unicode_segs;

use offset_types::{DocByteOffset, RangeExt as _};
use operation_types::Replace;

use similar::DiffableStrRef as _;
use unicode_segmentation::UnicodeSegmentation as _;

pub fn diff(from: &str, to: &str) -> Vec<Replace> {
    let mut result = Vec::new();

    let from_segs = unicode_segs::calc(from);
    let to_segs = unicode_segs::calc(to);

    let mut from_words: Vec<_> = from
        .split_word_bound_indices()
        .map(|(idx, _)| DocByteOffset(idx))
        .collect();
    from_words.push(DocByteOffset(from.len()));

    let mut to_words: Vec<_> = to
        .split_word_bound_indices()
        .map(|(idx, _)| DocByteOffset(idx))
        .collect();
    to_words.push(DocByteOffset(to.len()));

    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .diff_unicode_words(from.as_diffable_str(), to.as_diffable_str());

    for diff_op in diff.ops().iter().cloned() {
        match diff_op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete { old_index, old_len, .. } => {
                let old_len = from_segs.offset_to_char(from_words[old_index + old_len])
                    - from_segs.offset_to_char(from_words[old_index]);
                let old_index = from_segs.offset_to_char(from_words[old_index]);

                let mut extended = false;
                if let Some(op) = result.last_mut() {
                    let Replace { range, .. } = op;
                    if range.1 == old_index {
                        range.1 = old_index + old_len;
                        extended = true;
                    }
                }

                if !extended {
                    let op =
                        Replace { range: (old_index, old_index + old_len), text: String::new() };
                    result.push(op);
                }
            }
            similar::DiffOp::Insert { old_index, new_index, new_len } => {
                let old_index = from_segs.offset_to_char(from_words[old_index]);
                let new_len = to_segs.offset_to_char(to_words[new_index + new_len])
                    - to_segs.offset_to_char(to_words[new_index]);
                let new_index = to_segs.offset_to_char(to_words[new_index]);

                let new_text_range = to_segs.range_to_byte((new_index, new_index + new_len));
                let new_text = to[new_text_range.start().0..new_text_range.end().0].to_string();

                let mut extended = false;
                if let Some(op) = result.last_mut() {
                    let Replace { range, text } = op;
                    if range.1 == old_index {
                        text.push_str(&new_text);
                        extended = true;
                    }
                }

                if !extended {
                    let op = Replace { range: (old_index, old_index), text: new_text };
                    result.push(op);
                }
            }
            similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                let old_len = from_segs.offset_to_char(from_words[old_index + old_len])
                    - from_segs.offset_to_char(from_words[old_index]);
                let old_index = from_segs.offset_to_char(from_words[old_index]);
                let new_len = to_segs.offset_to_char(to_words[new_index + new_len])
                    - to_segs.offset_to_char(to_words[new_index]);
                let new_index = to_segs.offset_to_char(to_words[new_index]);

                let new_text_range = to_segs.range_to_byte((new_index, new_index + new_len));
                let new_text = to[new_text_range.start().0..new_text_range.end().0].to_string();

                let mut extended = false;
                if let Some(op) = result.last_mut() {
                    let Replace { range, text } = op;
                    if range.1 == old_index {
                        range.1 = old_index + old_len;
                        text.push_str(&new_text);
                        extended = true;
                    }
                }

                if !extended {
                    let op = Replace { range: (old_index, old_index + old_len), text: new_text };
                    result.push(op);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod test {
    use rand::rngs::StdRng;
    use rand::{Rng as _, SeedableRng as _};

    #[test]
    fn diff_full_replace() {
        let from = "Hello";
        let to = "Goodbye";

        let result = super::diff(from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range, (0.into(), 5.into()));
        assert_eq!(result[0].text, "Goodbye");
    }

    #[test]
    fn diff_partial_replace() {
        let from = "Hello, world!";
        let to = "Hello, Rust!";

        let result = super::diff(from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range, (7.into(), 12.into()));
        assert_eq!(result[0].text, "Rust");
    }

    #[test]
    fn diff_fuzz() {
        let mut count = 0;
        let mut rng = StdRng::seed_from_u64(0);
        loop {
            let from: String = rand_str(&mut rng, rand::random::<usize>() % 10);
            let to: String = rand_str(&mut rng, rand::random::<usize>() % 10);
            let _ = super::diff(&from, &to);
            count += 1;
            if count == 1000 {
                break;
            }
        }
    }

    fn rand_str(rng: &mut StdRng, length: usize) -> String {
        let unicode_string: String = (0..length)
            .map(|_| {
                let code_point = rng.gen_range(0x0020..=0xD7FF);
                std::char::from_u32(code_point).unwrap_or('?')
            })
            .collect();
        unicode_string
    }
}
