use crate::buffer::Buffer;
use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::element::ItemType;
use crate::galleys::Galleys;
use crate::layouts::{Annotation, Layouts};
use crate::offset_types::{DocCharOffset, RelCharOffset};
use crate::unicode_segs;
use crate::unicode_segs::UnicodeSegs;
use egui::{Event, Key, PointerButton, Vec2};
use std::cmp::Ordering;
use std::iter;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// represents a modification made as a result of event processing
#[derive(Debug)]
enum Modification<'a> {
    Cursor { cursor: Cursor },    // modify the cursor state
    Insert { text: &'a str },     // insert text at cursor location
    InsertOwned { text: String }, // insert text at cursor location
    Delete(RelCharOffset),        // delete selection or characters before cursor
    DebugToggle,                  // toggle debug overlay
}

/// processes `events` and returns a boolean representing whether text was updated
pub fn process(
    events: &[Event], layouts: &Layouts, galleys: &Galleys, buffer: &mut Buffer,
    segs: &mut UnicodeSegs, cursor: &mut Cursor, debug: &mut DebugInfo,
) -> bool {
    let modifications = calc_modifications(events, segs, layouts, galleys, buffer, *cursor);
    apply_modifications(modifications, buffer, segs, cursor, debug)
}

fn apply_modifications(
    mut modifications: Vec<Modification>, buffer: &mut Buffer, segs: &mut UnicodeSegs,
    cursor: &mut Cursor, debug: &mut DebugInfo,
) -> bool {
    let mut text_updated = false;

    let mut cur_cursor = *cursor;
    modifications.reverse();
    while let Some(modification) = modifications.pop() {
        // todo: reduce duplication
        match modification {
            Modification::Cursor { cursor: cur } => {
                cur_cursor = cur;
            }
            Modification::Insert { text: text_replacement } => {
                let replaced_text_range = cur_cursor
                    .selection()
                    .unwrap_or(Range { start: cur_cursor.pos, end: cur_cursor.pos });

                modify_subsequent_cursors(
                    replaced_text_range.clone(),
                    text_replacement,
                    &mut modifications,
                    &mut cur_cursor,
                );

                buffer.replace_range(replaced_text_range, text_replacement, segs);
                *segs = unicode_segs::calc(buffer);
                text_updated = true;
            }
            Modification::InsertOwned { text: text_replacement } => {
                let replaced_text_range = cur_cursor
                    .selection()
                    .unwrap_or(Range { start: cur_cursor.pos, end: cur_cursor.pos });

                modify_subsequent_cursors(
                    replaced_text_range.clone(),
                    &text_replacement,
                    &mut modifications,
                    &mut cur_cursor,
                );

                buffer.replace_range(replaced_text_range, &text_replacement, segs);
                *segs = unicode_segs::calc(buffer);
                text_updated = true;
            }
            Modification::Delete(n_chars) => {
                let text_replacement = "";
                let replaced_text_range = cur_cursor.selection().unwrap_or(Range {
                    start: if cur_cursor.pos.0 == 0 {
                        DocCharOffset(0)
                    } else {
                        cur_cursor.pos - n_chars
                    },
                    end: cur_cursor.pos,
                });

                modify_subsequent_cursors(
                    replaced_text_range.clone(),
                    text_replacement,
                    &mut modifications,
                    &mut cur_cursor,
                );

                buffer.replace_range(replaced_text_range, text_replacement, segs);
                *segs = unicode_segs::calc(buffer);
                text_updated = true;
            }
            Modification::DebugToggle => {
                debug.draw_enabled = !debug.draw_enabled;
            }
        }
    }

    *cursor = cur_cursor;
    text_updated
}

fn modify_subsequent_cursors(
    replaced_text_range: Range<DocCharOffset>, text_replacement: &str,
    modifications: &mut [Modification], cur_cursor: &mut Cursor,
) {
    let replaced_text_len = replaced_text_range.end - replaced_text_range.start;
    let text_replacement_len =
        UnicodeSegmentation::grapheme_indices(text_replacement, true).count();

    for mod_cursor in modifications
        .iter_mut()
        .filter_map(|modification| {
            if let Modification::Cursor { cursor: cur } = modification {
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
                        mod_cursor.pos = mod_cursor.pos + text_replacement_len - replaced_text_len;
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
                mod_cursor.pos = replaced_text_range.end + text_replacement_len - replaced_text_len;
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
                        mod_cursor.pos = mod_cursor.pos + text_replacement_len - replaced_text_len;
                    }
                } else {
                    panic!("this code should be unreachable")
                }
            }
        }
    }
}

fn calc_modifications<'a>(
    events: &'a [Event], segs: &UnicodeSegs, layouts: &Layouts, galleys: &Galleys, buffer: &Buffer,
    cursor: Cursor,
) -> Vec<Modification<'a>> {
    let mut modifications = Vec::new();
    let mut previous_cursor = cursor;
    let mut cursor = cursor;

    cursor.fix(false, segs, galleys);
    if cursor != previous_cursor {
        modifications.push(Modification::Cursor { cursor });
        previous_cursor = cursor;
    }

    for event in events {
        let mut new_cursor_position = None;
        match event {
            Event::Key { key: Key::ArrowRight, pressed: true, modifiers } => {
                cursor.x_target = None;

                let (galley_idx, cur_cursor) =
                    galleys.galley_and_cursor_by_char_offset(cursor.pos, segs);
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.alt {
                    cursor.advance_word(false, buffer, segs, galleys);
                } else if modifiers.command {
                    let galley = &galleys[galley_idx];
                    let new_cursor = galley.galley.cursor_end_of_row(&cur_cursor);
                    cursor.pos =
                        galleys.char_offset_by_galley_and_cursor(galley_idx, &new_cursor, segs);
                } else {
                    cursor.advance_char(false, segs, galleys);
                }
            }
            Event::Key { key: Key::ArrowLeft, pressed: true, modifiers } => {
                cursor.x_target = None;

                let (galley_idx, cur_cursor) =
                    galleys.galley_and_cursor_by_char_offset(cursor.pos, segs);
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.alt {
                    cursor.advance_word(true, buffer, segs, galleys);
                } else if modifiers.command {
                    let galley = &galleys[galley_idx];
                    let new_cursor = galley.galley.cursor_begin_of_row(&cur_cursor);
                    cursor.pos =
                        galleys.char_offset_by_galley_and_cursor(galley_idx, &new_cursor, segs);
                } else {
                    cursor.advance_char(true, segs, galleys);
                }
            }
            Event::Key { key: Key::ArrowDown, pressed: true, modifiers } => {
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.command {
                    cursor.pos = segs.last_cursor_position();
                    cursor.fix(false, segs, galleys);
                    cursor.x_target = None;
                } else {
                    let (cur_galley_idx, cur_cursor) =
                        galleys.galley_and_cursor_by_char_offset(cursor.pos, segs);
                    let cur_galley = &galleys[cur_galley_idx];

                    // the first time we use an up or down arrow, remember the x we started at
                    let x_target = cursor.set_x_target(cur_galley, cur_cursor);

                    let at_bottom_of_cur_galley =
                        cur_cursor.rcursor.row == cur_galley.galley.rows.len() - 1;
                    let in_last_galley = cur_galley_idx == galleys.len() - 1;
                    let (mut new_cursor, new_galley_idx) =
                        if at_bottom_of_cur_galley && !in_last_galley {
                            // move to the first row of the next galley
                            let new_galley_idx = cur_galley_idx + 1;
                            let new_galley = &galleys[new_galley_idx];
                            let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                                x: 0.0, // overwritten below
                                y: 0.0, // top of new galley
                            });
                            (new_cursor, new_galley_idx)
                        } else {
                            // move down one row in the current galley
                            let new_cursor = cur_galley.galley.cursor_down_one_row(&cur_cursor);
                            (new_cursor, cur_galley_idx)
                        };

                    if !(at_bottom_of_cur_galley && in_last_galley) {
                        // move to the x_target in the new row/galley
                        new_cursor = Cursor::move_to_x_target(
                            &galleys[new_galley_idx],
                            new_cursor,
                            x_target,
                        );
                    } else {
                        // we moved to the end of the last line
                        cursor.x_target = None;
                    }

                    cursor.pos =
                        galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_cursor, segs);
                }
            }
            Event::Key { key: Key::ArrowUp, pressed: true, modifiers } => {
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.command {
                    cursor.pos = DocCharOffset(0);
                    cursor.fix(false, segs, galleys);
                    cursor.x_target = None;
                } else {
                    let (cur_galley_idx, cur_cursor) =
                        galleys.galley_and_cursor_by_char_offset(cursor.pos, segs);
                    let cur_galley = &galleys[cur_galley_idx];

                    // the first time we use an up or down arrow, remember the x we started at
                    let x_target = cursor.set_x_target(cur_galley, cur_cursor);

                    let at_top_of_cur_galley = cur_cursor.rcursor.row == 0;
                    let in_first_galley = cur_galley_idx == 0;
                    let (mut new_cursor, new_galley_idx) =
                        if at_top_of_cur_galley && !in_first_galley {
                            // move to the last row of the previous galley
                            let new_galley_idx = cur_galley_idx - 1;
                            let new_galley = &galleys[new_galley_idx];
                            let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                                x: 0.0,                          // overwritten below
                                y: new_galley.galley.rect.max.y, // bottom of new galley
                            });
                            (new_cursor, new_galley_idx)
                        } else {
                            // move up one row in the current galley
                            let new_cursor = cur_galley.galley.cursor_up_one_row(&cur_cursor);
                            (new_cursor, cur_galley_idx)
                        };

                    if !(at_top_of_cur_galley && in_first_galley) {
                        // move to the x_target in the new row/galley
                        new_cursor = Cursor::move_to_x_target(
                            &galleys[new_galley_idx],
                            new_cursor,
                            x_target,
                        );
                    } else {
                        // we moved to the start of the first line
                        cursor.x_target = None;
                    }

                    cursor.pos =
                        galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_cursor, segs);
                }
            }
            Event::Paste(text) | Event::Text(text) => {
                cursor.x_target = None;

                modifications.push(Modification::Insert { text });

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Backspace, pressed: true, modifiers: _modifiers } => {
                cursor.x_target = None;

                let layout_idx = layouts.layout_at_char(cursor.pos, segs);
                let layout = &layouts[layout_idx];
                if layout.head_size > 0
                    && segs.char_offset_to_byte(cursor.pos) == layout.range.start + layout.head_size
                    && cursor.selection().is_none()
                {
                    // delete layout head (e.g. bullet)
                    modifications.push(Modification::Delete(layout.head_size_chars(buffer)));

                    // if we deleted an item in a numbered list, decrement subsequent items
                    if let Some(Annotation::Item(ItemType::Numbered(_), indent_level)) =
                        layout.annotation
                    {
                        modifications.extend(increment_numbered_list_items(
                            layout_idx,
                            indent_level,
                            1,
                            true,
                            segs,
                            layouts,
                            buffer,
                            cursor,
                        ));
                    }
                } else {
                    // delete selected text or one character
                    modifications.push(Modification::Delete(1.into()));
                }

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Enter, pressed: true, modifiers: _ } => {
                cursor.x_target = None;

                modifications.push(Modification::Insert { text: "\n" });

                let layout_idx = layouts.layout_at_char(cursor.pos, segs);
                let layout = &layouts[layout_idx];
                if segs.char_offset_to_byte(cursor.pos) == layout.range.end - layout.tail_size {
                    if matches!(layout.annotation, Some(Annotation::Item(..))) {
                        // empty list item -> delete current annotation
                        if layout.size() - layout.head_size - layout.tail_size == 0 {
                            modifications.push(Modification::Cursor {
                                cursor: Cursor {
                                    pos: segs
                                        .byte_offset_to_char(layout.range.start + layout.head_size),
                                    selection_origin: Some(
                                        segs.byte_offset_to_char(layout.range.start),
                                    ),
                                    ..Default::default()
                                },
                            });
                            modifications.push(Modification::Delete(0.into()));
                            modifications.push(Modification::Cursor { cursor });
                            modifications.push(Modification::Insert { text: "\n" });
                        }
                    } else {
                        // nonempty list item -> insert new list item
                        match layout.annotation {
                            Some(Annotation::Item(ItemType::Bulleted, _)) => {
                                modifications.push(Modification::InsertOwned {
                                    text: layout.head(buffer).to_string(),
                                });
                            }
                            Some(Annotation::Item(
                                ItemType::Numbered(cur_number),
                                indent_level,
                            )) => {
                                let head = layout.head(buffer);
                                let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                                    .to_string()
                                    + &(cur_number + 1).to_string()
                                    + ". ";
                                modifications.push(Modification::InsertOwned { text });

                                modifications.extend(increment_numbered_list_items(
                                    layout_idx,
                                    indent_level,
                                    1,
                                    false,
                                    segs,
                                    layouts,
                                    buffer,
                                    cursor,
                                ));
                            }
                            Some(Annotation::Item(ItemType::Todo(_), _)) => {
                                let head = layout.head(buffer);
                                let text = head[0..head.len() - 4].to_string() + "- [ ]";
                                modifications.push(Modification::InsertOwned { text });
                            }
                            Some(Annotation::Image(_, _, _)) => {}
                            Some(Annotation::Rule) => {}
                            None => {}
                        }
                    }
                }

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Tab, pressed: true, modifiers } => {
                // if we're in a list item, tab/shift+tab will indent/de-indent
                // otherwise, tab will insert a tab and shift tab will do nothing
                let layout_idx = layouts.layout_at_char(cursor.pos, segs);
                let layout = &layouts[layout_idx];
                if let Some(annotation) = &layout.annotation {
                    match annotation {
                        Annotation::Item(item_type, indent_level) => {
                            // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                            let layout_text = &buffer.raw[layout.range.start.0..layout.range.end.0];
                            let indent_seq = if layout_text.starts_with('\t') {
                                "\t"
                            } else if layout_text.starts_with("    ") {
                                "    "
                            } else if layout_text.starts_with("  ") {
                                "  "
                            } else {
                                "\t"
                            };

                            // indent or de-indent if able
                            let new_indent_level = if modifiers.shift {
                                let mut can_deindent = true;
                                if *indent_level == 1 {
                                    can_deindent = false; // cannot de-indent un-indented list item
                                } else if layout_idx != layouts.len() - 1 {
                                    let next_layout = &layouts[layout_idx + 1];
                                    if let Some(Annotation::Item(
                                        next_item_type,
                                        next_indent_level,
                                    )) = &next_layout.annotation
                                    {
                                        if next_item_type == item_type
                                            && next_indent_level > indent_level
                                        {
                                            can_deindent = false; // list item cannot be de-indented if already indented less than next item
                                        }
                                    }
                                }

                                if can_deindent {
                                    // de-indentation: select text, delete selection, restore cursor
                                    modifications.push(Modification::Cursor {
                                        cursor: Cursor {
                                            pos: segs.byte_offset_to_char(
                                                layout.range.start + indent_seq.len(),
                                            ),
                                            selection_origin: Some(
                                                segs.byte_offset_to_char(layout.range.start),
                                            ),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(Modification::Delete(0.into()));
                                    modifications.push(Modification::Cursor { cursor });

                                    indent_level - 1
                                } else {
                                    *indent_level
                                }
                            } else {
                                let mut can_indent = true;
                                if layout_idx == 0 {
                                    can_indent = false; // first layout cannot be indented
                                }
                                let prior_layout = &layouts[layout_idx - 1];
                                if let Some(Annotation::Item(prior_item_type, prior_indent_level)) =
                                    &prior_layout.annotation
                                {
                                    if prior_item_type != item_type {
                                        can_indent = false; // first list item of a given type in a list cannot be indented
                                    }
                                    if prior_indent_level < indent_level {
                                        can_indent = false; // list item cannot be indented if already indented more than prior item
                                    }
                                } else {
                                    can_indent = false; // first list item of a list cannot be indented
                                }

                                if can_indent {
                                    // indentation: set cursor to galley start, insert indentation sequence, restore cursor
                                    modifications.push(Modification::Cursor {
                                        cursor: Cursor {
                                            pos: segs.byte_offset_to_char(layout.range.start),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(Modification::InsertOwned {
                                        text: indent_seq.to_string(),
                                    });
                                    modifications.push(Modification::Cursor { cursor });

                                    indent_level + 1
                                } else {
                                    *indent_level
                                }
                            };

                            // re-number numbered lists
                            if new_indent_level != *indent_level {
                                if let ItemType::Numbered(cur_number) = item_type {
                                    // assign a new_number to this item based on position in new nested list
                                    let new_number = {
                                        let mut new_number = 1;
                                        let mut prior_layout_idx = layout_idx;
                                        while prior_layout_idx > 0 {
                                            prior_layout_idx -= 1;
                                            let prior_layout = &layouts[prior_layout_idx];
                                            if let Some(Annotation::Item(
                                                ItemType::Numbered(prior_number),
                                                prior_indent_level,
                                            )) = prior_layout.annotation
                                            {
                                                match prior_indent_level.cmp(&new_indent_level) {
                                                    Ordering::Greater => {
                                                        continue; // skip more-nested list items
                                                    }
                                                    Ordering::Less => {
                                                        break; // our element is the first in its sublist
                                                    }
                                                    Ordering::Equal => {
                                                        new_number = prior_number + 1; // our element comes after this one in its sublist
                                                        break;
                                                    }
                                                }
                                            } else {
                                                break;
                                            }
                                        }
                                        new_number
                                    };

                                    // replace cur_number with new_number in head
                                    modifications.push(Modification::Cursor {
                                        cursor: Cursor {
                                            pos: segs.byte_offset_to_char(
                                                layout.range.start + layout.head_size
                                                    - (cur_number).to_string().len()
                                                    - 2,
                                            ),
                                            selection_origin: Some(segs.byte_offset_to_char(
                                                layout.range.start + layout.head_size,
                                            )),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(Modification::InsertOwned {
                                        text: new_number.to_string() + ". ",
                                    });
                                    modifications.push(Modification::Cursor { cursor });

                                    if modifiers.shift {
                                        // decrement numbers in old list by this item's old number
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            *indent_level,
                                            *cur_number,
                                            true,
                                            segs,
                                            layouts,
                                            buffer,
                                            cursor,
                                        ));

                                        // increment numbers in new nested list by one
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            new_indent_level,
                                            1,
                                            false,
                                            segs,
                                            layouts,
                                            buffer,
                                            cursor,
                                        ));
                                    } else {
                                        // decrement numbers in old list by one
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            *indent_level,
                                            1,
                                            true,
                                            segs,
                                            layouts,
                                            buffer,
                                            cursor,
                                        ));

                                        // increment numbers in new nested list by this item's new number
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            new_indent_level,
                                            new_number,
                                            false,
                                            segs,
                                            layouts,
                                            buffer,
                                            cursor,
                                        ));
                                    }
                                }
                            }
                        }
                        Annotation::Image(..) => {}
                        Annotation::Rule => {}
                    }
                } else if !modifiers.shift {
                    modifications.push(Modification::InsertOwned { text: "\t".to_string() });
                }
            }
            Event::Key { key: Key::A, pressed: true, modifiers } => {
                if modifiers.command {
                    cursor.selection_origin = Some(DocCharOffset(0));
                    cursor.pos = segs.last_cursor_position();
                }
            }
            Event::Key { key: Key::F2, pressed: true, modifiers: _modifiers } => {
                modifications.push(Modification::DebugToggle);
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            } => {
                if !modifiers.shift {
                    // click: end selection
                    cursor.selection_origin = None;
                } else {
                    // shift+click: begin selection
                    cursor.set_selection_origin();
                }
                // any click: begin drag; update cursor
                cursor.set_click_and_drag_origin();
                new_cursor_position = Some(pos);
            }
            Event::PointerMoved(pos) => {
                if cursor.click_and_drag_origin.is_some() {
                    // drag: begin selection; update cursor
                    cursor.set_selection_origin();
                    new_cursor_position = Some(pos);
                }
            }
            Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } => {
                // click released: end drag; don't update cursor
                cursor.click_and_drag_origin = None;
            }
            _ => {}
        }

        if let Some(&pos) = new_cursor_position {
            if !galleys.is_empty() {
                if pos.y < galleys[0].ui_location.min.y {
                    // click position is above first galley
                    cursor.pos = DocCharOffset(0);
                } else if pos.y >= galleys[galleys.len() - 1].ui_location.max.y {
                    // click position is below last galley
                    cursor.pos = segs.last_cursor_position();
                } else {
                    for galley_idx in 0..galleys.len() {
                        let galley = &galleys[galley_idx];
                        if galley.ui_location.contains(pos) {
                            // click position is in a galley
                            let relative_pos = pos - galley.text_location;
                            let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                            cursor.pos = galleys.char_offset_by_galley_and_cursor(
                                galley_idx,
                                &new_cursor,
                                segs,
                            );
                        }
                    }
                }
            }
        }

        if cursor != previous_cursor {
            modifications.push(Modification::Cursor { cursor });
            previous_cursor = cursor;
        }
    }

    modifications
}

#[allow(clippy::too_many_arguments)]
fn increment_numbered_list_items<'a>(
    starting_layout_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    segs: &UnicodeSegs, layouts: &Layouts, buffer: &Buffer, cursor: Cursor,
) -> Vec<Modification<'a>> {
    let mut modifications = Vec::new();

    let mut layout_idx = starting_layout_idx;
    loop {
        layout_idx += 1;
        if layout_idx == layouts.len() {
            break;
        }
        let layout = &layouts[layout_idx];
        if let Some(Annotation::Item(ItemType::Numbered(cur_number), cur_indent_level)) =
            layout.annotation
        {
            match cur_indent_level.cmp(&indent_level) {
                Ordering::Greater => {
                    continue; // skip nested list items
                }
                Ordering::Less => {
                    break; // end of nested list
                }
                Ordering::Equal => {
                    // replace cur_number with next_number in head
                    modifications.push(Modification::Cursor {
                        cursor: Cursor {
                            pos: segs.byte_offset_to_char(layout.range.start + layout.head_size),
                            selection_origin: Some(segs.byte_offset_to_char(layout.range.start)),
                            ..Default::default()
                        },
                    });
                    let head = layout.head(buffer);
                    let text = head[0..head.len() - (cur_number).to_string().len() - 2].to_string()
                        + &(if !decrement { cur_number + amount } else { cur_number - amount })
                            .to_string()
                        + ". ";
                    modifications.push(Modification::InsertOwned { text });
                    modifications.push(Modification::Cursor { cursor });
                }
            }
        } else {
            break;
        }
    }

    modifications
}

#[cfg(test)]
mod test {
    use crate::cursor::Cursor;
    use crate::events::{apply_modifications, Modification};
    use crate::unicode_segs;

    #[test]
    fn apply_modifications_none_empty_doc() {
        let mut buffer = Default::default();
        let mut cursor = Default::default();
        let mut debug = Default::default();
        let mut segs = unicode_segs::calc(&buffer);

        let modifications = Default::default();

        let text_updated =
            apply_modifications(modifications, &mut buffer, &mut segs, &mut cursor, &mut debug);

        assert_eq!(buffer.raw, "");
        assert_eq!(cursor, Default::default());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_modifications_none() {
        let mut buffer = "document content".into();
        let mut cursor = 9.into();
        let mut debug = Default::default();
        let mut segs = unicode_segs::calc(&buffer);

        let modifications = Default::default();

        let text_updated =
            apply_modifications(modifications, &mut buffer, &mut segs, &mut cursor, &mut debug);

        assert_eq!(buffer.raw, "document content");
        assert_eq!(cursor, 9.into());
        assert!(!debug.draw_enabled);
        assert!(!text_updated);
    }

    #[test]
    fn apply_modifications_insert() {
        let mut buffer = "document content".into();
        let mut cursor = 9.into();
        let mut debug = Default::default();
        let mut segs = unicode_segs::calc(&buffer);

        let modifications = vec![Modification::Insert { text: "new " }];

        let text_updated =
            apply_modifications(modifications, &mut buffer, &mut segs, &mut cursor, &mut debug);

        assert_eq!(buffer.raw, "document new content");
        assert_eq!(cursor, 13.into());
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
            let mut cursor = Cursor {
                pos: case.cursor_a.0.into(),
                x_target: None,
                selection_origin: Some(case.cursor_a.1.into()),
                click_and_drag_origin: None,
            };
            let mut buffer = "1234567".into();
            let mut debug = Default::default();
            let mut segs = unicode_segs::calc(&buffer);

            let modifications = vec![
                Modification::Insert { text: "a" },
                Modification::Cursor {
                    cursor: Cursor {
                        pos: case.cursor_b.0.into(),
                        x_target: None,
                        selection_origin: Some(case.cursor_b.1.into()),
                        click_and_drag_origin: None,
                    },
                },
                Modification::Insert { text: "b" },
            ];

            let text_updated =
                apply_modifications(modifications, &mut buffer, &mut segs, &mut cursor, &mut debug);

            assert_eq!(buffer.raw, case.expected_buffer);
            assert_eq!(cursor.pos.0, case.expected_cursor.0);
            assert_eq!(cursor.selection_origin, Some(case.expected_cursor.1.into()));
            assert!(!debug.draw_enabled);
            assert!(text_updated);
        }
    }
}
