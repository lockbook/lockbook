use crate::tab::markdown_editor;
use markdown_editor::appearance::Appearance;
use markdown_editor::debug::DebugInfo;
use markdown_editor::input::cursor::CursorState;
use markdown_editor::input::merge::merge;
use markdown_editor::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelCharOffset};
use markdown_editor::unicode_segs;
use markdown_editor::unicode_segs::UnicodeSegs;
use std::collections::VecDeque;
use std::iter;
use std::ops::{Index, Range};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

static MAX_UNDOS: usize = 100; // todo: make this much larger and measure performance impact

/// don't type text for this long, and the text before and after are considered separate undo events
static UNDO_DEBOUNCE_PERIOD: Duration = Duration::from_millis(300);

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
    /// to determine which operations are outstanding. Must be adjusted using `history_seq` to account for operations
    /// being compacted into the history base.
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
    history_ops: Vec<(Operation, OpMeta)>,

    // State for tracking out-of-editor changes
    /// Text last loaded into the editor. Used as a reference point for merging out-of-editor changes with in-editor
    /// changes, similar to a base in a 3-way merge. May be a state that never appears in the buffer's history.
    external_text: String,
    external_segs: UnicodeSegs,
    external_seq: usize,

    // Undo/redo metadata
    /// Inclusive/exclusive ranges of operations in `history_ops` submitted as atomic. We never undo/redo part of one
    /// of these as a matter of upholding our interface. All operations in history are in exactly one undo atom.
    undo_atoms: Vec<(usize, usize)>,

    /// Inclusive/exclusive ranges of operations in `history_ops` grouped into units for a more user-friendly undo/redo
    /// experience. We undo/redo all operations in a unit at once. Undo unit boundaries never split undo atoms. This
    /// considers the time of events e.g. so that rapidly typing a small amount of text can be undone with one undo.
    undo_units: Vec<(usize, usize)>,

    /// Operations representing undos that have been prepared for performance.
    undo_stack: Vec<Operation>,

    /// Operations that have been undone, in order of undo. Operations are moved from `ops` to this when undone. While
    /// being moved, they are 'inverted' so that to redo them you simply apply them as they appear in this stack.
    redo_stack: Vec<Operation>,
}

/// Buffer operation optimized for simplicity. Used in buffer's interface and internals to represent a building block
/// of text manipulation with support for undo/redo and collaborative editing.
#[derive(Debug)]
pub enum Operation {
    Replace { range: (DocCharOffset, DocCharOffset), text: String },
    Select { range: (DocCharOffset, DocCharOffset) },
}

/// Additional metadata tracked alongside operations internally.
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
        self.history_ops
            .extend(ops.into_iter().map(|op| (op, OpMeta { timestamp, base })));
    }

    /// Loads a new string into the buffer, merging out-of-editor changes made since last load with in-editor changes
    /// made since last load. The buffer's undo history is preserved; undo'ing will revert in-editor changes only.
    /// Exercising undo's may put the buffer in never-before-seen states and exercising all undo's will revert the
    /// buffer to the most recently loaded state (undo limit permitting).
    pub fn reload(&mut self, text: String) {
        let timestamp = Instant::now();
        let base = self.current_seq;
        let ops = merge(&self.external_text, &self.current_text, &text);
        self.history_ops
            .extend(ops.into_iter().map(|op| (op, OpMeta { timestamp, base })));

        self.external_text = text;
        self.external_segs = unicode_segs::calc(&self.external_text);
        self.external_seq = self.current_seq;
    }

    /// Indicates to the buffer that the current text has been saved. This is necessary to track out-of-editor changes.
    pub fn saved(&mut self) {
        self.external_text = self.current_text.clone();
        self.external_segs = self.current_segs.clone();
        self.external_seq = self.current_seq;
    }

    /// Apply all operations in the buffer's input queue. Returns a (text_updated, selection_updated) pair.
    pub fn update(&mut self) -> (bool, bool) {
        todo!()
    }

    /// Undo the most recent operation. Returns true if there was an operation to undo.
    pub fn undo(&mut self) -> bool {
        todo!()
    }

    /// Reports whether there are any operations to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Redo the most recently undone operation. Returns true if there was an operation to redo.
    pub fn redo(&mut self) -> bool {
        todo!()
    }

    /// Reports whether there are any operations to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
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

// // todo: lazy af name
// #[derive(Clone, Debug)]
// pub struct SubBuffer {
//     pub cursor: CursorState,
//     pub text: String,
//     pub segs: UnicodeSegs,
// }

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        Self {
            current_text: value.to_string(),
            current_segs: unicode_segs::calc(value),
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

// impl From<&str> for SubBuffer {
//     fn from(value: &str) -> Self {
//         Self { text: value.into(), cursor: 0.into(), segs: unicode_segs::calc(value) }
//     }
// }

// impl SubBuffer {
//     pub fn is_empty(&self) -> bool {
//         self.text.is_empty()
//     }

//     fn apply_modification(
//         &mut self, mut mods: Mutation, debug: &mut DebugInfo, appearance: &mut Appearance,
//     ) -> (bool, Option<String>, Option<String>) {
//         let mut text_updated = false;
//         let mut to_clipboard = None;
//         let mut opened_url = None;

//         let mut cur_cursor = self.cursor;
//         mods.reverse();
//         while let Some(modification) = mods.pop() {
//             // todo: reduce duplication
//             match modification {
//                 SubMutation::Cursor { cursor: cur } => {
//                     cur_cursor = cur;
//                 }
//                 SubMutation::Insert { text: text_replacement, advance_cursor } => {
//                     let replaced_text_range = cur_cursor.selection_or_position();

//                     Self::modify_subsequent_cursors(
//                         replaced_text_range.clone(),
//                         &text_replacement,
//                         advance_cursor,
//                         &mut mods,
//                         &mut cur_cursor,
//                     );

//                     self.replace_range(replaced_text_range, &text_replacement);
//                     self.segs = unicode_segs::calc(&self.text);
//                     text_updated = true;
//                 }
//                 SubMutation::Delete(n_chars) => {
//                     let text_replacement = "";
//                     let replaced_text_range = cur_cursor.selection().unwrap_or(Range {
//                         start: cur_cursor.selection.1 - n_chars,
//                         end: cur_cursor.selection.1,
//                     });

//                     Self::modify_subsequent_cursors(
//                         replaced_text_range.clone(),
//                         text_replacement,
//                         false,
//                         &mut mods,
//                         &mut cur_cursor,
//                     );

//                     self.replace_range(replaced_text_range, text_replacement);
//                     self.segs = unicode_segs::calc(&self.text);
//                     text_updated = true;
//                 }
//                 SubMutation::DebugToggle => {
//                     debug.draw_enabled = !debug.draw_enabled;
//                 }
//                 SubMutation::SetBaseFontSize(size) => {
//                     appearance.base_font_size = Some(size);
//                 }
//                 SubMutation::ToClipboard { text } => {
//                     to_clipboard = Some(text);
//                 }
//                 SubMutation::OpenedUrl { url } => {
//                     opened_url = Some(url);
//                 }
//             }
//         }

//         self.cursor = cur_cursor;
//         (text_updated, to_clipboard, opened_url)
//     }

//     fn modify_subsequent_cursors(
//         replaced_text_range: Range<DocCharOffset>, text_replacement: &str, advance_cursor: bool,
//         mods: &mut [SubMutation], cur_cursor: &mut CursorState,
//     ) {
//         let text_replacement_len = text_replacement.grapheme_indices(true).count();

//         for mod_cursor in mods
//             .iter_mut()
//             .filter_map(|modification| {
//                 if let SubMutation::Cursor { cursor: cur } = modification {
//                     Some(cur)
//                 } else {
//                     None
//                 }
//             })
//             .chain(iter::once(cur_cursor))
//         {
//             Self::adjust_subsequent_range(
//                 (replaced_text_range.start, replaced_text_range.end),
//                 text_replacement_len.into(),
//                 advance_cursor,
//                 Some(&mut mod_cursor.selection),
//             );
//             Self::adjust_subsequent_range(
//                 (replaced_text_range.start, replaced_text_range.end),
//                 text_replacement_len.into(),
//                 advance_cursor,
//                 mod_cursor.mark.as_mut(),
//             );
//             Self::adjust_subsequent_range(
//                 (replaced_text_range.start, replaced_text_range.end),
//                 text_replacement_len.into(),
//                 advance_cursor,
//                 mod_cursor.mark_highlight.as_mut(),
//             );
//         }
//     }

/// Adjust a range based on a text replacement. Positions before the replacement generally are not adjusted,
/// positions after the replacement generally are, and positions within the replacement are adjusted to the end of
/// the replacement if `prefer_advance` is true or are adjusted to the start of the replacement otherwise.
pub fn adjust_subsequent_range(
    replaced_range: (DocCharOffset, DocCharOffset), replacement_len: RelCharOffset,
    prefer_advance: bool, maybe_range: Option<&mut (DocCharOffset, DocCharOffset)>,
) {
    if let Some(range) = maybe_range {
        for position in [&mut range.0, &mut range.1] {
            adjust_subsequent_position(replaced_range, replacement_len, prefer_advance, position);
        }
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
        &self.current_text[index.0 .0..index.1 .0]
    }
}

impl Index<(DocCharOffset, DocCharOffset)> for Buffer {
    type Output = str;

    fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        let index = self.current_segs.range_to_byte(index);
        &self.current_text[index.0 .0..index.1 .0]
    }
}
