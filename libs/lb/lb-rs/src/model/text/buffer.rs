use super::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelCharOffset};
use super::operation_types::{InverseOperation, Operation, Replace};
use super::unicode_segs::UnicodeSegs;
use super::{diff, unicode_segs};
use std::ops::Index;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

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
    pub selection: (DocCharOffset, DocCharOffset),
    pub seq: usize,
}

impl Snapshot {
    fn apply_select(&mut self, range: (DocCharOffset, DocCharOffset)) -> Response {
        self.selection = range;
        Response { text_updated: false }
    }

    fn apply_replace(&mut self, replace: &Replace) -> Response {
        let Replace { range, text } = replace;
        let byte_range = self.segs.range_to_byte(*range);

        self.text
            .replace_range(byte_range.start().0..byte_range.end().0, text);
        self.segs = unicode_segs::calc(&self.text);
        adjust_subsequent_range(
            *range,
            text.graphemes(true).count().into(),
            false,
            &mut self.selection,
        );

        Response { text_updated: true }
    }

    fn invert(&self, op: &Operation) -> InverseOperation {
        let mut inverse = InverseOperation { replace: None, select: self.selection };
        if let Operation::Replace(replace) = op {
            inverse.replace = Some(self.invert_replace(replace));
        }
        inverse
    }

    fn invert_replace(&self, replace: &Replace) -> Replace {
        let Replace { range, text } = replace;
        let byte_range = self.segs.range_to_byte(*range);
        let replaced_text = self[byte_range].into();
        let replacement_range = (range.start(), range.start() + text.graphemes(true).count());
        Replace { range: replacement_range, text: replaced_text }
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
    text_updated: bool,
}

impl std::ops::BitOrAssign for Response {
    fn bitor_assign(&mut self, other: Response) {
        self.text_updated |= other.text_updated;
    }
}

impl From<Response> for bool {
    fn from(value: Response) -> Self {
        value.text_updated
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
            self.ops.processed_seq = self.current.seq;
        } else {
            return Response::default();
        }

        // transform & apply
        let mut result = Response::default();
        for idx in self.current_idx()..self.current_idx() + queue_len {
            let mut op = self.ops.all[idx].clone();
            let meta = &self.ops.meta[idx];
            self.transform(&mut op, meta);
            self.ops.transformed_inverted.push(self.current.invert(&op));
            self.ops.transformed.push(op.clone());
            self.ops.processed_seq += 1;

            result |= self.redo();
        }

        result
    }

    fn transform(&self, op: &mut Operation, meta: &OpMeta) {
        let base_idx = meta.base - self.base.seq;
        for transforming_idx in base_idx..self.ops.processed_seq {
            let preceding_op = &self.ops.transformed[transforming_idx];
            if let Operation::Replace(Replace {
                range: preceding_replaced_range,
                text: preceding_replacement_text,
            }) = preceding_op
            {
                if let Operation::Replace(Replace { range: transformed_range, text }) = op {
                    if preceding_replaced_range.intersects(transformed_range, true)
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
                            preceding_replacement_text.graphemes(true).count().into(),
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
            let op = &self.ops.transformed[self.current_idx()];

            self.current.seq += 1;

            response |= match op {
                Operation::Replace(replace) => self.current.apply_replace(replace),
                Operation::Select(range) => self.current.apply_select(*range),
            };

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
            let op = &self.ops.transformed_inverted[self.current_idx()];

            if let Some(replace) = &op.replace {
                response |= self.current.apply_replace(replace);
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
    replaced_range: (DocCharOffset, DocCharOffset), replacement_len: RelCharOffset,
    prefer_advance: bool, range: &mut (DocCharOffset, DocCharOffset),
) {
    for position in [&mut range.0, &mut range.1] {
        adjust_subsequent_position(replaced_range, replacement_len, prefer_advance, position);
    }
}

/// Adjust a position based on a text replacement. Positions before the replacement generally are not adjusted,
/// positions after the replacement generally are, and positions within the replacement are adjusted to the end of
/// the replacement if `prefer_advance` is true or are adjusted to the start of the replacement otherwise.
fn adjust_subsequent_position(
    replaced_range: (DocCharOffset, DocCharOffset), replacement_len: RelCharOffset,
    prefer_advance: bool, position: &mut DocCharOffset,
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
    let bind = |start: &DocCharOffset, end: &DocCharOffset, pos: &DocCharOffset| {
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

impl Index<(DocByteOffset, DocByteOffset)> for Snapshot {
    type Output = str;

    fn index(&self, index: (DocByteOffset, DocByteOffset)) -> &Self::Output {
        &self.text[index.start().0..index.end().0]
    }
}

impl Index<(DocCharOffset, DocCharOffset)> for Snapshot {
    type Output = str;

    fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        let index = self.segs.range_to_byte(index);
        &self.text[index.start().0..index.end().0]
    }
}

impl Index<(DocByteOffset, DocByteOffset)> for Buffer {
    type Output = str;

    fn index(&self, index: (DocByteOffset, DocByteOffset)) -> &Self::Output {
        &self.current[index]
    }
}

impl Index<(DocCharOffset, DocCharOffset)> for Buffer {
    type Output = str;

    fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        &self.current[index]
    }
}

#[cfg(test)]
mod test {
    use super::Buffer;

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
            "content local"
        );
        assert_eq!(
            Buffer::from(base_content).merge(remote_content.into(), local_content.into()),
            "remote"
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
}
