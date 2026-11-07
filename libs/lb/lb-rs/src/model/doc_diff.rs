use std::ops::Range;

use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffTag};
use web_time::{Duration, Instant};

use super::errors::{LbErrKind, LbResult, Unexpected};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Insert {
        at: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    Replace {
        start: u64,
        len: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct DocDiff {
    pub ops: Vec<DiffOp>,
}

#[derive(Serialize, Deserialize)]
enum DiffFormat {
    V1(DocDiff),
}

const DEADLINE: Duration = Duration::from_millis(50);
const MIN_GAP: usize = 8;

impl DocDiff {
    pub fn compute(base: &[u8], target: &[u8]) -> Self {
        let prefix = base.iter().zip(target).take_while(|(a, b)| a == b).count();
        let suffix = base[prefix..]
            .iter()
            .rev()
            .zip(target[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let base_middle = &base[prefix..base.len() - suffix];
        let target_middle = &target[prefix..target.len() - suffix];

        let mut edits: Vec<(Range<usize>, Vec<u8>)> = Vec::new();
        let mut edit = |replaced: Range<usize>, bytes: &[u8]| {
            if replaced.is_empty() && bytes.is_empty() {
                return;
            }
            if let Some((last, last_bytes)) = edits.last_mut() {
                if replaced.start - last.end < MIN_GAP {
                    last_bytes.extend_from_slice(&base[last.end..replaced.start]);
                    last_bytes.extend_from_slice(bytes);
                    last.end = replaced.end;
                    return;
                }
            }
            edits.push((replaced, bytes.to_vec()));
        };

        if base_middle.is_empty() || target_middle.is_empty() {
            edit(prefix..base.len() - suffix, target_middle);
        } else {
            let (mut base_at, mut target_at) = (prefix, 0);
            for op in similar::capture_diff_slices_deadline(
                Algorithm::Myers,
                base_middle,
                target_middle,
                Some(Instant::now() + DEADLINE),
            ) {
                let (tag, old, new) = op.as_tag_tuple();
                let replaced = base_at..base_at + old.len();
                let bytes = &target_middle[target_at..target_at + new.len()];
                base_at = replaced.end;
                target_at += new.len();
                if tag != DiffTag::Equal {
                    edit(replaced, bytes);
                }
            }
        }

        let ops = edits
            .into_iter()
            .map(|(replaced, bytes)| {
                if replaced.is_empty() {
                    DiffOp::Insert { at: replaced.start as u64, bytes }
                } else {
                    DiffOp::Replace {
                        start: replaced.start as u64,
                        len: replaced.len() as u64,
                        bytes,
                    }
                }
            })
            .collect();
        DocDiff { ops }
    }

    pub fn encode(self) -> LbResult<Vec<u8>> {
        bincode::serialize(&DiffFormat::V1(self)).map_unexpected()
    }

    pub fn decode(bytes: &[u8]) -> LbResult<Self> {
        match bincode::deserialize(bytes).map_unexpected()? {
            DiffFormat::V1(diff) => Ok(diff),
        }
    }

    pub fn apply(&self, base: &[u8]) -> LbResult<Vec<u8>> {
        let mut result = Vec::with_capacity(base.len());
        let mut cursor = 0;
        for op in &self.ops {
            let (start, len, bytes) = match op {
                DiffOp::Insert { at, bytes } => (*at, 0, bytes),
                DiffOp::Replace { start, len, bytes } => (*start, *len, bytes),
            };
            let replaced = usize::try_from(start)
                .ok()
                .zip(usize::try_from(len).ok())
                .and_then(|(start, len)| Some(start..start.checked_add(len)?))
                .filter(|replaced| cursor <= replaced.start && replaced.end <= base.len())
                .ok_or_else(|| {
                    LbErrKind::Unexpected(
                        "diff edits are out of order or outside of their base".to_string(),
                    )
                })?;
            result.extend_from_slice(&base[cursor..replaced.start]);
            result.extend_from_slice(bytes);
            cursor = replaced.end;
        }
        result.extend_from_slice(&base[cursor..]);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn roundtrip(base: &[u8], target: &[u8]) -> DocDiff {
        let diff = DocDiff::compute(base, target);
        assert_eq!(diff.apply(base).unwrap(), target);
        diff
    }

    #[test]
    fn typing_is_one_insert() {
        let diff = roundtrip(b"hello world", b"hello brave world");
        assert_eq!(diff.ops, vec![DiffOp::Insert { at: 6, bytes: b"brave ".to_vec() }]);
    }

    #[test]
    fn deleting_is_one_empty_replace() {
        let diff = roundtrip(b"hello brave world", b"hello world");
        assert_eq!(diff.ops, vec![DiffOp::Replace { start: 6, len: 6, bytes: vec![] }]);
    }

    #[test]
    fn edge_cases_roundtrip() {
        roundtrip(b"", b"");
        roundtrip(b"", b"new");
        roundtrip(b"old", b"");
        roundtrip(b"same", b"same");
        roundtrip(b"aaaa", b"aa");
        roundtrip(b"aa", b"aaaa");
        roundtrip(b"abcabc", b"cbacba");
    }

    #[test]
    fn scattered_edits_leave_the_rest_alone() {
        let base = b"line one\nline two\nline three\nline four\n".repeat(50);
        let mut target = base.clone();
        target[10] = b'X';
        let last = target.len() - 10;
        target[last] = b'Y';
        let diff = roundtrip(&base, &target);
        assert_eq!(
            diff.ops,
            vec![
                DiffOp::Replace { start: 10, len: 1, bytes: b"X".to_vec() },
                DiffOp::Replace { start: last as u64, len: 1, bytes: b"Y".to_vec() },
            ]
        );
    }

    #[test]
    fn rewrites_are_one_replace() {
        let diff = roundtrip(
            b"Intro that stays.\nThe quick brown fox jumps over the lazy dog.\nOutro that stays.\n",
            b"Intro that stays.\nPack my box with five dozen liquor jugs.\nOutro that stays.\n",
        );
        assert_eq!(
            diff.ops,
            vec![DiffOp::Replace {
                start: 18,
                len: 43,
                bytes: b"Pack my box with five dozen liquor jugs".to_vec()
            }]
        );
    }

    #[test]
    fn inserts_go_after_the_match_before_them() {
        roundtrip(b"ca", b"aac");
    }

    #[test]
    fn random_edits_roundtrip() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let len = rng.gen_range(0..400);
            let base: Vec<u8> = (0..len).map(|_| rng.gen_range(b'a'..b'e')).collect();
            let mut target = base.clone();
            for _ in 0..rng.gen_range(0..6) {
                let at = rng.gen_range(0..=target.len());
                match rng.gen_range(0..3) {
                    0 => {
                        let end = rng.gen_range(at..=target.len());
                        target.drain(at..end);
                    }
                    1 => {
                        let insert: Vec<u8> = (0..rng.gen_range(0..20))
                            .map(|_| rng.gen_range(b'a'..b'e'))
                            .collect();
                        target.splice(at..at, insert);
                    }
                    _ => {
                        if at < target.len() {
                            target[at] = rng.gen_range(b'a'..b'e');
                        }
                    }
                }
            }
            roundtrip(&base, &target);
        }
    }

    #[test]
    fn encoding_roundtrips() {
        let diff = DocDiff::compute(b"base content", b"base new content");
        assert_eq!(DocDiff::decode(&diff.clone().encode().unwrap()).unwrap(), diff);
    }

    #[test]
    fn edits_out_of_order_or_outside_the_base_are_rejected() {
        let replace = |start, len| DiffOp::Replace { start, len, bytes: b"new".to_vec() };
        for ops in [
            vec![replace(3, 5)],
            vec![replace(u64::MAX, 2)],
            vec![DiffOp::Insert { at: 6, bytes: vec![] }],
            vec![replace(2, 1), replace(1, 1)],
            vec![replace(0, 3), replace(2, 1)],
        ] {
            assert!(DocDiff { ops }.apply(b"short").is_err());
        }
    }
}
