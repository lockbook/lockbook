//! A three-way merge of JSON with a routine per kind of value, each
//! recursing into the next as the document nests: objects key by key, arrays
//! as sequences, anything else whole. Concurrent edits to one value go to the
//! last writer, `ours`, the side saving now; a deletion wins over any edit of
//! what it deleted.

use std::hash::Hash;

use serde_json::{Map, Value};
use similar::{Algorithm, DiffOp, capture_diff_slices};

/// `base` with the changes `ours` and `theirs` each made to it.
pub fn merge(base: &Value, ours: &Value, theirs: &Value) -> Value {
    if ours == theirs || theirs == base {
        return ours.clone();
    }
    if ours == base {
        return theirs.clone();
    }
    match (base, ours, theirs) {
        (Value::Object(b), Value::Object(o), Value::Object(t)) => {
            Value::Object(merge_objects(b, o, t))
        }
        (Value::Array(b), Value::Array(o), Value::Array(t)) => Value::Array(merge_arrays(b, o, t)),
        _ => merge_added(ours, theirs),
    }
}

/// The documents merged, or none if one isn't JSON. An empty base is a
/// document both sides added to.
pub fn merge_bytes(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let ours = serde_json::from_slice::<Value>(ours).ok()?;
    let theirs = serde_json::from_slice::<Value>(theirs).ok()?;
    let merged = if base.is_empty() {
        merge_added(&ours, &theirs)
    } else {
        merge(&serde_json::from_slice(base).ok()?, &ours, &theirs)
    };
    let mut bytes = serde_json::to_vec_pretty(&merged).ok()?;
    bytes.push(b'\n');
    Some(bytes)
}

/// Two values added where there was none: merged as if from empty when
/// they're both objects or both arrays, else the last writer's.
fn merge_added(ours: &Value, theirs: &Value) -> Value {
    match (ours, theirs) {
        (Value::Object(o), Value::Object(t)) => Value::Object(merge_objects(&Map::new(), o, t)),
        (Value::Array(o), Value::Array(t)) => Value::Array(merge_arrays(&[], o, t)),
        _ => ours.clone(),
    }
}

/// Key by key: a key either side deleted is gone, whatever the other did.
fn merge_objects(
    base: &Map<String, Value>, ours: &Map<String, Value>, theirs: &Map<String, Value>,
) -> Map<String, Value> {
    let mut merged = Map::new();
    for key in ours.keys().chain(theirs.keys()) {
        if merged.contains_key(key) {
            continue;
        }
        let value = match (base.get(key), ours.get(key), theirs.get(key)) {
            (Some(b), Some(o), Some(t)) => merge(b, o, t),
            (Some(_), _, _) => continue,
            (None, Some(o), Some(t)) => merge_added(o, t),
            (None, Some(v), None) | (None, None, Some(v)) => v.clone(),
            (None, None, None) => continue,
        };
        merged.insert(key.clone(), value);
    }
    merged
}

fn merge_arrays(base: &[Value], ours: &[Value], theirs: &[Value]) -> Vec<Value> {
    // compared by their text, since values themselves don't hash
    let text = |values: &[Value]| -> Vec<String> { values.iter().map(Value::to_string).collect() };
    let keys = [text(base), text(ours), text(theirs)];
    merge_sequences(base, ours, theirs, &keys, merge)
}

/// A sequence merge for any elements, `keys` standing in for `base`, `ours`
/// and `theirs` when diffing them (Myers, as text is). Between elements
/// neither side changed: a stretch only one side changed takes that side's;
/// a stretch one side deleted is gone; elements both sides changed one for
/// one merge with `each`; insertions both made in one place are both kept,
/// theirs first; anything else is the last writer's.
pub fn merge_sequences<T: Clone, K: Hash + Eq + Ord>(
    base: &[T], ours: &[T], theirs: &[T], keys: &[Vec<K>; 3], each: impl Fn(&T, &T, &T) -> T,
) -> Vec<T> {
    let [kb, ko, kt] = keys;
    let kept = |side: &[K]| -> Vec<Option<usize>> {
        let mut at = vec![None; kb.len()];
        for op in capture_diff_slices(Algorithm::Myers, kb, side) {
            if let DiffOp::Equal { old_index, new_index, len } = op {
                for i in 0..len {
                    at[old_index + i] = Some(new_index + i);
                }
            }
        }
        at
    };
    let (in_ours, in_theirs) = (kept(ko), kept(kt));

    let mut merged = Vec::with_capacity(ours.len().max(theirs.len()));
    let (mut b, mut o, mut t) = (0, 0, 0);
    loop {
        // the next element both sides kept
        let next = (b..base.len()).find_map(|i| Some((i, in_ours[i]?, in_theirs[i]?)));
        let (nb, no, nt) = next.unwrap_or((base.len(), ours.len(), theirs.len()));
        let (sb, so, st) = (&base[b..nb], &ours[o..no], &theirs[t..nt]);
        let (kb, ko, kt) = (&kb[b..nb], &ko[o..no], &kt[t..nt]);
        if ko == kb || ko == kt {
            merged.extend_from_slice(st);
        } else if kt == kb {
            merged.extend_from_slice(so);
        } else if sb.is_empty() {
            merged.extend_from_slice(st);
            merged.extend_from_slice(so);
        } else if so.is_empty() || st.is_empty() {
            // deleted on one side: gone
        } else if so.len() == sb.len() && st.len() == sb.len() {
            merged.extend((0..sb.len()).map(|i| each(&sb[i], &so[i], &st[i])));
        } else {
            merged.extend_from_slice(so);
        }
        let Some((nb, no, nt)) = next else { break };
        merged.push(ours[no].clone());
        (b, o, t) = (nb + 1, no + 1, nt + 1);
    }
    merged
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn edits_to_different_keys_and_elements_both_stay() {
        let base =
            json!({ "nodes": [{ "id": "a", "at": [0, 0], "q": 1 }, { "id": "b" }], "bpm": 96 });
        let ours =
            json!({ "nodes": [{ "id": "a", "at": [5, 5], "q": 1 }, { "id": "b" }], "bpm": 96 });
        let theirs =
            json!({ "nodes": [{ "id": "a", "at": [0, 0], "q": 9 }, { "id": "b" }], "bpm": 120 });
        let merged = merge(&base, &ours, &theirs);
        assert_eq!(
            merged,
            json!({ "nodes": [{ "id": "a", "at": [5, 5], "q": 9 }, { "id": "b" }], "bpm": 120 })
        );
    }

    #[test]
    fn the_last_writer_wins_a_value_both_changed() {
        assert_eq!(
            merge(&json!({ "q": 1 }), &json!({ "q": 2 }), &json!({ "q": 3 })),
            json!({ "q": 2 })
        );
        assert_eq!(
            merge(&json!([1, 2, 3]), &json!([1, 7, 3]), &json!([1, 8, 3])),
            json!([1, 7, 3])
        );
    }

    #[test]
    fn a_deletion_wins_over_an_edit() {
        let base = json!({ "a": { "x": 1 }, "list": [1, 2, 3] });
        let ours = json!({ "list": [1, 3] });
        let theirs = json!({ "a": { "x": 2 }, "list": [1, 9, 3] });
        assert_eq!(merge(&base, &ours, &theirs), json!({ "list": [1, 3] }));
    }

    #[test]
    fn insertions_on_both_sides_are_both_kept() {
        let merged = merge(&json!([1, 5]), &json!([1, 2, 5, 6]), &json!([0, 1, 3, 5]));
        assert_eq!(merged, json!([0, 1, 3, 2, 5, 6]));
    }

    #[test]
    fn bare_numbers_merge_as_a_sequence() {
        let base = json!([1, 2, 3, 4, 5]);
        let ours = json!([1, 2, 3, 4, 5, 6]);
        let theirs = json!([0, 1, 2, 4, 5]);
        assert_eq!(merge(&base, &ours, &theirs), json!([0, 1, 2, 4, 5, 6]));
    }

    #[test]
    fn merging_with_no_change_on_one_side_is_the_other() {
        for (base, side) in [
            (json!([1, { "a": [2, 3] }]), json!([{ "a": [3] }, 4])),
            (json!({ "k": null }), json!({ "k": [1, 2], "j": true })),
        ] {
            assert_eq!(merge(&base, &side, &base), side);
            assert_eq!(merge(&base, &base, &side), side);
            assert_eq!(merge(&base, &side, &side), side);
        }
    }

    /// A small random JSON value, nested up to `depth`.
    fn value(rng: &mut impl rand::Rng, depth: u32) -> Value {
        match rng.gen_range(0..if depth == 0 { 3 } else { 5 }) {
            0 => json!(rng.gen_range(0..4)),
            1 => json!(["a", "b", "c"][rng.gen_range(0..3)]),
            2 => Value::Null,
            3 => Value::Array(
                (0..rng.gen_range(0..5))
                    .map(|_| value(rng, depth - 1))
                    .collect(),
            ),
            _ => Value::Object(
                (0..rng.gen_range(0..4))
                    .map(|_| {
                        (["k", "j", "id"][rng.gen_range(0..3)].to_string(), value(rng, depth - 1))
                    })
                    .collect(),
            ),
        }
    }

    /// `v` with a few random edits: values replaced, elements and keys
    /// added or removed.
    fn edited(rng: &mut impl rand::Rng, v: &Value, depth: u32) -> Value {
        if rng.gen_ratio(1, 5) {
            return value(rng, depth);
        }
        match v {
            Value::Array(a) => {
                let mut a: Vec<Value> = a
                    .iter()
                    .map(|e| edited(rng, e, depth.saturating_sub(1)))
                    .collect();
                if !a.is_empty() && rng.gen_ratio(1, 3) {
                    a.remove(rng.gen_range(0..a.len()));
                }
                if rng.gen_ratio(1, 3) {
                    let at = rng.gen_range(0..=a.len());
                    a.insert(at, value(rng, depth.saturating_sub(1)));
                }
                Value::Array(a)
            }
            Value::Object(o) => {
                let mut kept = Map::new();
                for (k, e) in o {
                    if !rng.gen_ratio(1, 4) {
                        kept.insert(k.clone(), edited(rng, e, depth.saturating_sub(1)));
                    }
                }
                let mut o = kept;
                if rng.gen_ratio(1, 3) {
                    o.insert("new".into(), value(rng, depth.saturating_sub(1)));
                }
                Value::Object(o)
            }
            v => v.clone(),
        }
    }

    #[test]
    fn a_side_with_no_change_leaves_the_other_and_one_change_twice_is_once() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        for _ in 0..5000 {
            let base = value(&mut rng, 3);
            let side = edited(&mut rng, &base, 3);
            assert_eq!(merge(&base, &side, &base), side, "{base} {side}");
            assert_eq!(merge(&base, &base, &side), side, "{base} {side}");
            assert_eq!(merge(&base, &side, &side), side, "{base} {side}");
            // and merging what anything produces never panics
            let other = edited(&mut rng, &base, 3);
            merge(&base, &side, &other);
        }
    }
}
