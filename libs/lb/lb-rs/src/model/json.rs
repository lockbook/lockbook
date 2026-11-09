//! Three-way merge of JSON documents. Objects merge key by key and lists merge by value; every
//! other value is exact. When both sides change a value differently, a delete wins, otherwise
//! ours does. Ours is the device merging, which syncs second.

use serde_json::{Map, Value};
use similar::DiffOp;
use similar::algorithms::{Capture, DiffHook, Replace, myers};
use std::ops::Range;

/// The merged document, or none if a side isn't JSON. An empty base is a document both sides
/// created.
pub fn merge(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    if theirs == base || ours == theirs {
        return Some(ours.to_vec());
    }
    if ours == base {
        return Some(theirs.to_vec());
    }
    let base = if base.is_empty() { None } else { Some(serde_json::from_slice(base).ok()?) };
    let ours_value = serde_json::from_slice(ours).ok()?;
    let theirs_value = serde_json::from_slice(theirs).ok()?;

    let merged = merge_changed(base.as_ref(), ours_value, theirs_value);
    let mut bytes = serde_json::to_vec_pretty(&merged).ok()?;
    if ours.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    Some(bytes)
}

/// A value that may be absent on either side; absent is deleted, or never added.
fn merge_slot(base: Option<&Value>, ours: Option<Value>, theirs: Option<Value>) -> Option<Value> {
    if theirs.as_ref() == base || ours == theirs {
        return ours;
    }
    if ours.as_ref() == base {
        return theirs;
    }
    Some(merge_changed(base, ours?, theirs?))
}

/// A value both sides changed, differently. Containers merge only if both sides added them or
/// the base was the same kind; a type change is exact.
fn merge_changed(base: Option<&Value>, ours: Value, theirs: Value) -> Value {
    match (base, ours, theirs) {
        (None | Some(Value::Object(_)), Value::Object(ours), Value::Object(theirs)) => {
            Value::Object(merge_objects(base.and_then(Value::as_object), ours, theirs))
        }
        (None | Some(Value::Array(_)), Value::Array(ours), Value::Array(theirs)) => {
            let base = base
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice);
            Value::Array(merge_lists(base, ours, theirs))
        }
        (_, ours, _) => ours,
    }
}

/// Keys keep ours' order, with keys only theirs added at the end.
fn merge_objects(
    base: Option<&Map<String, Value>>, ours: Map<String, Value>, mut theirs: Map<String, Value>,
) -> Map<String, Value> {
    let base_value = |key: &str| base.and_then(|base| base.get(key));
    let mut merged = Map::new();
    for (key, ours) in ours {
        let theirs = theirs.shift_remove(&key);
        if let Some(value) = merge_slot(base_value(&key), Some(ours), theirs) {
            merged.insert(key, value);
        }
    }
    for (key, theirs) in theirs {
        if let Some(value) = merge_slot(base_value(&key), None, Some(theirs)) {
            merged.insert(key, value);
        }
    }
    merged
}

/// One side's replacement of `base[start..end]` with `side[from..to]`.
#[derive(Clone, Copy)]
struct Change {
    start: usize,
    end: usize,
    from: usize,
    to: usize,
}

impl Change {
    fn deletes_only(self) -> bool {
        self.from == self.to
    }

    /// Two changes overlap if they share a base element, or one inserts strictly inside the
    /// other. Two inserts at one point don't.
    fn overlaps(self, start: usize, end: usize) -> bool {
        self.start < end && start < self.end
    }
}

fn changes(base: &[Value], side: &[Value]) -> Vec<Change> {
    let mut hook = Replace::new(Capture::new());
    let Ok(()) = myers::diff(&mut hook, base, 0..base.len(), side, 0..side.len());
    let Ok(()) = hook.finish();
    hook.into_inner()
        .into_ops()
        .into_iter()
        .filter_map(|op| match op {
            DiffOp::Equal { .. } => None,
            DiffOp::Delete { old_index, old_len, new_index } => Some(Change {
                start: old_index,
                end: old_index + old_len,
                from: new_index,
                to: new_index,
            }),
            DiffOp::Insert { old_index, new_index, new_len } => Some(Change {
                start: old_index,
                end: old_index,
                from: new_index,
                to: new_index + new_len,
            }),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => Some(Change {
                start: old_index,
                end: old_index + old_len,
                from: new_index,
                to: new_index + new_len,
            }),
        })
        .collect()
}

/// Walks the base once, applying each side's changes where they don't overlap. Overlapping
/// changes from both sides grow into one hunk that one side wins whole: a side that only deleted
/// there, both deletions if both did, otherwise ours.
fn merge_lists(base: &[Value], mut ours: Vec<Value>, mut theirs: Vec<Value>) -> Vec<Value> {
    let ours_changes = changes(base, &ours);
    let theirs_changes = changes(base, &theirs);
    let (mut o, mut t) = (0, 0);
    let mut merged = Vec::with_capacity(base.len().max(ours.len()));
    let mut cursor = 0;

    loop {
        match (ours_changes.get(o).copied(), theirs_changes.get(t).copied()) {
            (None, None) => break,
            (Some(a), Some(b)) if a.overlaps(b.start, b.end) => {
                let (o_from, t_from) = (o, t);
                let (mut start, mut end) = (a.start.min(b.start), a.end.max(b.end));
                (o, t) = (o + 1, t + 1);
                loop {
                    let next = if ours_changes.get(o).is_some_and(|c| c.overlaps(start, end)) {
                        o += 1;
                        ours_changes[o - 1]
                    } else if theirs_changes
                        .get(t)
                        .is_some_and(|c| c.overlaps(start, end))
                    {
                        t += 1;
                        theirs_changes[t - 1]
                    } else {
                        break;
                    };
                    (start, end) = (start.min(next.start), end.max(next.end));
                }
                merged.extend_from_slice(&base[cursor..start]);
                let ours_hunk = &ours_changes[o_from..o];
                let theirs_hunk = &theirs_changes[t_from..t];
                let ours_deletes = ours_hunk.iter().all(|c| c.deletes_only());
                let theirs_deletes = theirs_hunk.iter().all(|c| c.deletes_only());
                if ours_deletes && theirs_deletes {
                    let kept = |&i: &usize| {
                        !ours_hunk
                            .iter()
                            .chain(theirs_hunk)
                            .any(|c| (c.start..c.end).contains(&i))
                    };
                    merged.extend((start..end).filter(kept).map(|i| base[i].clone()));
                } else if theirs_deletes {
                    apply(&mut merged, base, start..end, theirs_hunk, &mut theirs);
                } else {
                    apply(&mut merged, base, start..end, ours_hunk, &mut ours);
                }
                cursor = end;
            }
            (Some(a), Some(b))
                if (a.start, a.end) == (b.start, b.end)
                    && ours[a.from..a.to] == theirs[b.from..b.to] =>
            {
                apply(&mut merged, base, cursor..a.end, &[a], &mut ours);
                (o, t, cursor) = (o + 1, t + 1, a.end);
            }
            (a, b) => {
                // a lone insert goes before a change starting at the same element, and ours goes
                // before theirs at the same point
                let order = |c: Change| (c.start, c.end > c.start);
                let ours_first = match (a, b) {
                    (Some(a), Some(b)) => order(a) <= order(b),
                    (a, _) => a.is_some(),
                };
                let (change, side) = if ours_first {
                    o += 1;
                    (ours_changes[o - 1], &mut ours)
                } else {
                    t += 1;
                    (theirs_changes[t - 1], &mut theirs)
                };
                apply(&mut merged, base, cursor..change.end, &[change], side);
                cursor = change.end;
            }
        }
    }
    merged.extend_from_slice(&base[cursor..]);
    merged
}

/// Writes `base[span]` with one side's changes inside it applied, moving their values out of
/// `side`.
fn apply(
    merged: &mut Vec<Value>, base: &[Value], span: Range<usize>, changes: &[Change],
    side: &mut [Value],
) {
    let (mut cursor, end) = (span.start, span.end);
    for change in changes {
        merged.extend_from_slice(&base[cursor..change.start]);
        merged.extend(side[change.from..change.to].iter_mut().map(std::mem::take));
        cursor = change.end;
    }
    merged.extend_from_slice(&base[cursor..end]);
}
