use crate::buffer::{EditorMutation, Mutation, SubBuffer, SubMutation};
use crate::element::ItemType;
use crate::galleys::Galleys;
use crate::input::canonical::{Location, Modification, Offset, Region};
use crate::input::cursor::Cursor;
use crate::layouts::{Annotation, Layouts};
use crate::offset_types::DocCharOffset;
use crate::unicode_segs::UnicodeSegs;
use egui::Pos2;
use std::cmp::Ordering;

#[allow(clippy::too_many_arguments)]
pub fn calc(
    modification: Modification, layouts: &Layouts, buffer: &SubBuffer, galleys: &Galleys,
) -> EditorMutation {
    let current_cursor = buffer.cursor;
    let segs = &buffer.segs;
    let mut modifications = Vec::new(); // todo: rename
    match modification {
        Modification::Select { region } => modifications.push(SubMutation::Cursor {
            cursor: region_to_cursor(region, current_cursor, buffer, galleys, segs),
        }),
        Modification::Mark { .. } => {} // todo
        Modification::Replace { region, text } => {
            modifications.push(SubMutation::Cursor {
                cursor: region_to_cursor(region, current_cursor, buffer, galleys, segs),
            });
            modifications.push(SubMutation::Insert { text });
            modifications.push(SubMutation::Cursor { cursor: current_cursor });
        }
        Modification::Newline => {
            let mut cursor = current_cursor;
            let layout_idx = layouts.layout_at_char(cursor.pos, segs);
            let layout = &layouts[layout_idx];
            if segs.char_offset_to_byte(cursor.pos) == layout.range.end - layout.tail_size
                && matches!(layout.annotation, Some(Annotation::Item(..)))
            {
                // cursor at end of list item
                if layout.size() - layout.head_size - layout.tail_size == 0 {
                    // empty list item -> delete current annotation
                    modifications.push(SubMutation::Cursor {
                        cursor: Cursor {
                            pos: segs.byte_offset_to_char(layout.range.start + layout.head_size),
                            selection_origin: Some(segs.byte_offset_to_char(layout.range.start)),
                            ..Default::default()
                        },
                    });
                    modifications.push(SubMutation::Delete(0.into()));
                    modifications.push(SubMutation::Cursor { cursor });
                } else {
                    // nonempty list item -> insert new list item
                    modifications.push(SubMutation::Insert { text: "\n".to_string() });

                    match layout.annotation {
                        Some(Annotation::Item(ItemType::Bulleted, _)) => {
                            modifications.push(SubMutation::Insert {
                                text: layout.head(buffer).to_string(),
                            });
                        }
                        Some(Annotation::Item(ItemType::Numbered(cur_number), indent_level)) => {
                            let head = layout.head(buffer);
                            let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                                .to_string()
                                + &(cur_number + 1).to_string()
                                + ". ";
                            modifications.push(SubMutation::Insert { text });

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
                            let text = head[0..head.len() - 6].to_string() + "- [ ] ";
                            modifications.push(SubMutation::Insert { text });
                        }
                        Some(Annotation::Image(_, _, _)) => {}
                        Some(Annotation::Rule) => {}
                        None => {}
                    }
                }
            } else if segs.char_offset_to_byte(cursor.pos) == layout.range.start + layout.head_size
                && !matches!(layout.annotation, Some(Annotation::Item(..)))
            {
                // cursor at start of non-list item -> insert newline before annotation
                modifications.push(SubMutation::Cursor {
                    cursor: Cursor {
                        pos: segs.byte_offset_to_char(layout.range.start),
                        ..Default::default()
                    },
                });
                modifications.push(SubMutation::Insert { text: "\n".to_string() });
                modifications.push(SubMutation::Cursor { cursor });
            } else {
                modifications.push(SubMutation::Insert { text: "\n".to_string() });
            }

            cursor.selection_origin = None;
        }
        Modification::Indent { deindent } => {
            // if we're in a list item, tab/shift+tab will indent/de-indent
            // otherwise, tab will insert a tab and shift tab will do nothing
            let layout_idx = layouts.layout_at_char(current_cursor.pos, segs);
            let layout = &layouts[layout_idx];
            if let Some(annotation) = &layout.annotation {
                match annotation {
                    Annotation::Item(item_type, indent_level) => {
                        // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                        let layout_text = &buffer.text[layout.range.start.0..layout.range.end.0];
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
                        let new_indent_level = if deindent {
                            let mut can_deindent = true;
                            if *indent_level == 1 {
                                can_deindent = false; // cannot de-indent un-indented list item
                            } else if layout_idx != layouts.len() - 1 {
                                let next_layout = &layouts[layout_idx + 1];
                                if let Some(Annotation::Item(next_item_type, next_indent_level)) =
                                    &next_layout.annotation
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
                                modifications.push(SubMutation::Cursor {
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
                                modifications.push(SubMutation::Delete(0.into()));
                                modifications.push(SubMutation::Cursor { cursor: current_cursor });

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
                                modifications.push(SubMutation::Cursor {
                                    cursor: Cursor {
                                        pos: segs.byte_offset_to_char(layout.range.start),
                                        ..Default::default()
                                    },
                                });
                                modifications
                                    .push(SubMutation::Insert { text: indent_seq.to_string() });
                                modifications.push(SubMutation::Cursor { cursor: current_cursor });

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
                                modifications.push(SubMutation::Cursor {
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
                                modifications.push(SubMutation::Insert {
                                    text: new_number.to_string() + ". ",
                                });
                                modifications.push(SubMutation::Cursor { cursor: current_cursor });

                                if deindent {
                                    // decrement numbers in old list by this item's old number
                                    modifications.extend(increment_numbered_list_items(
                                        layout_idx,
                                        *indent_level,
                                        *cur_number,
                                        true,
                                        segs,
                                        layouts,
                                        buffer,
                                        current_cursor,
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
                                        current_cursor,
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
                                        current_cursor,
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
                                        current_cursor,
                                    ));
                                }
                            }
                        }
                    }
                    Annotation::Image(..) => {}
                    Annotation::Rule => {}
                }
            } else if !deindent {
                modifications.push(SubMutation::Insert { text: "\t".to_string() });
            }
        }
        Modification::Undo => {
            return EditorMutation::Undo;
        }
        Modification::Redo => {
            return EditorMutation::Redo;
        }
        Modification::Cut => {
            modifications.push(SubMutation::ToClipboard {
                text: current_cursor.selection_text(buffer, segs).to_string(),
            });
            modifications.push(SubMutation::Insert { text: "".to_string() });
        }
        Modification::Copy => {
            modifications.push(SubMutation::ToClipboard {
                text: current_cursor.selection_text(buffer, segs).to_string(),
            });
        }
        Modification::OpenUrl(url) => {
            modifications.push(SubMutation::OpenedUrl { url });
        }
        Modification::ToggleDebug => {
            modifications.push(SubMutation::DebugToggle);
        }
        Modification::ToggleCheckbox(galley_idx) => {
            let galley = &galleys[galley_idx];
            if let Some(Annotation::Item(ItemType::Todo(checked), ..)) = galley.annotation {
                modifications.push(SubMutation::Cursor {
                    cursor: Cursor {
                        pos: segs.byte_offset_to_char(galley.range.start + galley.head_size),
                        selection_origin: Some(
                            segs.byte_offset_to_char(galley.range.start + galley.head_size - 6),
                        ),
                        ..Default::default()
                    },
                });
                modifications.push(SubMutation::Insert {
                    text: if checked { "- [ ] " } else { "- [x] " }.to_string(),
                });
                modifications.push(SubMutation::Cursor { cursor: current_cursor });
            }
        }
    }
    EditorMutation::Buffer(modifications)
}

pub fn region_to_cursor(
    region: Region, current_cursor: Cursor, buffer: &SubBuffer, galleys: &Galleys,
    segs: &UnicodeSegs,
) -> Cursor {
    match region {
        Region::Location(location) => {
            location_to_char_offset(location, current_cursor, galleys, segs).into()
        }
        Region::ToLocation(location) => (
            current_cursor.selection_origin(),
            location_to_char_offset(location, current_cursor, galleys, segs),
        )
            .into(),
        Region::Selection => current_cursor,
        Region::SelectionOrOffset { offset, backwards } => {
            if current_cursor.selection().is_none() {
                let mut cursor = current_cursor;
                cursor.advance(offset, backwards, buffer, segs, galleys);
                cursor.selection_origin = Some(current_cursor.pos);
                cursor
            } else {
                current_cursor
            }
        }
        Region::ToOffset { offset, backwards, extend_selection } => {
            let mut cursor = current_cursor;
            cursor.advance(offset, backwards, buffer, segs, galleys);
            if extend_selection {
                cursor.selection_origin =
                    current_cursor.selection_origin.or(Some(current_cursor.pos));
            } else {
                cursor.selection_origin = None;
            }
            cursor
        }
        Region::Bound { bound } => {
            let mut cursor = current_cursor;
            cursor.advance(Offset::To(bound), true, buffer, segs, galleys);
            cursor.selection_origin = Some(cursor.pos);
            cursor.advance(Offset::To(bound), false, buffer, segs, galleys);
            cursor
        }
        Region::BoundAt { bound, location } => {
            let mut cursor: Cursor =
                location_to_char_offset(location, current_cursor, galleys, segs).into();
            cursor.advance(Offset::To(bound), true, buffer, segs, galleys);
            cursor.selection_origin = Some(cursor.pos);
            cursor.advance(Offset::To(bound), false, buffer, segs, galleys);
            cursor
        }
    }
}

pub fn location_to_char_offset(
    location: Location, current_cursor: Cursor, galleys: &Galleys, segs: &UnicodeSegs,
) -> DocCharOffset {
    match location {
        Location::CurrentCursor => current_cursor.pos,
        Location::DocCharOffset(o) => o,
        Location::Pos(pos) => pos_to_char_offset(pos, galleys, segs),
    }
}

pub fn pos_to_char_offset(pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs) -> DocCharOffset {
    if !galleys.is_empty() && pos.y < galleys[0].galley_location.min.y {
        // click position is above first galley
        0.into()
    } else if !galleys.is_empty() && pos.y >= galleys[galleys.len() - 1].galley_location.max.y {
        // click position is below last galley
        segs.last_cursor_position()
    } else {
        let mut result = 0.into();
        for galley_idx in 0..galleys.len() {
            let galley = &galleys[galley_idx];
            if galley.galley_location.min.y <= pos.y && pos.y <= galley.galley_location.max.y {
                // click position is in a galley
                let relative_pos = pos - galley.text_location;
                let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                result = galleys.char_offset_by_galley_and_cursor(galley_idx, &new_cursor, segs);
            }
        }
        result
    }
}

#[allow(clippy::too_many_arguments)]
pub fn increment_numbered_list_items(
    starting_layout_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    segs: &UnicodeSegs, layouts: &Layouts, buffer: &SubBuffer, cursor: Cursor,
) -> Mutation {
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
                        modifications.push(SubMutation::Cursor {
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
                        modifications.push(SubMutation::Insert { text });
                        modifications.push(SubMutation::Cursor { cursor });
                    }
                }
            }
        } else {
            break;
        }
    }

    modifications
}
