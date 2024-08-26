use crate::tab::markdown_editor;
use markdown_editor::input::merge::patch_ops;
use markdown_editor::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelCharOffset};
use markdown_editor::unicode_segs;
use markdown_editor::unicode_segs::UnicodeSegs;
use std::ops::Index;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

static MAX_UNDOS: usize = 100; // todo: make this much larger and measure performance impact

/// don't type text for this long, and the text before and after are considered separate undo events
static UNDO_DEBOUNCE_PERIOD: Duration = Duration::from_millis(300);

/// Buffer operation optimized for simplicity. Used in buffer's interface and internals to represent a building block
/// of text manipulation with support for undo/redo and collaborative editing.
///
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
/// # Undo
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
#[derive(Clone, Debug)]
pub enum Operation {
    Replace(Replacement),
    Select((DocCharOffset, DocCharOffset)),
}

/// Represents the inverse of an operation in a particular application. Includes selection and optional replacement
/// because replacing text also affects the selection in ways that are not reversible based on the replacement alone.
#[derive(Clone, Debug)]
pub struct InverseOperation {
    replace: Option<Replacement>,
    select: (DocCharOffset, DocCharOffset),
}

#[derive(Clone, Debug)]
pub struct Replacement {
    pub range: (DocCharOffset, DocCharOffset),
    pub text: String,
}

/// Long-lived state of the editor's text buffer.
#[derive(Default)]
pub struct Buffer {
    // Current contents of the buffer (what should be displayed in the editor)
    /// Current text content of the buffer
    pub current_text: String,

    /// Current unicode segments of the buffer - the buffer takes care to keep this up-to-date with `text`. Assists
    /// with translation between byte offsets and character offsets.
    pub current_segs: UnicodeSegs,

    /// Selected text. When selection is empty, elements are equal. First element represents start
    /// of selection and second element represents end of selection, which is the primary cursor
    /// position - elements are not ordered by value.
    pub current_selection: (DocCharOffset, DocCharOffset),

    /// Index of the most recent operation in `history_ops` that has been applied to the current buffer contents. Used
    /// to determine which operations are outstanding. Externally: use this to facilitate external changes.
    // Subtract `history_seq` for the index in `history_ops` of the next operation.
    pub current_seq: usize,

    // History of the buffer
    /// Content of buffer at the earliest undoable state. Operations are compacted into this as they overflow the undo
    /// limit.
    history_base_text: String,
    history_base_segs: UnicodeSegs,
    history_base_selection: (DocCharOffset, DocCharOffset),

    /// Index of the earliest undoable operation in `history_ops` in the sense that operations have immutable indices.
    /// This represents the number of operations that have been compacted into the history base and serves as a
    /// translation factor so that indexes don't need to be updated in all the places they're tracked. If undo history
    /// was unlimited, this would always be zero.
    history_seq: usize,

    /// Operations that have been applied to the buffer, in order of application. This plus the history base is
    /// sufficient information to reproduce the state of the buffer at any point in the undo history (but not enough
    /// to navigate it effectively).
    history_ops: Vec<Operation>,
    history_meta: Vec<OpMeta>,

    // Derived data for undo/redo (invalidated by undo'ing operations from the middle of the chain)
    /// Operations that have been applied to the buffer and already transformed, in order of application. Each of these
    /// operations is based on the previous operation in this list, with the first based on the history base.
    transformed_ops: Vec<Operation>,

    /// Operations representing the inverse of the operations in `transformed_ops`, in order of application. Useful for
    /// undoing operations. The data model differs because an operation that replaces text containing the cursor needs
    /// two operations to revert the text and cursor.
    inverse_transformed_ops: Vec<InverseOperation>,

    // State for tracking out-of-editor changes
    /// Text last loaded into the editor. Used as a reference point for merging out-of-editor changes with in-editor
    /// changes, similar to a base in a 3-way merge. May be a state that never appears in the buffer's history.
    external_text: String,

    /// Index of the last external operation referenced when merging changes. May be ahead of current_seq if there has
    /// not been a call to `update()` (updates current_seq) since the last call to `reload()` (assigns new greatest seq
    /// to `external_seq`).
    external_seq: usize,
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
    pub fn queue(&mut self, ops: Vec<Operation>) {
        let timestamp = Instant::now();
        let base = self.current_seq;

        self.history_meta
            .extend(ops.iter().map(|_| OpMeta { timestamp, base }));
        self.history_ops.extend(ops);
    }

    /// Loads a new string into the buffer, merging out-of-editor changes made since last load with in-editor changes
    /// made since last load. The buffer's undo history is preserved; undo'ing will revert in-editor changes only.
    /// Exercising undo's may put the buffer in never-before-seen states and exercising all undo's will revert the
    /// buffer to the most recently loaded state (undo limit permitting).
    pub fn reload(&mut self, text: String) {
        let timestamp = Instant::now();
        let base = self.external_seq;
        let ops = patch_ops(&self.external_text, &text);

        println!(
            "buffer: reloaded to produce ops {:?} with base {:?}",
            self.history_ops.len()..(self.history_ops.len() + ops.len()),
            base
        );

        self.history_meta
            .extend(ops.iter().map(|_| OpMeta { timestamp, base }));
        self.history_ops.extend(ops);

        self.external_text = text;
        self.external_seq = self.history_ops.len();
    }

    /// Indicates to the buffer the changes that have been saved outside the editor. This will serve as the new base
    /// for merging external changes. The sequence number should be taken from `current_seq` of the buffer when the
    /// buffer's contents are read for saving.
    pub fn saved(&mut self, external_seq: usize, external_text: String) {
        println!("buffer: saved {} ({:?})", external_seq, external_text);
        self.external_text = external_text;
        self.external_seq = external_seq;
    }

    /// Apply all operations in the buffer's input queue. Returns a (text_updated, selection_updated) pair.
    pub fn update(&mut self) -> (bool, bool) {
        if self.current_seq == self.history_ops.len() {
            return (false, false);
        }

        println!("buffer: updating");
        // this print statement causes performance issues once the history reaches a few hundred events
        // println!(
        //     "\tapplied_ops ({:?}): {:?}",
        //     self.current_seq,
        //     &self.history_ops[0..self.current_seq]
        // );
        println!(
            "\tqueued_ops ({:?}): {:?}",
            self.history_ops.len() - self.current_seq,
            &self.history_ops[self.current_seq..self.history_ops.len()]
        );

        let mut text_updated = false;
        let mut selection_updated = false;

        // iterate queue
        let min_queued_idx = self.current_seq - self.history_seq;
        for transformed_op_idx in min_queued_idx..self.history_ops.len() {
            // transform based on history
            let mut transformed_op = self.history_ops[transformed_op_idx].clone();
            let queued_meta = &self.history_meta[transformed_op_idx];
            let transformed_op_base_idx = queued_meta.base - self.history_seq;

            println!(
                "buffer: op {:?} to be transformed by {:?} (inc/exc)",
                transformed_op_idx,
                transformed_op_base_idx..transformed_op_idx
            );

            for preceding_idx in transformed_op_base_idx..transformed_op_idx {
                let preceding_op = &self.history_ops[preceding_idx];
                if let Operation::Replace(Replacement {
                    range: preceding_replaced_range,
                    text: preceding_replacement_text,
                }) = preceding_op
                {
                    println!(
                        "buffer: transforming queued op {:?} ({:?}) with preceding op {:?} ({:?} -> {:?})",
                        transformed_op_idx,
                        transformed_op,
                        preceding_idx,
                        preceding_replaced_range,
                        preceding_replacement_text
                    );

                    match &mut transformed_op {
                        Operation::Replace(Replacement { range: transformed_range, .. })
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

            // apply
            let mut inverse_transformed_op =
                InverseOperation { replace: None, select: self.current_selection };
            match transformed_op {
                Operation::Replace(Replacement { range, ref text }) => {
                    let byte_range = self.current_segs.range_to_byte(range);

                    // record inverse of transformed operation
                    let replaced_text = self[byte_range].into();
                    let replacement_range = (range.start(), range.start() + text.len());
                    inverse_transformed_op.replace =
                        Some(Replacement { range: replacement_range, text: replaced_text });

                    // update buffer and dependent data
                    self.current_text
                        .replace_range(byte_range.start().0..byte_range.end().0, text);
                    self.current_segs = unicode_segs::calc(&self.current_text);
                    adjust_subsequent_range(
                        range,
                        text.graphemes(true).count().into(),
                        false,
                        &mut self.current_selection,
                    );

                    // change detection
                    text_updated = true;
                    selection_updated = true;
                }
                Operation::Select(range) => {
                    self.current_selection = range;
                    selection_updated = true;
                }
            }

            // bookkeeping
            self.transformed_ops.push(transformed_op);
            self.inverse_transformed_ops.push(inverse_transformed_op);

            self.current_seq = transformed_op_idx + 1;
            println!("buffer: current seq = {:?}", self.current_seq);
        }

        #[cfg(debug_assertions)]
        assert_eq!(self.current_seq, self.history_seq + self.history_ops.len());

        (text_updated, selection_updated)
    }

    /// Undo the most recent operation. Returns true if there was an operation to undo.
    pub fn undo(&mut self) -> bool {
        if self.can_undo() {
            true
        } else {
            false
        }
    }

    /// Reports whether there are any operations to undo.
    pub fn can_undo(&self) -> bool {
        todo!()
    }

    /// Redo the most recently undone operation. Returns true if there was an operation to redo.
    pub fn redo(&mut self) -> bool {
        if self.can_redo() {
            todo!();
            true
        } else {
            false
        }
    }

    /// Reports whether there are any operations to redo.
    pub fn can_redo(&self) -> bool {
        todo!()
    }

    /// Reports whether the buffer's current text is empty.
    pub fn is_empty(&self) -> bool {
        self.current_text.is_empty()
    }

    pub fn selection_text(&self) -> String {
        self[self.current_selection].to_string()
    }
}

// #[derive(Debug)]
// pub struct OldBuffer {
//     /// contents of the buffer, as they appear in the editor
//     pub current: SubBuffer,

//     /// contents of the buffer MAX_UNDOs modifications ago; after exercising all undo's, the buffer would be this
//     pub undo_base: SubBuffer,

//     /// modifications made between undo_base and raw;
//     pub undo_queue: VecDeque<Vec<Mutation>>,

//     /// additional, most recent element for queue, contains at most one text update, flushed to queue when another text update is applied
//     pub current_text_mods: Option<Vec<Mutation>>,

//     /// modifications reverted by undo and available for redo; used as a stack
//     pub redo_stack: Vec<Vec<Mutation>>,

//     // instant of last modification application
//     pub last_apply: Instant,
// }

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        Self {
            current_text: value.to_string(),
            current_segs: unicode_segs::calc(value),
            external_text: value.to_string(),
            ..Default::default()
        }
    }
}

// impl OldBuffer {
//     pub fn is_empty(&self) -> bool {
//         self.current.is_empty()
//     }

//     /// applies `modification` and returns a boolean representing whether text was updated, new contents for clipboard
//     /// (optional), and a link that was opened (optional)
//     // todo: less cloning
//     pub fn apply(
//         &mut self, modification: Mutation, debug: &mut DebugInfo, appearance: &mut Appearance,
//     ) -> (bool, Option<String>, Option<String>) {
//         let now = Instant::now();

//         // accumulate modifications into one modification until a non-cursor update is applied (for purposes of undo)
//         if modification
//             .iter()
//             .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
//         {
//             if let Some(ref mut current_text_mods) = self.current_text_mods {
//                 // extend current modification until new cursor placement
//                 if current_text_mods.iter().any(|modification| {
//                     !modification
//                         .iter()
//                         .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
//                 }) || now - self.last_apply > UNDO_DEBOUNCE_PERIOD
//                 {
//                     self.undo_queue.push_back(current_text_mods.clone());
//                     if self.undo_queue.len() > MAX_UNDOS {
//                         // when modifications overflow the queue, apply them to undo_base
//                         if let Some(undo_mods) = self.undo_queue.pop_front() {
//                             for m in undo_mods {
//                                 self.undo_base.apply_modification(m, debug, appearance);
//                             }
//                         }
//                     }
//                     self.current_text_mods = Some(vec![modification.clone()]);
//                 } else {
//                     current_text_mods.push(modification.clone());
//                 }
//             } else {
//                 self.current_text_mods = Some(vec![modification.clone()]);
//             }
//         } else if let Some(ref mut current_text_mods) = self.current_text_mods {
//             current_text_mods.push(modification.clone());
//         } else {
//             self.current_text_mods = Some(vec![modification.clone()]);
//         }

//         self.last_apply = now;
//         self.redo_stack = Vec::new();
//         self.current
//             .apply_modification(modification, debug, appearance)
//     }

//     /// undoes one modification, if able
//     pub fn undo(&mut self, debug: &mut DebugInfo, appearance: &mut Appearance) {
//         if let Some(current_text_mods) = &self.current_text_mods {
//             // don't undo cursor-only updates
//             if !current_text_mods.iter().any(|modification| {
//                 modification
//                     .iter()
//                     .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
//             }) {
//                 for m in current_text_mods {
//                     self.current
//                         .apply_modification(m.clone(), debug, appearance);
//                 }
//                 return;
//             }

//             // reconstruct the modification queue
//             let mut mods_to_apply = VecDeque::new();
//             std::mem::swap(&mut mods_to_apply, &mut self.undo_queue);

//             // current starts over from undo base
//             self.current = self.undo_base.clone();

//             // move the (undone) current modification to the redo stack
//             self.redo_stack.push(current_text_mods.clone());

//             // undo the current modification by applying the whole queue but not the current modification
//             while let Some(mods) = mods_to_apply.pop_front() {
//                 for m in &mods {
//                     self.current
//                         .apply_modification(m.clone(), debug, appearance);
//                 }

//                 // final element of the queue moved from queue to current
//                 if mods_to_apply.is_empty() {
//                     self.current_text_mods = Some(mods);
//                     break;
//                 }

//                 self.undo_queue.push_back(mods);
//             }
//         }
//     }

//     /// redoes one modification, if able
//     pub fn redo(&mut self, debug: &mut DebugInfo, appearance: &mut Appearance) {
//         if let Some(mods) = self.redo_stack.pop() {
//             if let Some(current_text_mods) = &self.current_text_mods {
//                 self.undo_queue.push_back(current_text_mods.clone());
//             }
//             self.current_text_mods = Some(mods.clone());
//             for m in mods {
//                 self.current.apply_modification(m, debug, appearance);
//             }
//         }
//     }
// }

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

// fn replace_range(&mut self, range: Range<DocCharOffset>, replacement: &str) {
//     self.text.replace_range(
//         Range {
//             start: self.segs.offset_to_byte(range.start).0,
//             end: self.segs.offset_to_byte(range.end).0,
//         },
//         replacement,
//     );
// }
// }

// impl Index<(DocByteOffset, DocByteOffset)> for SubBuffer {
//     type Output = str;

//     fn index(&self, index: (DocByteOffset, DocByteOffset)) -> &Self::Output {
//         &self.text[index.0 .0..index.1 .0]
//     }
// }

// impl Index<(DocCharOffset, DocCharOffset)> for SubBuffer {
//     type Output = str;

//     fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
//         let index = self.segs.range_to_byte(index);
//         &self.text[index.0 .0..index.1 .0]
//     }
// }

impl Index<(DocByteOffset, DocByteOffset)> for Buffer {
    type Output = str;

    fn index(&self, index: (DocByteOffset, DocByteOffset)) -> &Self::Output {
        &self.current_text[index.start().0..index.end().0]
    }
}

impl Index<(DocCharOffset, DocCharOffset)> for Buffer {
    type Output = str;

    fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        let index = self.current_segs.range_to_byte(index);
        &self.current_text[index.start().0..index.end().0]
    }
}
