use super::offset_types::{Byte, Grapheme, Graphemes, RangeExt};
use super::operation_types::{InverseOperation, Operation, Replace};
use super::unicode_segs::UnicodeSegs;
use super::{diff, unicode_segs};
use std::ops::Index;
use web_time::{Duration, Instant};

/// Long-lived state of the editor's text buffer. Factored into sub-structs for borrow-checking.
/// # Operation algebra
/// Operations are created based on a version of the buffer. This version is called the operation's base and is
/// identified with a sequence number. When the base of an operation is equal to the buffer's current sequence number,
/// the operation can be applied and increments the buffer's sequence number.
///
/// When multiple operations are created based on the same version of the buffer, such as when a user types a few
/// keystrokes in one frame or issues a command like indenting multiple list items, the operations all have the same
/// base. Once the first operation is applied and the buffer's sequence number is incremented, the base of the
/// remaining operations must be incremented by using the first operation to transform them before they can be applied.
/// This corresponds to the reality that the buffer state has changed since the operation was created and the operation
/// must be re-interpreted. For example, if text is typed at the beginning then end of a buffer in one frame, the
/// position of the text typed at the end of the buffer is greater when it is applied than it was when it was typed.
///
/// External changes are merged into the buffer by creating a set of operations that would transform the buffer from
/// the last external state to the current state. These operations, based on the version of the buffer at the last
/// successful save or load, must be transformed by all operations that have been applied since (this means we must
/// preserve the undo history for at least that long; if this presents performance issues, we can always save). Each
/// operation that is transforming the new operations will match the base of the new operations at the time of
/// transformation. Finally, the operations will need to transform each other just like any other set of operations
/// made in a single frame/made based on the same version of the buffer.
///
/// # Undo (work in progress)
/// Undo should revert local changes only, leaving external changes in-tact, so that when all local changes are undone,
/// the buffer is in a state reflecting external changes only. This is complicated by the fact that external changes
/// may have been based on local changes that were synced to another client. To undo an operation that had an external
/// change based on it, we have to interpret the external change in the absence of local changes that were present when
/// it was created. This is the opposite of interpreting the external change in the presence of local changes that were
/// not present when it was created i.e. the normal flow of merging external changes. Here, we are removing a local
/// operation from the middle of the chain of operations that led to the current state of the buffer.
///
/// To do this, we perform the dance of transforming operations in reverse, taking a chain of operations each based on
/// the prior and transforming them into a set of operations based on the same base as the operation to be undone. Then
/// we remove the operation to be undone and apply the remaining operations with the forward transformation flow.
///
/// Operations are not invertible i.e. you cannot construct an inverse operation that will perfectly cancel out the
/// effect of another operation regardless of the time of interpretation. For example, with a text replacement, you can
/// construct an inverse text replacement that replaces the new range with the original text, but when operations are
/// undone from the middle of the chain, it may affect the original text. The operation will be re-interpreted based on
/// a new state of the buffer at its time of application. The replaced text has no fixed value by design.
///
/// However, it is possible to undo the specific application of an operation in the context of the state of the buffer
/// when it was applied. We store information necessary to undo applied operations alongside the operations themselves
/// i.e. the text replaced in the application. When the operation is transformed for any reason, this undo information
/// is invalidated.
#[derive(Default)]
pub struct Buffer {
    /// Current contents of the buffer (what should be displayed in the editor). Todo: hide behind a read-only accessor
    pub current: Snapshot,

    /// Snapshot of the buffer at the earliest undoable state. Operations are compacted into this as they overflow the
    /// undo limit.
    base: Snapshot,

    /// Operations received by the buffer. Used for undo/redo and for merging external changes.
    ops: Ops,

    /// State for tracking out-of-editor changes
    external: External,
}

#[derive(Debug, Default)]
pub struct Snapshot {
    pub text: String,
    pub segs: UnicodeSegs,
    pub selection: (Grapheme, Grapheme),
    pub seq: usize,
}

impl Snapshot {
    fn apply_select(&mut self, range: (Grapheme, Grapheme)) -> Response {
        self.selection = range;
        Response::default()
    }

    fn apply_replace(&mut self, replace: &Replace) -> (Response, Graphemes) {
        let Replace { range, text } = replace;
        let byte_range = self.segs.range_to_byte(*range);

        // Capture pre-apply segs so `Graphemes::measure_replace` can compute
        // the buffer delta. It's the only construction path for an
        // OT-correct grapheme count — bypassing it (e.g.
        // `text.graphemes(true).count()`) would produce a `usize`, not a
        // `Graphemes`, so any caller of this function wouldn't compile.
        let old_segs = self.segs.clone();

        self.text
            .replace_range(byte_range.start().0..byte_range.end().0, text);
        self.segs = unicode_segs::calc(&self.text);

        let actual_len = Graphemes::measure_replace(&old_segs, &self.segs, *range);

        adjust_subsequent_range(*range, actual_len, false, &mut self.selection);

        (Response { text_updated: true, ..Default::default() }, actual_len)
    }

    /// Captures the inverse-relevant state from the buffer *before* an op is
    /// applied. Combined with the actual replacement length (only knowable
    /// post-apply) by `PartialInverse::finalize` to produce the full inverse.
    fn invert_pre(&self, op: &Operation) -> PartialInverse {
        let mut partial = PartialInverse { select: self.selection, replace: None };
        if let Operation::Replace(Replace { range, text: _ }) = op {
            let byte_range = self.segs.range_to_byte(*range);
            let replaced_text = self[byte_range].into();
            partial.replace = Some((range.start(), replaced_text));
        }
        partial
    }
}

struct PartialInverse {
    select: (Grapheme, Grapheme),
    /// (start, replaced_text) — the inverse range's end is `start +
    /// actual_len`, which `finalize` fills in once apply has measured the
    /// buffer delta.
    replace: Option<(Grapheme, String)>,
}

impl PartialInverse {
    fn finalize(self, actual_len: Graphemes) -> InverseOperation {
        InverseOperation {
            select: self.select,
            replace: self.replace.map(|(start, replaced_text)| Replace {
                range: (start, start + actual_len),
                text: replaced_text,
            }),
        }
    }
}

#[derive(Default)]
struct Ops {
    /// Operations that have been received by the buffer
    all: Vec<Operation>,
    meta: Vec<OpMeta>,

    /// Sequence number of the first unapplied operation. Operations starting with this one are queued for processing.
    processed_seq: usize,

    /// Operations that have been applied to the buffer and already transformed, in order of application. Each of these
    /// operations is based on the previous operation in this list, with the first based on the history base. Derived
    /// from other data and invalidated by some undo/redo flows.
    transformed: Vec<Operation>,

    /// Operations representing the inverse of the operations in `transformed_ops`, in order of application. Useful for
    /// undoing operations. The data model differs because an operation that replaces text containing the cursor needs
    /// two operations to revert the text and cursor. Derived from other data and invalidated by some undo/redo flows.
    transformed_inverted: Vec<InverseOperation>,

    /// Actual graphemes contributed by each `transformed` op once spliced
    /// into the buffer. The `Graphemes` newtype enforces this distinction
    /// at the type level — populating this field requires a value from
    /// `Graphemes::measure_replace`, not `text.graphemes(true).count()`,
    /// which would account for in-isolation counts only and miss seam fusion
    /// (Devanagari spacing marks, ZWJ sequences). Always 0 for
    /// `Operation::Select`.
    transformed_actual_len: Vec<Graphemes>,
}

impl Ops {
    fn len(&self) -> usize {
        self.all.len()
    }

    fn is_undo_checkpoint(&self, idx: usize) -> bool {
        // start and end of undo history are checkpoints
        if idx == 0 {
            return true;
        }
        if idx == self.len() {
            return true;
        }

        // events separated by enough time are checkpoints
        let meta = &self.meta[idx];
        let prev_meta = &self.meta[idx - 1];
        if meta.timestamp - prev_meta.timestamp > Duration::from_millis(500) {
            return true;
        }

        // immediately after a standalone selection is a checkpoint
        let mut prev_op_standalone = meta.base != prev_meta.base;
        if idx > 1 {
            let prev_prev_meta = &self.meta[idx - 2];
            prev_op_standalone &= prev_meta.base != prev_prev_meta.base;
        }
        let prev_op_selection = matches!(&self.all[idx - 1], Operation::Select(..));
        if prev_op_standalone && prev_op_selection {
            return true;
        }

        false
    }
}

#[derive(Default)]
struct External {
    /// Text last loaded into the editor. Used as a reference point for merging out-of-editor changes with in-editor
    /// changes, similar to a base in a 3-way merge. May be a state that never appears in the buffer's history.
    text: String,

    /// Index of the last external operation referenced when merging changes. May be ahead of current.seq if there has
    /// not been a call to `update()` (updates current.seq) since the last call to `reload()` (assigns new greatest seq
    /// to `external_seq`).
    seq: usize,
}

#[derive(Default)]
pub struct Response {
    pub text_updated: bool,
    pub open_camera: bool,
    /// Sequence range of operations applied this frame. Use
    /// `buffer.replacements_since(seq_before)` to get the edits.
    pub seq_before: usize,
    pub seq_after: usize,
}

impl std::ops::BitOrAssign for Response {
    fn bitor_assign(&mut self, other: Response) {
        self.text_updated |= other.text_updated;
        self.open_camera |= other.open_camera;
        // keep the earliest seq_before and latest seq_after
        if self.seq_before == self.seq_after {
            self.seq_before = other.seq_before;
        }
        self.seq_after = other.seq_after;
    }
}

/// Additional metadata tracked alongside operations internally.
#[derive(Clone, Debug)]
struct OpMeta {
    /// At what time was this operation applied? Affects undo units.
    pub timestamp: Instant,

    /// What version of the buffer was the modifier looking at when they made this operation? Used for operational
    /// transformation, both when applying multiple operations in one frame and when merging out-of-editor changes.
    /// The magic happens here.
    pub base: usize,
}

impl Buffer {
    /// Push a series of operations onto the buffer's input queue; operations will be undone/redone atomically. Useful
    /// for batches of internal operations produced from a single input event e.g. multi-line list identation.
    pub fn queue(&mut self, mut ops: Vec<Operation>) {
        let timestamp = Instant::now();
        let base = self.current.seq;

        // combine adjacent replacements
        let mut combined_ops = Vec::new();
        ops.sort_by_key(|op| match op {
            Operation::Select(range) | Operation::Replace(Replace { range, .. }) => range.start(),
        });
        for op in ops.into_iter() {
            match &op {
                Operation::Replace(Replace { range: op_range, text: op_text }) => {
                    if let Some(Operation::Replace(Replace {
                        range: last_op_range,
                        text: last_op_text,
                    })) = combined_ops.last_mut()
                    {
                        if last_op_range.end() == op_range.start() {
                            last_op_range.1 = op_range.1;
                            last_op_text.push_str(op_text);
                        } else {
                            combined_ops.push(op);
                        }
                    } else {
                        combined_ops.push(op);
                    }
                }
                Operation::Select(_) => combined_ops.push(op),
            }
        }

        self.ops
            .meta
            .extend(combined_ops.iter().map(|_| OpMeta { timestamp, base }));
        self.ops.all.extend(combined_ops);
    }

    /// Loads a new string into the buffer, merging out-of-editor changes made since last load with in-editor changes
    /// made since last load. The buffer's undo history is preserved; undo'ing will revert in-editor changes only.
    /// Exercising undo's may put the buffer in never-before-seen states and exercising all undo's will revert the
    /// buffer to the most recently loaded state (undo limit permitting).
    /// Note: undo behavior described here is aspirational and not yet implemented.
    pub fn reload(&mut self, text: String) {
        let timestamp = Instant::now();
        let base = self.external.seq;
        let ops = diff(&self.external.text, &text);

        self.ops
            .meta
            .extend(ops.iter().map(|_| OpMeta { timestamp, base }));
        self.ops.all.extend(ops.into_iter().map(Operation::Replace));

        self.external.text = text;
        self.external.seq = self.base.seq + self.ops.all.len();
    }

    /// Indicates to the buffer the changes that have been saved outside the editor. This will serve as the new base
    /// for merging external changes. The sequence number should be taken from `current.seq` of the buffer when the
    /// buffer's contents are read for saving.
    pub fn saved(&mut self, external_seq: usize, external_text: String) {
        self.external.text = external_text;
        self.external.seq = external_seq;
    }

    pub fn merge(mut self, external_text_a: String, external_text_b: String) -> String {
        let ops_a = diff(&self.external.text, &external_text_a);
        let ops_b = diff(&self.external.text, &external_text_b);

        let timestamp = Instant::now();
        let base = self.external.seq;
        self.ops
            .meta
            .extend(ops_a.iter().map(|_| OpMeta { timestamp, base }));
        self.ops
            .meta
            .extend(ops_b.iter().map(|_| OpMeta { timestamp, base }));

        self.ops
            .all
            .extend(ops_a.into_iter().map(Operation::Replace));
        self.ops
            .all
            .extend(ops_b.into_iter().map(Operation::Replace));

        self.update();
        self.current.text
    }

    /// Applies all operations in the buffer's input queue
    pub fn update(&mut self) -> Response {
        // clear redo stack
        //             v base        v current    v processed
        // ops before: |<- applied ->|<- undone ->|<- queued ->|
        // ops after:  |<- applied ->|<- queued ->|
        let queue_len = self.base.seq + self.ops.len() - self.ops.processed_seq;
        if queue_len > 0 {
            let drain_range = self.current.seq..self.ops.processed_seq;
            self.ops.all.drain(drain_range.clone());
            self.ops.meta.drain(drain_range.clone());
            self.ops.transformed.drain(drain_range.clone());
            self.ops.transformed_inverted.drain(drain_range.clone());
            self.ops.transformed_actual_len.drain(drain_range.clone());
            self.ops.processed_seq = self.current.seq;
        } else {
            return Response::default();
        }

        // transform & apply
        let mut result = Response { seq_before: self.current.seq, ..Default::default() };
        for idx in self.current_idx()..self.current_idx() + queue_len {
            let mut op = self.ops.all[idx].clone();
            let meta = &self.ops.meta[idx];
            self.transform(&mut op, meta);
            // Capture inverse-relevant state before apply (it needs the
            // pre-apply text); finalize once redo's apply has measured the
            // actual contribution and stored it in `transformed_actual_len`.
            let partial_inverse = self.current.invert_pre(&op);
            self.ops.transformed.push(op.clone());
            self.ops.transformed_actual_len.push(Graphemes::default());
            self.ops.processed_seq += 1;

            result |= self.redo();

            let actual_len = *self.ops.transformed_actual_len.last().unwrap();
            self.ops
                .transformed_inverted
                .push(partial_inverse.finalize(actual_len));
        }

        result.seq_after = self.current.seq;
        result
    }

    fn transform(&self, op: &mut Operation, meta: &OpMeta) {
        let base_idx = meta.base - self.base.seq;
        for transforming_idx in base_idx..self.ops.processed_seq {
            let preceding_op = &self.ops.transformed[transforming_idx];
            let preceding_actual_len = self.ops.transformed_actual_len[transforming_idx];
            if let Operation::Replace(Replace {
                range: preceding_replaced_range,
                text: _preceding_replacement_text,
            }) = preceding_op
            {
                if let Operation::Replace(Replace { range: transformed_range, text }) = op {
                    if preceding_replaced_range.intersects(transformed_range, false)
                        && !(preceding_replaced_range.is_empty() && transformed_range.is_empty())
                    {
                        // concurrent replacements to intersecting ranges choose the first/local edit as the winner
                        // this doesn't create self-conflicts during merge because merge combines adjacent replacements
                        // this doesn't create self-conflicts for same-frame editor changes because our final condition
                        // is that we don't simultaneously insert text for both operations, which creates un-ideal
                        // behavior (see test buffer_merge_insert)
                        *text = "".into();
                        transformed_range.1 = transformed_range.0;
                    }
                }

                match op {
                    Operation::Replace(Replace { range: transformed_range, .. })
                    | Operation::Select(transformed_range) => {
                        adjust_subsequent_range(
                            *preceding_replaced_range,
                            preceding_actual_len,
                            true,
                            transformed_range,
                        );
                    }
                }
            }
        }
    }

    pub fn can_redo(&self) -> bool {
        self.current.seq < self.ops.processed_seq
    }

    pub fn can_undo(&self) -> bool {
        self.current.seq > self.base.seq
    }

    pub fn redo(&mut self) -> Response {
        let mut response = Response::default();
        while self.can_redo() {
            let idx = self.current_idx();
            // Clone so we can mutate `self` (storing actual_len) without
            // holding a borrow of `self.ops.transformed`.
            let op = self.ops.transformed[idx].clone();

            self.current.seq += 1;

            let actual_len = match &op {
                Operation::Replace(replace) => {
                    let (resp, len) = self.current.apply_replace(replace);
                    response |= resp;
                    len
                }
                Operation::Select(range) => {
                    response |= self.current.apply_select(*range);
                    Graphemes::default()
                }
            };
            self.ops.transformed_actual_len[idx] = actual_len;

            if self.ops.is_undo_checkpoint(self.current_idx()) {
                break;
            }
        }
        response
    }

    pub fn undo(&mut self) -> Response {
        let mut response = Response::default();
        while self.can_undo() {
            self.current.seq -= 1;
            // Clone so we can mutate `self.current` while reading the inverse.
            let op = self.ops.transformed_inverted[self.current_idx()].clone();

            if let Some(replace) = &op.replace {
                let (resp, _) = self.current.apply_replace(replace);
                response |= resp;
            }
            response |= self.current.apply_select(op.select);

            if self.ops.is_undo_checkpoint(self.current_idx()) {
                break;
            }
        }
        response
    }

    fn current_idx(&self) -> usize {
        self.current.seq - self.base.seq
    }

    /// Transforms a range through all replacements applied between
    /// `since_seq` and the current sequence. Returns `None` if the range
    /// intersected a replacement (i.e. the content it referred to was
    /// modified).
    pub fn transform_range(&self, since_seq: usize, range: &mut (Grapheme, Grapheme)) -> bool {
        let start = since_seq.saturating_sub(self.base.seq);
        let end = self.current_idx();
        for (i, op) in self.ops.transformed[start..end].iter().enumerate() {
            if let Operation::Replace(replace) = op {
                if range.intersects(&replace.range, true)
                    && !(range.is_empty() && replace.range.is_empty())
                {
                    return false;
                }
                let replacement_len = self.ops.transformed_actual_len[start + i];
                adjust_subsequent_range(replace.range, replacement_len, false, range);
            }
        }
        true
    }

    /// Reports whether the buffer's current text is empty.
    pub fn is_empty(&self) -> bool {
        self.current.text.is_empty()
    }

    pub fn selection_text(&self) -> String {
        self[self.current.selection].to_string()
    }
}

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        let mut result = Self::default();
        result.current.text = value.to_string();
        result.current.segs = unicode_segs::calc(value);
        result.external.text = value.to_string();
        result
    }
}

/// Adjust a range based on a text replacement. Positions before the replacement generally are not adjusted,
/// positions after the replacement generally are, and positions within the replacement are adjusted to the end of
/// the replacement if `prefer_advance` is true or are adjusted to the start of the replacement otherwise.
pub fn adjust_subsequent_range(
    replaced_range: (Grapheme, Grapheme), replacement_len: Graphemes, prefer_advance: bool,
    range: &mut (Grapheme, Grapheme),
) {
    for position in [&mut range.0, &mut range.1] {
        adjust_subsequent_position(replaced_range, replacement_len, prefer_advance, position);
    }
}

/// Adjust a position based on a text replacement. Positions before the replacement generally are not adjusted,
/// positions after the replacement generally are, and positions within the replacement are adjusted to the end of
/// the replacement if `prefer_advance` is true or are adjusted to the start of the replacement otherwise.
fn adjust_subsequent_position(
    replaced_range: (Grapheme, Grapheme), replacement_len: Graphemes, prefer_advance: bool,
    position: &mut Grapheme,
) {
    let replaced_len = replaced_range.len();
    let replacement_start = replaced_range.start();
    let replacement_end = replacement_start + replacement_len;

    enum Mode {
        Insert,
        Replace,
    }
    let mode = if replaced_range.is_empty() { Mode::Insert } else { Mode::Replace };

    let sorted_bounds = {
        let mut bounds = vec![replaced_range.start(), replaced_range.end(), *position];
        bounds.sort();
        bounds
    };
    let bind = |start: &Grapheme, end: &Grapheme, pos: &Grapheme| {
        start == &replaced_range.start() && end == &replaced_range.end() && pos == &*position
    };

    *position = match (mode, &sorted_bounds[..]) {
        // case 1: position at point of text insertion
        //                       text before replacement: * *
        //                        range of replaced text:  |
        //          range of subsequent cursor selection:  |
        //                        text after replacement: * X *
        // advance:
        // adjusted range of subsequent cursor selection:    |
        // don't advance:
        // adjusted range of subsequent cursor selection:  |
        (Mode::Insert, [start, end, pos]) if bind(start, end, pos) && end == pos => {
            if prefer_advance {
                replacement_end
            } else {
                replacement_start
            }
        }

        // case 2: position at start of text replacement
        //                       text before replacement: * * * *
        //                        range of replaced text:  |<->|
        //          range of subsequent cursor selection:  |
        //                        text after replacement: * X *
        // adjusted range of subsequent cursor selection:  |
        (Mode::Replace, [start, pos, end]) if bind(start, end, pos) && start == pos => {
            if prefer_advance {
                replacement_end
            } else {
                replacement_start
            }
        }

        // case 3: position at end of text replacement
        //                       text before replacement: * * * *
        //                        range of replaced text:  |<->|
        //          range of subsequent cursor selection:      |
        //                        text after replacement: * X *
        // adjusted range of subsequent cursor selection:    |
        (Mode::Replace, [start, end, pos]) if bind(start, end, pos) && end == pos => {
            if prefer_advance {
                replacement_end
            } else {
                replacement_start
            }
        }

        // case 4: position before point/start of text insertion/replacement
        //                       text before replacement: * * * * *
        //       (possibly empty) range of replaced text:    |<->|
        //          range of subsequent cursor selection:  |
        //                        text after replacement: * * X *
        // adjusted range of subsequent cursor selection:  |
        (_, [pos, start, end]) if bind(start, end, pos) => *position,

        // case 5: position within text replacement
        //                       text before replacement: * * * *
        //                        range of replaced text:  |<->|
        //          range of subsequent cursor selection:    |
        //                        text after replacement: * X *
        // advance:
        // adjusted range of subsequent cursor selection:    |
        // don't advance:
        // adjusted range of subsequent cursor selection:  |
        (Mode::Replace, [start, pos, end]) if bind(start, end, pos) => {
            if prefer_advance {
                replacement_end
            } else {
                replacement_start
            }
        }

        // case 6: position after point/end of text insertion/replacement
        //                       text before replacement: * * * * *
        //       (possibly empty) range of replaced text:  |<->|
        //          range of subsequent cursor selection:        |
        //                        text after replacement: * X * *
        // adjusted range of subsequent cursor selection:      |
        (_, [start, end, pos]) if bind(start, end, pos) => {
            *position + replacement_len - replaced_len
        }
        _ => unreachable!(),
    }
}

impl Index<(Byte, Byte)> for Snapshot {
    type Output = str;

    fn index(&self, index: (Byte, Byte)) -> &Self::Output {
        &self.text[index.start().0..index.end().0]
    }
}

impl Index<(Grapheme, Grapheme)> for Snapshot {
    type Output = str;

    fn index(&self, index: (Grapheme, Grapheme)) -> &Self::Output {
        let index = self.segs.range_to_byte(index);
        &self.text[index.start().0..index.end().0]
    }
}

impl Index<(Byte, Byte)> for Buffer {
    type Output = str;

    fn index(&self, index: (Byte, Byte)) -> &Self::Output {
        &self.current[index]
    }
}

impl Index<(Grapheme, Grapheme)> for Buffer {
    type Output = str;

    fn index(&self, index: (Grapheme, Grapheme)) -> &Self::Output {
        &self.current[index]
    }
}

#[cfg(test)]
mod test {
    use super::Buffer;
    use crate::model::text::offset_types::{Grapheme, RangeExt as _};
    use crate::model::text::operation_types::{Operation, Replace};
    use unicode_segmentation::UnicodeSegmentation;

    fn type_into_selection(buffer: &mut Buffer, text: &str) {
        let range = buffer.current.selection;
        buffer.queue(vec![
            Operation::Replace(Replace { range, text: text.into() }),
            Operation::Select((range.start(), range.start())),
        ]);
        buffer.update();
    }

    #[test]
    fn type_into_forward_selection() {
        let mut buffer = Buffer::from("hello");
        buffer.current.selection = (Grapheme(0), Grapheme(5));
        type_into_selection(&mut buffer, "X");
        assert_eq!(buffer.current.text, "X");
        assert_eq!(buffer.current.selection, (Grapheme(1), Grapheme(1)));
    }

    #[test]
    fn type_into_backward_selection() {
        let mut buffer = Buffer::from("hello");
        buffer.current.selection = (Grapheme(5), Grapheme(0));
        type_into_selection(&mut buffer, "X");
        assert_eq!(buffer.current.text, "X");
        assert_eq!(buffer.current.selection, (Grapheme(1), Grapheme(1)));
    }

    #[test]
    fn buffer_merge_nonintersecting_replace() {
        let base_content = "base content base";
        let local_content = "local content base";
        let remote_content = "base content remote";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "local content remote"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "local content remote"
        );
    }

    #[test]
    fn buffer_merge_prefix_replace() {
        let base_content = "base content";
        let local_content = "local content";
        let remote_content = "remote content";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "local content"
        );
    }

    #[test]
    fn buffer_merge_infix_replace() {
        let base_content = "con base tent";
        let local_content = "con local tent";
        let remote_content = "con remote tent";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "con local tent"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "con remote tent"
        );
    }

    #[test]
    fn buffer_merge_postfix_replace() {
        let base_content = "content base";
        let local_content = "content local";
        let remote_content = "content remote";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "content local"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "content remote"
        );
    }

    #[test]
    fn buffer_merge_insert() {
        let base_content = "content";
        let local_content = "content local";
        let remote_content = "content remote";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "content local remote"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "content remote local"
        );
    }

    #[test]
    // this test case documents behavior moreso than asserting target state
    fn buffer_merge_insert_replace() {
        let base_content = "content";
        let local_content = "content local";
        let remote_content = "remote";

        assert_eq!(
            Buffer::from(base_content).merge(local_content.into(), remote_content.into()),
            "remote"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "remote local"
        );
    }

    #[test]
    // this test case used to crash `merge`
    fn buffer_merge_crash() {
        let base_content = "con tent";
        let local_content = "cont tent locallocal";
        let remote_content = "cont remote tent";

        let _ = Buffer::from(base_content).merge(local_content.into(), remote_content.into());
        let _ = Buffer::from(base_content).merge(remote_content.into(), local_content.into());
    }

    // ── Fuzz ──────────────────────────────────────────────────────────────

    use rand::prelude::*;

    /// Grapheme clusters for fuzz testing. Includes ASCII, multi-byte, multi-codepoint emoji
    /// (ZWJ sequences, skin tones, flags), and combining characters.
    const POOL: &[&str] = &[
        "a",
        "b",
        "z",
        " ",
        "\n",
        "\t",
        "é",
        "ñ",
        "ü",
        "日",
        "本",
        "語",
        "👋",
        "🎉",
        "🔥",
        "❤️",
        "👨‍👩‍👧‍👦",
        "🏳️‍🌈",
        "👍🏽",
        "🇺🇸",
        "🇯🇵",
        "e\u{0301}",
        "a\u{0308}", // combining sequences: é, ä
    ];

    /// Generate a random grapheme-level edit of a document. Picks uniformly from:
    /// - 0: Delete 1-5 consecutive graphemes at a random position
    /// - 1: Insert 1-5 random graphemes from POOL at a random position
    /// - 2: Replace 1-5 consecutive graphemes with 1-3 random graphemes from POOL
    /// - 3: Clear everything
    ///
    /// When the document is empty, cases 0/2/3 fall through to insert (the _ arm).
    fn random_edit(rng: &mut StdRng, doc: &str) -> String {
        let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(doc, true).collect();
        let len = graphemes.len();

        let mut g: Vec<String> = graphemes.iter().map(|s| s.to_string()).collect();

        match rng.gen_range(0..4) {
            0 if len > 0 => {
                let pos = rng.gen_range(0..len);
                let del = rng.gen_range(1..=(len - pos).min(5));
                g.drain(pos..pos + del);
            }
            1 => {
                let pos = rng.gen_range(0..=len);
                let n = rng.gen_range(1..=5);
                for j in 0..n {
                    g.insert(pos + j, POOL[rng.gen_range(0..POOL.len())].into());
                }
            }
            2 if len > 0 => {
                let pos = rng.gen_range(0..len);
                let del = rng.gen_range(1..=(len - pos).min(5));
                let ins: Vec<String> = (0..rng.gen_range(1..=3))
                    .map(|_| POOL[rng.gen_range(0..POOL.len())].into())
                    .collect();
                g.splice(pos..pos + del, ins);
            }
            3 if len > 0 => {
                g.clear();
            }
            _ => {
                let n = rng.gen_range(1..=5);
                for _ in 0..n {
                    g.push(POOL[rng.gen_range(0..POOL.len())].into());
                }
            }
        }
        g.concat()
    }

    #[test]
    fn buffer_merge_fuzz() {
        let mut rng = StdRng::seed_from_u64(42);
        let bases = ["hello world", "👨‍👩‍👧‍👦🇺🇸🔥", "café ñoño 日本語", ""];
        for _ in 0..10_000 {
            let base = bases[rng.gen_range(0..bases.len())];
            let a = random_edit(&mut rng, base);
            let b = random_edit(&mut rng, base);

            // must not panic
            let _ = Buffer::from(base).merge(a.clone(), b.clone());
            let _ = Buffer::from(base).merge(b, a);
        }
    }

    // ── Chain convergence ──

    /// Simulates a sync channel between two adjacent nodes. Holds the last-agreed-upon
    /// document text, which serves as the 3-way merge base. When two nodes sync, their
    /// documents are merged against this base, both nodes adopt the result, and the base
    /// advances. This mirrors how lockbook's sync works: each client keeps a base (last
    /// synced state) and merges local vs remote changes against it.
    struct SyncLink {
        base: String,
    }

    impl SyncLink {
        fn new(base: &str) -> Self {
            Self { base: base.into() }
        }

        fn sync(&mut self, left: &mut String, right: &mut String) {
            let merged = Buffer::from(self.base.as_str()).merge(left.clone(), right.clone());
            *left = merged.clone();
            *right = merged.clone();
            self.base = merged;
        }
    }

    /// Sync all adjacent pairs in both directions until the chain stabilizes.
    /// With N nodes and N-1 links, 2*N passes ensures changes propagate end-to-end.
    fn full_sync(nodes: &mut [String], links: &mut [SyncLink]) {
        for _ in 0..nodes.len() * 2 {
            for i in 0..links.len() {
                let (left, right) = nodes.split_at_mut(i + 1);
                links[i].sync(&mut left[i], &mut right[0]);
            }
            for i in (0..links.len()).rev() {
                let (left, right) = nodes.split_at_mut(i + 1);
                links[i].sync(&mut left[i], &mut right[0]);
            }
        }
    }

    fn partial_sync(nodes: &mut [String], links: &mut [SyncLink], rng: &mut StdRng) {
        for _ in 0..3 {
            for i in 0..links.len() {
                if rng.gen_bool(0.5) {
                    let (left, right) = nodes.split_at_mut(i + 1);
                    links[i].sync(&mut left[i], &mut right[0]);
                }
            }
        }
    }

    fn assert_converged(nodes: &[String]) {
        for (i, node) in nodes.iter().enumerate().skip(1) {
            assert_eq!(
                &nodes[0], node,
                "node 0 and node {} diverged: {:?} vs {:?}",
                i, nodes[0], node
            );
        }
    }

    #[test]
    fn buffer_merge_fuzz_chain_2() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..10_000 {
            let init = if rng.gen_bool(0.5) { "hello 👋🏽" } else { "" };
            let mut nodes: Vec<String> = (0..2).map(|_| init.into()).collect();
            let mut links: Vec<SyncLink> = (0..1).map(|_| SyncLink::new(init)).collect();

            for _ in 0..rng.gen_range(1..=4) {
                for _ in 0..rng.gen_range(1..=3) {
                    let i = rng.gen_range(0..2);
                    nodes[i] = random_edit(&mut rng, &nodes[i]);
                }
                if rng.gen_bool(0.5) {
                    partial_sync(&mut nodes, &mut links, &mut rng);
                }
            }

            full_sync(&mut nodes, &mut links);
            assert_converged(&nodes);
        }
    }

    #[test]
    fn buffer_merge_fuzz_chain_5() {
        let mut rng = StdRng::seed_from_u64(77);
        for _ in 0..5_000 {
            let init = if rng.gen_bool(0.5) { "café 日本語 🇯🇵" } else { "abc" };
            let mut nodes: Vec<String> = (0..5).map(|_| init.into()).collect();
            let mut links: Vec<SyncLink> = (0..4).map(|_| SyncLink::new(init)).collect();

            for _ in 0..rng.gen_range(1..=3) {
                for _ in 0..rng.gen_range(1..=5) {
                    let i = rng.gen_range(0..5);
                    nodes[i] = random_edit(&mut rng, &nodes[i]);
                }
                if rng.gen_bool(0.5) {
                    partial_sync(&mut nodes, &mut links, &mut rng);
                }
            }

            full_sync(&mut nodes, &mut links);
            assert_converged(&nodes);
        }
    }
}
