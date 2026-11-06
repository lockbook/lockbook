use serde::{Deserialize, Serialize};
use similar::{Algorithm, DiffOp as SimilarOp};

use super::errors::{LbErrKind, LbResult, Unexpected};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Copy { start: u64, len: u64 },
    Insert(#[serde(with = "serde_bytes")] Vec<u8>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct DocDiff {
    pub ops: Vec<DiffOp>,
}

#[derive(Serialize, Deserialize)]
enum DiffFormat {
    V1(DocDiff),
}

#[cfg(target_family = "wasm")]
const MAX_UNTIMED_MIDDLE: usize = 1 << 16;

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

        let mut diff = DocDiff::default();
        diff.copy(0, prefix);
        if base_middle.is_empty()
            || target_middle.is_empty()
            || !Self::worth_aligning(base_middle, target_middle)
        {
            diff.insert(target_middle);
        } else {
            for op in similar::capture_diff_slices_deadline(
                Algorithm::Myers,
                base_middle,
                target_middle,
                Self::deadline(),
            ) {
                match op {
                    SimilarOp::Equal { old_index, len, .. } => diff.copy(prefix + old_index, len),
                    SimilarOp::Delete { .. } => {}
                    SimilarOp::Insert { new_index, new_len, .. }
                    | SimilarOp::Replace { new_index, new_len, .. } => {
                        diff.insert(&target_middle[new_index..new_index + new_len])
                    }
                }
            }
        }
        diff.copy(base.len() - suffix, suffix);
        diff
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
        for op in &self.ops {
            match op {
                DiffOp::Copy { start, len } => {
                    let copied = usize::try_from(*start)
                        .ok()
                        .zip(usize::try_from(*len).ok())
                        .and_then(|(start, len)| Some(start..start.checked_add(len)?))
                        .and_then(|range| base.get(range))
                        .ok_or_else(|| {
                            LbErrKind::Unexpected("diff copies outside of its base".to_string())
                        })?;
                    result.extend_from_slice(copied);
                }
                DiffOp::Insert(bytes) => result.extend_from_slice(bytes),
            }
        }
        Ok(result)
    }

    fn copy(&mut self, start: usize, len: usize) {
        if len == 0 {
            return;
        }
        if let Some(DiffOp::Copy { start: last_start, len: last_len }) = self.ops.last_mut() {
            if *last_start + *last_len == start as u64 {
                *last_len += len as u64;
                return;
            }
        }
        self.ops
            .push(DiffOp::Copy { start: start as u64, len: len as u64 });
    }

    fn insert(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(DiffOp::Insert(last)) = self.ops.last_mut() {
            last.extend_from_slice(bytes);
            return;
        }
        self.ops.push(DiffOp::Insert(bytes.to_vec()));
    }

    #[cfg(not(target_family = "wasm"))]
    fn worth_aligning(_base: &[u8], _target: &[u8]) -> bool {
        true
    }

    #[cfg(target_family = "wasm")]
    fn worth_aligning(base: &[u8], target: &[u8]) -> bool {
        base.len() + target.len() <= MAX_UNTIMED_MIDDLE
    }

    #[cfg(not(target_family = "wasm"))]
    fn deadline() -> Option<std::time::Instant> {
        Some(std::time::Instant::now() + std::time::Duration::from_millis(50))
    }

    #[cfg(target_family = "wasm")]
    fn deadline() -> Option<std::time::Instant> {
        None
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
    fn typing_is_one_insert_between_two_copies() {
        let diff = roundtrip(b"hello world", b"hello brave world");
        assert_eq!(
            diff.ops,
            vec![
                DiffOp::Copy { start: 0, len: 6 },
                DiffOp::Insert(b"brave ".to_vec()),
                DiffOp::Copy { start: 6, len: 5 },
            ]
        );
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
    fn scattered_edits_copy_the_unchanged_parts() {
        let base = b"line one\nline two\nline three\nline four\n".repeat(50);
        let mut target = base.clone();
        target[10] = b'X';
        let last = target.len() - 10;
        target[last] = b'Y';
        let diff = roundtrip(&base, &target);
        let inserted: usize = diff
            .ops
            .iter()
            .map(|op| match op {
                DiffOp::Insert(bytes) => bytes.len(),
                DiffOp::Copy { .. } => 0,
            })
            .sum();
        assert!(inserted < 10);
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
    fn copies_outside_the_base_are_rejected() {
        let diff = DocDiff { ops: vec![DiffOp::Copy { start: 3, len: 5 }] };
        assert!(diff.apply(b"short").is_err());
        let diff = DocDiff { ops: vec![DiffOp::Copy { start: u64::MAX, len: 2 }] };
        assert!(diff.apply(b"short").is_err());
    }
}
