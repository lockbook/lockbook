use crate::debug::DebugInfo;
use crate::input::cursor::Cursor;
use crate::offset_types::{DocByteOffset, DocCharOffset, RangeExt, RelCharOffset};
use crate::unicode_segs;
use crate::unicode_segs::UnicodeSegs;
use std::collections::VecDeque;
use std::iter;
use std::ops::{Index, Range};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

static MAX_UNDOS: usize = 100; // todo: make this much larger and measure performance impact

/// don't type text for this long, and the text before and after are considered separate undo events
static UNDO_DEBOUNCE_PERIOD: Duration = Duration::from_millis(300);

/// represents a modification made as a result of event processing
pub type Mutation = Vec<SubMutation>; // todo: tinyvec candidate

#[derive(Clone, Debug)]
pub enum EditorMutation {
    Buffer(Mutation), // todo: tinyvec candidate
    Undo,
    Redo,
    // todo: redefine
    // SetCursor { cursor: (DocCharOffset, DocCharOffset), marked: bool }, // set the cursor
    // Replace { text: String }, // replace the current selection
}

#[derive(Clone, Debug)]
pub enum SubMutation {
    Cursor { cursor: Cursor },    // modify the cursor state
    Insert { text: String },      // insert text at cursor location
    Delete(RelCharOffset),        // delete selection or characters before cursor
    DebugToggle,                  // toggle debug overlay
    ToClipboard { text: String }, // cut or copy text to clipboard
    OpenedUrl { url: String },    // open a url
}

#[derive(Debug)]
pub struct Buffer {
    /// contents of the buffer, as they appear in the editor
    pub current: SubBuffer,

    /// contents of the buffer MAX_UNDOs modifications ago; after exercising all undo's, the buffer would be this
    pub undo_base: SubBuffer,

    /// modifications made between undo_base and raw;
    pub undo_queue: VecDeque<Vec<Mutation>>,

    /// additional, most recent element for queue, contains at most one text update, flushed to queue when another text update is applied
    pub current_text_mods: Option<Vec<Mutation>>,

    /// modifications reverted by undo and available for redo; used as a stack
    pub redo_stack: Vec<Vec<Mutation>>,

    // instant of last modification application
    pub last_apply: Instant,
}

// todo: lazy af name
#[derive(Clone, Debug)]
pub struct SubBuffer {
    pub cursor: Cursor,
    pub text: String,
    pub segs: UnicodeSegs,
}

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        Self {
            current: value.into(),
            undo_base: value.into(),
            undo_queue: VecDeque::new(),
            current_text_mods: None,
            redo_stack: Vec::new(),
            last_apply: Instant::now(),
        }
    }
}

impl Buffer {
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// applies `modification` and returns a boolean representing whether text was updated, new contents for clipboard
    /// (optional), and a link that was opened (optional)
    // todo: less cloning
    pub fn apply(
        &mut self, modification: Mutation, debug: &mut DebugInfo,
    ) -> (bool, Option<String>, Option<String>) {
        let now = Instant::now();

        // accumulate modifications into one modification until a non-cursor update is applied (for purposes of undo)
        if modification
            .iter()
            .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
        {
            if let Some(ref mut current_text_mods) = self.current_text_mods {
                // extend current modification until new cursor placement
                if current_text_mods.iter().any(|modification| {
                    !modification
                        .iter()
                        .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
                }) || now - self.last_apply > UNDO_DEBOUNCE_PERIOD
                {
                    self.undo_queue.push_back(current_text_mods.clone());
                    if self.undo_queue.len() > MAX_UNDOS {
                        // when modifications overflow the queue, apply them to undo_base
                        if let Some(undo_mods) = self.undo_queue.pop_front() {
                            for m in undo_mods {
                                self.undo_base.apply_modification(m, debug);
                            }
                        }
                    }
                    self.current_text_mods = Some(vec![modification.clone()]);
                } else {
                    current_text_mods.push(modification.clone());
                }
            } else {
                self.current_text_mods = Some(vec![modification.clone()]);
            }
        } else if let Some(ref mut current_text_mods) = self.current_text_mods {
            current_text_mods.push(modification.clone());
        } else {
            self.current_text_mods = Some(vec![modification.clone()]);
        }

        self.last_apply = now;
        self.redo_stack = Vec::new();
        self.current.apply_modification(modification, debug)
    }

    /// undoes one modification, if able
    pub fn undo(&mut self, debug: &mut DebugInfo) {
        if let Some(current_text_mods) = &self.current_text_mods {
            // don't undo cursor-only updates
            if !current_text_mods.iter().any(|modification| {
                modification
                    .iter()
                    .any(|m| matches!(m, SubMutation::Insert { .. } | SubMutation::Delete(..)))
            }) {
                for m in current_text_mods {
                    self.current.apply_modification(m.clone(), debug);
                }
                return;
            }

            // reconstruct the modification queue
            let mut mods_to_apply = VecDeque::new();
            std::mem::swap(&mut mods_to_apply, &mut self.undo_queue);

            // current starts over from undo base
            self.current = self.undo_base.clone();

            // move the (undone) current modification to the redo stack
            self.redo_stack.push(current_text_mods.clone());

            // undo the current modification by applying the whole queue but not the current modification
            while let Some(mods) = mods_to_apply.pop_front() {
                for m in &mods {
                    self.current.apply_modification(m.clone(), debug);
                }

                // final element of the queue moved from queue to current
                if mods_to_apply.is_empty() {
                    self.current_text_mods = Some(mods);
                    break;
                }

                self.undo_queue.push_back(mods);
            }
        }
    }

    /// redoes one modification, if able
    pub fn redo(&mut self, debug: &mut DebugInfo) {
        if let Some(mods) = self.redo_stack.pop() {
            if let Some(current_text_mods) = &self.current_text_mods {
                self.undo_queue.push_back(current_text_mods.clone());
            }
            self.current_text_mods = Some(mods.clone());
            for m in mods {
                self.current.apply_modification(m, debug);
            }
        }
    }
}

impl From<&str> for SubBuffer {
    fn from(value: &str) -> Self {
        Self { text: value.into(), cursor: 0.into(), segs: unicode_segs::calc(value) }
    }
}

impl SubBuffer {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn apply_modification(
        &mut self, mut mods: Mutation, debug: &mut DebugInfo,
    ) -> (bool, Option<String>, Option<String>) {
        let mut text_updated = false;
        let mut to_clipboard = None;
        let mut opened_url = None;

        let mut cur_cursor = self.cursor;
        mods.reverse();
        while let Some(modification) = mods.pop() {
            // todo: reduce duplication
            match modification {
                SubMutation::Cursor { cursor: cur } => {
                    cur_cursor = cur;
                }
                SubMutation::Insert { text: text_replacement } => {
                    let replaced_text_range = cur_cursor.selection_or_position();

                    Self::modify_subsequent_cursors(
                        replaced_text_range.clone(),
                        &text_replacement,
                        &mut mods,
                        &mut cur_cursor,
                    );

                    self.replace_range(replaced_text_range, &text_replacement);
                    self.segs = unicode_segs::calc(&self.text);
                    text_updated = true;
                }
                SubMutation::Delete(n_chars) => {
                    let text_replacement = "";
                    let replaced_text_range = cur_cursor.selection().unwrap_or(Range {
                        start: cur_cursor.selection.1 - n_chars,
                        end: cur_cursor.selection.1,
                    });

                    Self::modify_subsequent_cursors(
                        replaced_text_range.clone(),
                        text_replacement,
                        &mut mods,
                        &mut cur_cursor,
                    );

                    self.replace_range(replaced_text_range, text_replacement);
                    self.segs = unicode_segs::calc(&self.text);
                    text_updated = true;
                }
                SubMutation::DebugToggle => {
                    debug.draw_enabled = !debug.draw_enabled;
                }
                SubMutation::ToClipboard { text } => {
                    to_clipboard = Some(text);
                }
                SubMutation::OpenedUrl { url } => {
                    opened_url = Some(url);
                }
            }
        }

        self.cursor = cur_cursor;
        (text_updated, to_clipboard, opened_url)
    }

    fn modify_subsequent_cursors(
        replaced_text_range: Range<DocCharOffset>, text_replacement: &str,
        mods: &mut [SubMutation], cur_cursor: &mut Cursor,
    ) {
        let text_replacement_len = text_replacement.grapheme_indices(true).count();

        for mod_cursor in mods
            .iter_mut()
            .filter_map(|modification| {
                if let SubMutation::Cursor { cursor: cur } = modification {
                    Some(cur)
                } else {
                    None
                }
            })
            .chain(iter::once(cur_cursor))
        {
            Self::modify_subsequent_range(
                (replaced_text_range.start, replaced_text_range.end),
                text_replacement_len.into(),
                Some(&mut mod_cursor.selection),
            );
            Self::modify_subsequent_range(
                (replaced_text_range.start, replaced_text_range.end),
                text_replacement_len.into(),
                mod_cursor.mark.as_mut(),
            );
            Self::modify_subsequent_range(
                (replaced_text_range.start, replaced_text_range.end),
                text_replacement_len.into(),
                mod_cursor.mark_highlight.as_mut(),
            );
        }
    }

    fn modify_subsequent_range(
        replaced_text_range: (DocCharOffset, DocCharOffset), text_replacement_len: RelCharOffset,
        maybe_range: Option<&mut (DocCharOffset, DocCharOffset)>,
    ) {
        if let Some(range) = maybe_range {
            let replaced_text_len = replaced_text_range.end() - replaced_text_range.start();

            // adjust subsequent cursor selections; no part of a cursor shall appear inside
            // text that was not rendered when the cursor was placed (though a selection may
            // contain it).
            match (
                range.start() < replaced_text_range.start(),
                range.end() < replaced_text_range.end(),
            ) {
                _ if range.start() >= replaced_text_range.end() => {
                    // case 1:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<->|
                    //          range of subsequent cursor selection:        |<->|
                    //                        text after replacement: * X * * * *
                    // adjusted range of subsequent cursor selection:      |<->|
                    range.1 = range.1 + text_replacement_len - replaced_text_len;
                    range.0 = range.0 + text_replacement_len - replaced_text_len;
                }
                _ if range.end() < replaced_text_range.start() => {
                    // case 2:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:        |<->|
                    //          range of subsequent cursor selection:  |<->|
                    //                        text after replacement: * * * * X *
                    // adjusted range of subsequent cursor selection:  |<->|
                }
                (false, false) => {
                    // case 3:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<--->|
                    //          range of subsequent cursor selection:      |<--->|
                    //                        text after replacement: * X * * *
                    // adjusted range of subsequent cursor selection:    |<->|
                    if range.1 < range.0 {
                        range.1 =
                            replaced_text_range.end() + text_replacement_len - replaced_text_len;
                        range.0 = range.0 + text_replacement_len - replaced_text_len;
                    } else {
                        range.0 =
                            replaced_text_range.end() + text_replacement_len - replaced_text_len;
                        range.1 = range.1 + text_replacement_len - replaced_text_len;
                    }
                }
                (true, true) => {
                    // case 4:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:      |<--->|
                    //          range of subsequent cursor selection:  |<--->|
                    //                        text after replacement: * * * X *
                    // adjusted range of subsequent cursor selection:  |<->|
                    if range.1 < range.0 {
                        range.0 = replaced_text_range.start();
                    } else {
                        range.1 = replaced_text_range.start();
                    }
                }
                (false, true) => {
                    // case 5:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<------->|
                    //          range of subsequent cursor selection:    |<--->|
                    //                        text after replacement: * X *
                    // adjusted range of subsequent cursor selection:    |
                    range.1 = replaced_text_range.end() + text_replacement_len - replaced_text_len;
                    range.0 = range.1;
                }
                (true, false) => {
                    // case 6:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:    |<--->|
                    //          range of subsequent cursor selection:  |<------->|
                    //                        text after replacement: * * X * *
                    // adjusted range of subsequent cursor selection:  |<--->|
                    if range.1 < range.0 {
                        range.0 = range.0 + text_replacement_len - replaced_text_len;
                    } else {
                        range.1 = range.1 + text_replacement_len - replaced_text_len;
                    }
                }
            }
        }
    }

    fn replace_range(&mut self, range: Range<DocCharOffset>, replacement: &str) {
        self.text.replace_range(
            Range {
                start: self.segs.offset_to_byte(range.start).0,
                end: self.segs.offset_to_byte(range.end).0,
            },
            replacement,
        );
    }
}

impl Index<(DocByteOffset, DocByteOffset)> for SubBuffer {
    type Output = str;

    fn index(&self, index: (DocByteOffset, DocByteOffset)) -> &Self::Output {
        &self.text[index.0 .0..index.1 .0]
    }
}

impl Index<(DocCharOffset, DocCharOffset)> for SubBuffer {
    type Output = str;

    fn index(&self, index: (DocCharOffset, DocCharOffset)) -> &Self::Output {
        let index = self.segs.range_to_byte(index);
        &self.text[index.0 .0..index.1 .0]
    }
}

#[cfg(test)]
mod test {
    use crate::buffer::{SubBuffer, SubMutation};
    use crate::input::cursor::Cursor;

    #[test]
    fn apply_mods_none_empty_doc() {
        let mut buffer: SubBuffer = "".into();
        buffer.cursor = Default::default();
        let mut debug = Default::default();

        let mods = Default::default();

        let (text_updated, _, _) = buffer.apply_modification(mods, &mut debug);

        assert_eq!(buffer.text, "");
        assert_eq!(buffer.cursor, Default::default());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_mods_none() {
        let mut buffer: SubBuffer = "document content".into();
        buffer.cursor = 9.into();
        let mut debug = Default::default();

        let mods = Default::default();

        let (text_updated, _, _) = buffer.apply_modification(mods, &mut debug);

        assert_eq!(buffer.text, "document content");
        assert_eq!(buffer.cursor, 9.into());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_mods_insert() {
        let mut buffer: SubBuffer = "document content".into();
        buffer.cursor = 9.into();
        let mut debug = Default::default();

        let mods = vec![SubMutation::Insert { text: "new ".to_string() }];

        let (text_updated, _, _) = buffer.apply_modification(mods, &mut debug);

        assert_eq!(buffer.text, "document new content");
        assert_eq!(buffer.cursor, 13.into());
        assert!(!debug.draw_enabled);
        assert!(text_updated);
    }

    #[test]
    fn apply_mods_selection_insert_twice() {
        struct Case {
            cursor_a: Cursor,
            cursor_b: Cursor,
            expected_buffer: &'static str,
            expected_cursor: (usize, usize),
        }

        let cases = [
            Case {
                cursor_a: (1, 3).into(),
                cursor_b: (4, 6).into(),
                expected_buffer: "1a4b7",
                expected_cursor: (4, 4),
            },
            Case {
                cursor_a: (4, 6).into(),
                cursor_b: (1, 3).into(),
                expected_buffer: "1b4a7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 5).into(),
                cursor_b: (2, 6).into(),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
            Case {
                cursor_a: (2, 6).into(),
                cursor_b: (1, 5).into(),
                expected_buffer: "1ba7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 6).into(),
                cursor_b: (2, 5).into(),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
            Case {
                cursor_a: (2, 5).into(),
                cursor_b: (1, 6).into(),
                expected_buffer: "1b7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 6).into(),
                cursor_b: (1, 1).into(),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
        ];

        for case in cases {
            let mut buffer: SubBuffer = "1234567".into();
            buffer.cursor = case.cursor_a;

            let mut debug = Default::default();

            let mods = vec![
                SubMutation::Insert { text: "a".to_string() },
                SubMutation::Cursor { cursor: case.cursor_b },
                SubMutation::Insert { text: "b".to_string() },
            ];

            let (text_updated, _, _) = buffer.apply_modification(mods, &mut debug);

            assert_eq!(buffer.text, case.expected_buffer);
            assert_eq!(buffer.cursor.selection.1 .0, case.expected_cursor.0);
            assert_eq!(buffer.cursor.selection.0, case.expected_cursor.1);
            assert!(!debug.draw_enabled);
            assert!(text_updated);
        }
    }
}
