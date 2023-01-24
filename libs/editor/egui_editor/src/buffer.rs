use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::offset_types::{DocCharOffset, RelCharOffset};
use crate::unicode_segs;
use crate::unicode_segs::UnicodeSegs;
use queues::{CircularBuffer, IsQueue};
use std::iter;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

static MAX_UNDOS: usize = 5; // todo: make this much larger and measure performance impact

/// represents a modification made as a result of event processing
pub type Modification = Vec<SubModification>; // todo: tinyvec candidate

#[derive(Clone, Debug)]
pub enum SubModification {
    Cursor { cursor: Cursor },    // modify the cursor state
    Insert { text: String },      // insert text at cursor location
    Delete(RelCharOffset),        // delete selection or characters before cursor
    DebugToggle,                  // toggle debug overlay
    ToClipboard { text: String }, // cut or copy text to clipboard
}

#[derive(Debug)]
pub struct Buffer {
    /// contents of the buffer, as they appear in the editor
    pub current: SubBuffer,

    /// contents of the buffer MAX_UNDOs modifications ago; after exercising all undo's, the buffer would be this
    pub undo_base: SubBuffer,

    /// modifications made between undo_base and raw;
    pub modification_queue: CircularBuffer<Modification>,

    /// modifications reverted by undo and available for redo; used as a stack
    pub modifications_undone: Vec<Modification>,
}

// todo: lazy af name
#[derive(Debug)]
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
            modification_queue: CircularBuffer::new(MAX_UNDOS),
            modifications_undone: Vec::new(),
        }
    }
}

impl Buffer {
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    pub fn len(&self) -> usize {
        self.current.len()
    }

    /// applies `modification` and returns a boolean representing whether text was updated and optionally new contents for clipboard
    pub fn apply(
        &mut self, modification: Modification, debug: &mut DebugInfo,
    ) -> (bool, Option<String>) {
        // todo: less cloning of modifications
        if let Ok(Some(undo_modification)) = self.modification_queue.add(modification.clone()) {
            // when modifications overflow the queue, apply them to undo_base
            self.undo_base.apply_modification(undo_modification, debug);
        }
        self.current.apply_modification(modification, debug)
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

    pub fn len(&self) -> usize {
        self.text.len()
    }

    fn apply_modification(
        &mut self, mut modifications: Modification, debug: &mut DebugInfo,
    ) -> (bool, Option<String>) {
        let mut text_updated = false;
        let mut to_clipboard = None;

        let mut cur_cursor = self.cursor;
        modifications.reverse();
        while let Some(modification) = modifications.pop() {
            // todo: reduce duplication
            match modification {
                SubModification::Cursor { cursor: cur } => {
                    cur_cursor = cur;
                }
                SubModification::Insert { text: text_replacement } => {
                    let replaced_text_range = cur_cursor
                        .selection()
                        .unwrap_or(Range { start: cur_cursor.pos, end: cur_cursor.pos });

                    Self::modify_subsequent_cursors(
                        replaced_text_range.clone(),
                        &text_replacement,
                        &mut modifications,
                        &mut cur_cursor,
                    );

                    self.replace_range(replaced_text_range, &text_replacement);
                    self.segs = unicode_segs::calc(&self.text);
                    text_updated = true;
                }
                SubModification::Delete(n_chars) => {
                    let text_replacement = "";
                    let replaced_text_range = cur_cursor.selection().unwrap_or(Range {
                        start: if cur_cursor.pos.0 == 0 {
                            DocCharOffset(0)
                        } else {
                            cur_cursor.pos - n_chars
                        },
                        end: cur_cursor.pos,
                    });

                    Self::modify_subsequent_cursors(
                        replaced_text_range.clone(),
                        text_replacement,
                        &mut modifications,
                        &mut cur_cursor,
                    );

                    self.replace_range(replaced_text_range, text_replacement);
                    self.segs = unicode_segs::calc(&self.text);
                    text_updated = true;
                }
                SubModification::DebugToggle => {
                    debug.draw_enabled = !debug.draw_enabled;
                }
                SubModification::ToClipboard { text } => {
                    to_clipboard = Some(text);
                }
            }
        }

        self.cursor = cur_cursor;
        (text_updated, to_clipboard)
    }

    fn modify_subsequent_cursors(
        replaced_text_range: Range<DocCharOffset>, text_replacement: &str,
        modifications: &mut [SubModification], cur_cursor: &mut Cursor,
    ) {
        let replaced_text_len = replaced_text_range.end - replaced_text_range.start;
        let text_replacement_len =
            UnicodeSegmentation::grapheme_indices(text_replacement, true).count();

        for mod_cursor in modifications
            .iter_mut()
            .filter_map(|modification| {
                if let SubModification::Cursor { cursor: cur } = modification {
                    Some(cur)
                } else {
                    None
                }
            })
            .chain(iter::once(cur_cursor))
        {
            // adjust subsequent cursor selections; no part of a cursor shall appear inside
            // text that was not rendered when the cursor was placed (though a selection may
            // contain it).
            let cur_selection = mod_cursor
                .selection()
                .unwrap_or(Range { start: mod_cursor.pos, end: mod_cursor.pos });

            match (
                cur_selection.start < replaced_text_range.start,
                cur_selection.end < replaced_text_range.end,
            ) {
                _ if cur_selection.start >= replaced_text_range.end => {
                    // case 1:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<->|
                    //          range of subsequent cursor selection:        |<->|
                    //                        text after replacement: * X * * * *
                    // adjusted range of subsequent cursor selection:      |<->|
                    mod_cursor.pos = mod_cursor.pos + text_replacement_len - replaced_text_len;
                    if let Some(selection_origin) = mod_cursor.selection_origin {
                        mod_cursor.selection_origin =
                            Some(selection_origin + text_replacement_len - replaced_text_len);
                    }
                }
                _ if cur_selection.end < replaced_text_range.start => {
                    // case 2:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:        |<->|
                    //          range of subsequent cursor selection:  |<->|
                    //                        text after replacement: * * * * X *
                    // adjusted range of subsequent cursor selection:  |<->|
                    continue;
                }
                (false, false) => {
                    // case 3:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<--->|
                    //          range of subsequent cursor selection:      |<--->|
                    //                        text after replacement: * X * * *
                    // adjusted range of subsequent cursor selection:    |<->|
                    if let Some(selection_origin) = mod_cursor.selection_origin {
                        if mod_cursor.pos < selection_origin {
                            mod_cursor.pos =
                                replaced_text_range.end + text_replacement_len - replaced_text_len;
                            mod_cursor.selection_origin =
                                Some(selection_origin + text_replacement_len - replaced_text_len);
                        } else {
                            mod_cursor.selection_origin = Some(
                                replaced_text_range.end + text_replacement_len - replaced_text_len,
                            );
                            mod_cursor.pos =
                                mod_cursor.pos + text_replacement_len - replaced_text_len;
                        }
                    } else {
                        panic!("this code should be unreachable")
                    }
                }
                (true, true) => {
                    // case 4:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:      |<--->|
                    //          range of subsequent cursor selection:  |<--->|
                    //                        text after replacement: * * * X *
                    // adjusted range of subsequent cursor selection:  |<->|
                    if let Some(selection_origin) = mod_cursor.selection_origin {
                        if mod_cursor.pos < selection_origin {
                            mod_cursor.selection_origin = Some(replaced_text_range.start);
                        } else {
                            mod_cursor.pos = replaced_text_range.start;
                        }
                    } else {
                        panic!("this code should be unreachable")
                    }
                }
                (false, true) => {
                    // case 5:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:  |<------->|
                    //          range of subsequent cursor selection:    |<--->|
                    //                        text after replacement: * X *
                    // adjusted range of subsequent cursor selection:    |
                    mod_cursor.pos =
                        replaced_text_range.end + text_replacement_len - replaced_text_len;
                    mod_cursor.selection_origin = Some(mod_cursor.pos);
                }
                (true, false) => {
                    // case 6:
                    //                       text before replacement: * * * * * * *
                    //                        range of replaced text:    |<--->|
                    //          range of subsequent cursor selection:  |<------->|
                    //                        text after replacement: * * X * *
                    // adjusted range of subsequent cursor selection:  |<--->|
                    if let Some(selection_origin) = mod_cursor.selection_origin {
                        if mod_cursor.pos < selection_origin {
                            mod_cursor.selection_origin =
                                Some(selection_origin + text_replacement_len - replaced_text_len);
                        } else {
                            mod_cursor.pos =
                                mod_cursor.pos + text_replacement_len - replaced_text_len;
                        }
                    } else {
                        panic!("this code should be unreachable")
                    }
                }
            }
        }
    }

    pub fn replace_range(&mut self, range: Range<DocCharOffset>, replacement: &str) {
        self.text.replace_range(
            Range {
                start: self.segs.char_offset_to_byte(range.start).0,
                end: self.segs.char_offset_to_byte(range.end).0,
            },
            replacement,
        );
    }
}

#[cfg(test)]
mod test {
    use crate::buffer::{SubBuffer, SubModification};
    use crate::cursor::Cursor;

    #[test]
    fn apply_modifications_none_empty_doc() {
        let mut buffer: SubBuffer = "".into();
        buffer.cursor = Default::default();
        let mut debug = Default::default();

        let modifications = Default::default();

        let (text_updated, _) = buffer.apply_modification(modifications, &mut debug);

        assert_eq!(buffer.text, "");
        assert_eq!(buffer.cursor, Default::default());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_modifications_none() {
        let mut buffer: SubBuffer = "document content".into();
        buffer.cursor = 9.into();
        let mut debug = Default::default();

        let modifications = Default::default();

        let (text_updated, _) = buffer.apply_modification(modifications, &mut debug);

        assert_eq!(buffer.text, "document content");
        assert_eq!(buffer.cursor, 9.into());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_modifications_insert() {
        let mut buffer: SubBuffer = "document content".into();
        buffer.cursor = 9.into();
        let mut debug = Default::default();

        let modifications = vec![SubModification::Insert { text: "new ".to_string() }];

        let (text_updated, _) = buffer.apply_modification(modifications, &mut debug);

        assert_eq!(buffer.text, "document new content");
        assert_eq!(buffer.cursor, 13.into());
        assert!(!debug.draw_enabled);
        assert!(text_updated);
    }

    #[test]
    fn apply_modifications_selection_insert_twice() {
        struct Case {
            cursor_a: (usize, usize),
            cursor_b: (usize, usize),
            expected_buffer: &'static str,
            expected_cursor: (usize, usize),
        }

        let cases = [
            Case {
                cursor_a: (1, 3),
                cursor_b: (4, 6),
                expected_buffer: "1a4b7",
                expected_cursor: (4, 4),
            },
            Case {
                cursor_a: (4, 6),
                cursor_b: (1, 3),
                expected_buffer: "1b4a7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 5),
                cursor_b: (2, 6),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
            Case {
                cursor_a: (2, 6),
                cursor_b: (1, 5),
                expected_buffer: "1ba7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 6),
                cursor_b: (2, 5),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
            Case {
                cursor_a: (2, 5),
                cursor_b: (1, 6),
                expected_buffer: "1b7",
                expected_cursor: (2, 2),
            },
            Case {
                cursor_a: (1, 6),
                cursor_b: (1, 1),
                expected_buffer: "1ab7",
                expected_cursor: (3, 3),
            },
        ];

        for case in cases {
            let mut buffer: SubBuffer = "1234567".into();
            buffer.cursor = Cursor {
                pos: case.cursor_a.0.into(),
                selection_origin: Some(case.cursor_a.1.into()),
                ..Default::default()
            };
            let mut debug = Default::default();

            let modifications = vec![
                SubModification::Insert { text: "a".to_string() },
                SubModification::Cursor {
                    cursor: Cursor {
                        pos: case.cursor_b.0.into(),
                        selection_origin: Some(case.cursor_b.1.into()),
                        ..Default::default()
                    },
                },
                SubModification::Insert { text: "b".to_string() },
            ];

            let (text_updated, _) = buffer.apply_modification(modifications, &mut debug);

            assert_eq!(buffer.text, case.expected_buffer);
            assert_eq!(buffer.cursor.pos.0, case.expected_cursor.0);
            assert_eq!(buffer.cursor.selection_origin, Some(case.expected_cursor.1.into()));
            assert!(!debug.draw_enabled);
            assert!(text_updated);
        }
    }
}
