use crate::appearance::Appearance;
use crate::buffer::{Buffer, Modification, SubBuffer, SubModification};
use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::element::ItemType;
use crate::galleys::Galleys;
use crate::layouts::{Annotation, Layouts};
use crate::offset_types::DocCharOffset;
use crate::unicode_segs::UnicodeSegs;
use egui::{Event, Key, PointerButton, Pos2, Vec2};
use std::cmp::Ordering;
use std::time::Instant;

/// processes `events` and returns a boolean representing whether text was updated and optionally new contents for clipboard
pub fn process(
    events: &[Event], layouts: &Layouts, galleys: &Galleys, appearance: &Appearance, ui_size: Vec2,
    buffer: &mut Buffer, debug: &mut DebugInfo,
) -> (bool, Option<String>) {
    let (mut text_updated, modification) =
        calc_modification(events, layouts, galleys, appearance, buffer, debug, ui_size);
    let mut to_clipboard = None;
    if !modification.is_empty() {
        let (text_updated_apply, to_clipboard_apply) = buffer.apply(modification, debug);
        text_updated |= text_updated_apply;
        to_clipboard = to_clipboard_apply;
    }
    (text_updated, to_clipboard)
}

// note: buffer and debug are mut because undo modifies it directly; todo: factor to make mutating subset of code obvious
fn calc_modification(
    events: &[Event], layouts: &Layouts, galleys: &Galleys, appearance: &Appearance,
    buffer: &mut Buffer, debug: &mut DebugInfo, ui_size: Vec2,
) -> (bool, Modification) {
    let mut text_updated = false;
    let mut modifications = Vec::new();
    let mut previous_cursor = buffer.current.cursor;
    let mut cursor = buffer.current.cursor;

    cursor.fix(false, &buffer.current.segs, galleys);
    if cursor != previous_cursor {
        modifications.push(SubModification::Cursor { cursor });
        previous_cursor = cursor;
    }

    for event in events {
        match event {
            Event::Key { key: Key::ArrowRight, pressed: true, modifiers } => {
                cursor.x_target = None;

                let (galley_idx, cur_cursor) =
                    galleys.galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.alt {
                    cursor.advance_word(false, &buffer.current, &buffer.current.segs, galleys);
                } else if modifiers.command {
                    let galley = &galleys[galley_idx];
                    let new_cursor = galley.galley.cursor_end_of_row(&cur_cursor);
                    cursor.pos = galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &new_cursor,
                        &buffer.current.segs,
                    );
                } else {
                    cursor.advance_char(false, &buffer.current.segs, galleys);
                }
            }
            Event::Key { key: Key::ArrowLeft, pressed: true, modifiers } => {
                cursor.x_target = None;

                let (galley_idx, cur_cursor) =
                    galleys.galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.alt {
                    cursor.advance_word(true, &buffer.current, &buffer.current.segs, galleys);
                } else if modifiers.command {
                    let galley = &galleys[galley_idx];
                    let new_cursor = galley.galley.cursor_begin_of_row(&cur_cursor);
                    cursor.pos = galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &new_cursor,
                        &buffer.current.segs,
                    );
                } else {
                    cursor.advance_char(true, &buffer.current.segs, galleys);
                }
            }
            Event::Key { key: Key::ArrowDown, pressed: true, modifiers } => {
                if modifiers.shift {
                    cursor.set_selection_origin();
                } else {
                    cursor.selection_origin = None;
                }
                if modifiers.command {
                    cursor.pos = buffer.current.segs.last_cursor_position();
                    cursor.fix(false, &buffer.current.segs, galleys);
                    cursor.x_target = None;
                } else {
                    let (cur_galley_idx, cur_cursor) =
                        galleys.galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
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

                    cursor.pos = galleys.char_offset_by_galley_and_cursor(
                        new_galley_idx,
                        &new_cursor,
                        &buffer.current.segs,
                    );
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
                    cursor.fix(false, &buffer.current.segs, galleys);
                    cursor.x_target = None;
                } else {
                    let (cur_galley_idx, cur_cursor) =
                        galleys.galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
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

                    cursor.pos = galleys.char_offset_by_galley_and_cursor(
                        new_galley_idx,
                        &new_cursor,
                        &buffer.current.segs,
                    );
                }
            }
            Event::Paste(text) | Event::Text(text) => {
                cursor.x_target = None;

                modifications.push(SubModification::Insert { text: text.clone() });

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Backspace, pressed: true, modifiers } => {
                cursor.x_target = None;

                let layout_idx = layouts.layout_at_char(cursor.pos, &buffer.current.segs);
                let layout = &layouts[layout_idx];
                if layout.head_size > 0
                    && buffer.current.segs.char_offset_to_byte(cursor.pos)
                        == layout.range.start + layout.head_size
                    && cursor.selection().is_none()
                {
                    // delete layout head (e.g. bullet)
                    modifications
                        .push(SubModification::Delete(layout.head_size_chars(&buffer.current)));

                    // if we deleted an item in a numbered list, decrement subsequent items
                    if let Some(Annotation::Item(ItemType::Numbered(_), indent_level)) =
                        layout.annotation
                    {
                        modifications.extend(increment_numbered_list_items(
                            layout_idx,
                            indent_level,
                            1,
                            true,
                            &buffer.current.segs,
                            layouts,
                            &buffer.current,
                            cursor,
                        ));
                    }
                } else {
                    if modifiers.command {
                        // select line start to current position
                        let (galley_idx, cur_cursor) = galleys
                            .galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
                        let galley = &galleys[galley_idx];
                        let begin_of_row_cursor = galley.galley.cursor_begin_of_row(&cur_cursor);
                        let begin_of_row_pos = galleys.char_offset_by_galley_and_cursor(
                            galley_idx,
                            &begin_of_row_cursor,
                            &buffer.current.segs,
                        );

                        modifications.push(SubModification::Cursor {
                            cursor: (begin_of_row_pos, cursor.pos).into(),
                        })
                    } else if modifiers.alt {
                        // select word
                        let end_of_word_pos = cursor.pos;
                        cursor.advance_word(true, &buffer.current, &buffer.current.segs, galleys);
                        let begin_of_word_pos = cursor.pos;

                        modifications.push(SubModification::Cursor {
                            cursor: (begin_of_word_pos, end_of_word_pos).into(),
                        })
                    }

                    // delete selected text or one character
                    modifications.push(SubModification::Delete(1.into()));
                }

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Enter, pressed: true, modifiers: _ } => {
                cursor.x_target = None;

                let layout_idx = layouts.layout_at_char(cursor.pos, &buffer.current.segs);
                let layout = &layouts[layout_idx];
                if buffer.current.segs.char_offset_to_byte(cursor.pos)
                    == layout.range.end - layout.tail_size
                    && matches!(layout.annotation, Some(Annotation::Item(..)))
                {
                    // cursor at end of list item
                    if layout.size() - layout.head_size - layout.tail_size == 0 {
                        // empty list item -> delete current annotation
                        modifications.push(SubModification::Cursor {
                            cursor: Cursor {
                                pos: buffer
                                    .current
                                    .segs
                                    .byte_offset_to_char(layout.range.start + layout.head_size),
                                selection_origin: Some(
                                    buffer.current.segs.byte_offset_to_char(layout.range.start),
                                ),
                                ..Default::default()
                            },
                        });
                        modifications.push(SubModification::Delete(0.into()));
                        modifications.push(SubModification::Cursor { cursor });
                    } else {
                        // nonempty list item -> insert new list item
                        modifications.push(SubModification::Insert { text: "\n".to_string() });

                        match layout.annotation {
                            Some(Annotation::Item(ItemType::Bulleted, _)) => {
                                modifications.push(SubModification::Insert {
                                    text: layout.head(&buffer.current).to_string(),
                                });
                            }
                            Some(Annotation::Item(
                                ItemType::Numbered(cur_number),
                                indent_level,
                            )) => {
                                let head = layout.head(&buffer.current);
                                let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                                    .to_string()
                                    + &(cur_number + 1).to_string()
                                    + ". ";
                                modifications.push(SubModification::Insert { text });

                                modifications.extend(increment_numbered_list_items(
                                    layout_idx,
                                    indent_level,
                                    1,
                                    false,
                                    &buffer.current.segs,
                                    layouts,
                                    &buffer.current,
                                    cursor,
                                ));
                            }
                            Some(Annotation::Item(ItemType::Todo(_), _)) => {
                                let head = layout.head(&buffer.current);
                                let text = head[0..head.len() - 6].to_string() + "- [ ] ";
                                modifications.push(SubModification::Insert { text });
                            }
                            Some(Annotation::Image(_, _, _)) => {}
                            Some(Annotation::Rule) => {}
                            None => {}
                        }
                    }
                } else if buffer.current.segs.char_offset_to_byte(cursor.pos)
                    == layout.range.start + layout.head_size
                    && !matches!(layout.annotation, Some(Annotation::Item(..)))
                {
                    // cursor at start of non-list item -> insert newline before annotation
                    modifications.push(SubModification::Cursor {
                        cursor: Cursor {
                            pos: buffer.current.segs.byte_offset_to_char(layout.range.start),
                            ..Default::default()
                        },
                    });
                    modifications.push(SubModification::Insert { text: "\n".to_string() });
                    modifications.push(SubModification::Cursor { cursor });
                } else {
                    modifications.push(SubModification::Insert { text: "\n".to_string() });
                }

                cursor.selection_origin = None;
            }
            Event::Key { key: Key::Tab, pressed: true, modifiers } => {
                // if we're in a list item, tab/shift+tab will indent/de-indent
                // otherwise, tab will insert a tab and shift tab will do nothing
                let layout_idx = layouts.layout_at_char(cursor.pos, &buffer.current.segs);
                let layout = &layouts[layout_idx];
                if let Some(annotation) = &layout.annotation {
                    match annotation {
                        Annotation::Item(item_type, indent_level) => {
                            // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                            let layout_text =
                                &&buffer.current.text[layout.range.start.0..layout.range.end.0];
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
                                    modifications.push(SubModification::Cursor {
                                        cursor: Cursor {
                                            pos: buffer.current.segs.byte_offset_to_char(
                                                layout.range.start + indent_seq.len(),
                                            ),
                                            selection_origin: Some(
                                                buffer
                                                    .current
                                                    .segs
                                                    .byte_offset_to_char(layout.range.start),
                                            ),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(SubModification::Delete(0.into()));
                                    modifications.push(SubModification::Cursor { cursor });

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
                                if let Some(Annotation::Item(_, prior_indent_level)) =
                                    &prior_layout.annotation
                                {
                                    if prior_indent_level < indent_level {
                                        can_indent = false; // list item cannot be indented if already indented more than prior item
                                    }
                                } else {
                                    can_indent = false; // first list item of a list cannot be indented
                                }

                                if can_indent {
                                    // indentation: set cursor to galley start, insert indentation sequence, restore cursor
                                    modifications.push(SubModification::Cursor {
                                        cursor: Cursor {
                                            pos: buffer
                                                .current
                                                .segs
                                                .byte_offset_to_char(layout.range.start),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(SubModification::Insert {
                                        text: indent_seq.to_string(),
                                    });
                                    modifications.push(SubModification::Cursor { cursor });

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
                                    modifications.push(SubModification::Cursor {
                                        cursor: Cursor {
                                            pos: buffer.current.segs.byte_offset_to_char(
                                                layout.range.start + layout.head_size
                                                    - (cur_number).to_string().len()
                                                    - 2,
                                            ),
                                            selection_origin: Some(
                                                buffer.current.segs.byte_offset_to_char(
                                                    layout.range.start + layout.head_size,
                                                ),
                                            ),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(SubModification::Insert {
                                        text: new_number.to_string() + ". ",
                                    });
                                    modifications.push(SubModification::Cursor { cursor });

                                    if modifiers.shift {
                                        // decrement numbers in old list by this item's old number
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            *indent_level,
                                            *cur_number,
                                            true,
                                            &buffer.current.segs,
                                            layouts,
                                            &buffer.current,
                                            cursor,
                                        ));

                                        // increment numbers in new nested list by one
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            new_indent_level,
                                            1,
                                            false,
                                            &buffer.current.segs,
                                            layouts,
                                            &buffer.current,
                                            cursor,
                                        ));
                                    } else {
                                        // decrement numbers in old list by one
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            *indent_level,
                                            1,
                                            true,
                                            &buffer.current.segs,
                                            layouts,
                                            &buffer.current,
                                            cursor,
                                        ));

                                        // increment numbers in new nested list by this item's new number
                                        modifications.extend(increment_numbered_list_items(
                                            layout_idx,
                                            new_indent_level,
                                            new_number,
                                            false,
                                            &buffer.current.segs,
                                            layouts,
                                            &buffer.current,
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
                    modifications.push(SubModification::Insert { text: "\t".to_string() });
                }
            }
            Event::Key { key: Key::A, pressed: true, modifiers } => {
                if modifiers.command {
                    cursor.selection_origin = Some(DocCharOffset(0));
                    cursor.pos = buffer.current.segs.last_cursor_position();
                }
            }
            Event::Key { key: Key::C, pressed: true, modifiers } => {
                if modifiers.command {
                    modifications.push(SubModification::ToClipboard {
                        text: cursor
                            .selection_text(&buffer.current, &buffer.current.segs)
                            .to_string(),
                    });
                }
            }
            Event::Key { key: Key::X, pressed: true, modifiers } => {
                if modifiers.command {
                    modifications.push(SubModification::ToClipboard {
                        text: cursor
                            .selection_text(&buffer.current, &buffer.current.segs)
                            .to_string(),
                    });
                    modifications.push(SubModification::Delete(0.into()));
                }
            }
            Event::Key { key: Key::Z, pressed: true, modifiers } => {
                if modifiers.command {
                    // mutate buffer directly - undo does not become a modification because it cannot be undone
                    if modifiers.shift {
                        buffer.redo(debug);
                    } else {
                        buffer.undo(debug);
                    }
                    text_updated = true;
                }
            }
            Event::Key { key: Key::F2, pressed: true, modifiers: _modifiers } => {
                modifications.push(SubModification::DebugToggle);
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers,
            } => {
                // do not process scrollbar clicks
                if pos.x <= ui_size.x {
                    // process checkbox clicks
                    let checkbox_click = {
                        let mut checkbox_click = false;
                        for galley in &galleys.galleys {
                            if let Some(Annotation::Item(ItemType::Todo(checked), ..)) =
                                galley.annotation
                            {
                                if galley.checkbox_bounds(appearance).contains(*pos) {
                                    modifications.push(SubModification::Cursor {
                                        cursor: Cursor {
                                            pos: buffer.current.segs.byte_offset_to_char(
                                                galley.range.start + galley.head_size,
                                            ),
                                            selection_origin: Some(
                                                buffer.current.segs.byte_offset_to_char(
                                                    galley.range.start + galley.head_size - 6,
                                                ),
                                            ),
                                            ..Default::default()
                                        },
                                    });
                                    modifications.push(SubModification::Insert {
                                        text: if checked { "- [ ] " } else { "- [x] " }.to_string(),
                                    });
                                    modifications.push(SubModification::Cursor { cursor });

                                    checkbox_click = true;
                                    break;
                                }
                            }
                        }
                        checkbox_click
                    };
                    if !checkbox_click {
                        // record instant for double/triple click
                        cursor.process_click_instant(Instant::now());

                        let mut double_click = false;
                        let mut triple_click = false;
                        if !modifiers.shift {
                            // click: end selection
                            cursor.selection_origin = None;

                            double_click = cursor.double_click();
                            triple_click = cursor.triple_click();
                        } else {
                            // shift+click: begin selection
                            cursor.set_selection_origin();
                        }
                        // any click: begin drag; update cursor
                        cursor.set_click_and_drag_origin();
                        if triple_click {
                            if let Some(click_offset) =
                                pos_to_char_offset(*pos, galleys, &buffer.current.segs)
                            {
                                cursor.pos = click_offset;
                            }

                            let (galley_idx, cur_cursor) = galleys
                                .galley_and_cursor_by_char_offset(cursor.pos, &buffer.current.segs);
                            let galley = &galleys[galley_idx];
                            let begin_of_row_cursor =
                                galley.galley.cursor_begin_of_row(&cur_cursor);
                            let end_of_row_cursor = galley.galley.cursor_end_of_row(&cur_cursor);

                            cursor.selection_origin =
                                Some(galleys.char_offset_by_galley_and_cursor(
                                    galley_idx,
                                    &begin_of_row_cursor,
                                    &buffer.current.segs,
                                ));
                            cursor.pos = galleys.char_offset_by_galley_and_cursor(
                                galley_idx,
                                &end_of_row_cursor,
                                &buffer.current.segs,
                            );
                        } else if double_click {
                            if let Some(click_offset) =
                                pos_to_char_offset(*pos, galleys, &buffer.current.segs)
                            {
                                cursor.pos = click_offset;
                            }

                            cursor.advance_word(
                                false,
                                &buffer.current,
                                &buffer.current.segs,
                                galleys,
                            );
                            let end_of_word_pos = cursor.pos;
                            cursor.advance_word(
                                true,
                                &buffer.current,
                                &buffer.current.segs,
                                galleys,
                            );
                            let begin_of_word_pos = cursor.pos;

                            cursor.selection_origin = Some(begin_of_word_pos);
                            cursor.pos = end_of_word_pos;
                        } else if let Some(click_offset) =
                            pos_to_char_offset(*pos, galleys, &buffer.current.segs)
                        {
                            cursor.pos = click_offset;
                        }
                    }
                }
            }
            Event::PointerMoved(pos) => {
                if cursor.click_and_drag_origin.is_some()
                    && !cursor.double_click()
                    && !cursor.triple_click()
                {
                    // drag: begin selection; update cursor
                    cursor.set_selection_origin();
                    if let Some(click_offset) =
                        pos_to_char_offset(*pos, galleys, &buffer.current.segs)
                    {
                        cursor.pos = click_offset;
                    }
                }
            }
            Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } => {
                // click released: end drag; don't update cursor
                cursor.click_and_drag_origin = None;
            }
            _ => {}
        }

        if cursor != previous_cursor {
            modifications.push(SubModification::Cursor { cursor });
            previous_cursor = cursor;
        }
    }

    // todo: more thoughtful way to group modifications
    // modifications must be grouped because operations like deleting an annotation involve several steps but should be undone as one operation
    // this way to group modifications puts all operations performed in one frame as a single group
    (text_updated, modifications)
}

pub fn pos_to_char_offset(
    pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs,
) -> Option<DocCharOffset> {
    if !galleys.is_empty() {
        if pos.y < galleys[0].galley_location.min.y {
            // click position is above first galley
            Some(DocCharOffset(0))
        } else if pos.y >= galleys[galleys.len() - 1].galley_location.max.y {
            // click position is below last galley
            Some(segs.last_cursor_position())
        } else {
            let mut result = None;
            for galley_idx in 0..galleys.len() {
                let galley = &galleys[galley_idx];
                if galley.galley_location.contains(pos) {
                    // click position is in a galley
                    let relative_pos = pos - galley.text_location;
                    let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                    result = Some(galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &new_cursor,
                        segs,
                    ));
                }
            }
            result
        }
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn increment_numbered_list_items(
    starting_layout_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    segs: &UnicodeSegs, layouts: &Layouts, buffer: &SubBuffer, cursor: Cursor,
) -> Modification {
    let mut modifications = Vec::new();

    let mut layout_idx = starting_layout_idx;
    loop {
        layout_idx += 1;
        if layout_idx == layouts.len() {
            break;
        }
        let layout = &layouts[layout_idx];
        if let Some(Annotation::Item(item_type, cur_indent_level)) = &layout.annotation {
            match cur_indent_level.cmp(&indent_level) {
                Ordering::Greater => {
                    continue; // skip nested list items
                }
                Ordering::Less => {
                    break; // end of nested list
                }
                Ordering::Equal => {
                    if let ItemType::Numbered(cur_number) = item_type {
                        // replace cur_number with next_number in head
                        modifications.push(SubModification::Cursor {
                            cursor: Cursor {
                                pos: segs
                                    .byte_offset_to_char(layout.range.start + layout.head_size),
                                selection_origin: Some(
                                    segs.byte_offset_to_char(layout.range.start),
                                ),
                                ..Default::default()
                            },
                        });
                        let head = layout.head(buffer);
                        let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                            .to_string()
                            + &(if !decrement {
                                cur_number.saturating_add(amount)
                            } else {
                                cur_number.saturating_sub(amount)
                            })
                            .to_string()
                            + ". ";
                        modifications.push(SubModification::Insert { text });
                        modifications.push(SubModification::Cursor { cursor });
                    }
                }
            }
        } else {
            break;
        }
    }

    modifications
}
