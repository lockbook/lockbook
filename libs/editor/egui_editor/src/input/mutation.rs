use crate::ast::{Ast, AstTextRangeType};
use crate::bounds::{AstTextRanges, Bounds, Text};
use crate::buffer::{EditorMutation, Mutation, SubBuffer, SubMutation};
use crate::galleys::Galleys;
use crate::input::canonical::{Location, Modification, Offset, Region};
use crate::input::cursor::Cursor;
use crate::layouts::Annotation;
use crate::offset_types::{DocCharOffset, RangeExt};
use crate::style::{InlineNodeType, ListItem, MarkdownNode, MarkdownNodeType};
use crate::unicode_segs::UnicodeSegs;
use egui::Pos2;
use std::cmp::Ordering;
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;

pub fn calc(
    modification: Modification, buffer: &SubBuffer, galleys: &Galleys, bounds: &Bounds, ast: &Ast,
) -> EditorMutation {
    let current_cursor = buffer.cursor;
    let mut mutation = Vec::new();
    match modification {
        Modification::Select { region } => mutation.push(SubMutation::Cursor {
            cursor: region_to_cursor(region, current_cursor, buffer, galleys, bounds),
        }),
        Modification::StageMarked { highlighted, text } => {
            let mut cursor = current_cursor;
            let text_length = text.grapheme_indices(true).count();

            // when inserting text, replacing existing marked text if any
            if let Some(mark) = cursor.mark {
                cursor.selection = mark;
            }

            // mark inserted text
            cursor.mark =
                Some((current_cursor.selection.0, current_cursor.selection.0 + text_length));

            // highlight is relative to text start
            cursor.mark_highlight = Some((
                current_cursor.selection.0 + highlighted.0,
                current_cursor.selection.0 + highlighted.1,
            ));

            mutation.push(SubMutation::Cursor { cursor });
            mutation.push(SubMutation::Insert { text, advance_cursor: true });
        }
        Modification::CommitMarked => {
            let mut cursor = current_cursor;
            cursor.mark = None;
            mutation.push(SubMutation::Cursor { cursor });
        }
        Modification::Replace { region, text } => {
            mutation.push(SubMutation::Cursor {
                cursor: region_to_cursor(region, current_cursor, buffer, galleys, bounds),
            });
            mutation.push(SubMutation::Insert { text, advance_cursor: true });
            mutation.push(SubMutation::Cursor { cursor: current_cursor });
        }
        Modification::ToggleStyle { region, style } => {
            let cursor = region_to_cursor(region, current_cursor, buffer, galleys, bounds);
            let unapply = region_completely_styled(cursor, &style, ast);
            if !unapply {
                for conflict in conflicting_styles(cursor, &style, ast) {
                    apply_style(cursor, conflict, true, buffer, ast, &mut mutation)
                }
            }
            apply_style(cursor, style.clone(), unapply, buffer, ast, &mut mutation);
            if current_cursor.selection.is_empty() {
                // toggling style at end of styled range moves cursor to outside of styled range
                if let Some(text_range) = ast.text_range_at_offset(current_cursor.selection.1) {
                    if text_range.node(ast).node_type() == style.node_type()
                        && text_range.range_type == AstTextRangeType::Tail
                    {
                        mutation.push(SubMutation::Cursor {
                            cursor: (text_range.range.end(), text_range.range.end()).into(),
                        });
                    }
                }
            } else if style.node_type() != MarkdownNodeType::Inline(InlineNodeType::Link) {
                // toggling link style leaves cursor where you can type link destination
                mutation.push(SubMutation::Cursor { cursor: current_cursor });
            }
        }
        Modification::Newline { advance_cursor } => {
            let mut cursor = current_cursor;
            let galley_idx = galleys.galley_at_char(cursor.selection.1);
            let galley = &galleys[galley_idx];
            let ast_text_range = ast.text_range_at_offset(cursor.selection.1);
            if matches!(galley.annotation, Some(Annotation::Item(..))) {
                // cursor at end of list item
                if galley.size() - galley.head_size - galley.tail_size == 0 {
                    // empty list item -> delete current annotation
                    mutation.push(SubMutation::Cursor {
                        cursor: (galley.range.start(), galley.range.start() + galley.head_size)
                            .into(),
                    });
                    mutation.push(SubMutation::Delete(0.into()));
                    mutation.push(SubMutation::Cursor { cursor });
                } else {
                    // nonempty list item -> insert new list item
                    mutation
                        .push(SubMutation::Insert { text: "\n".to_string(), advance_cursor: true });

                    match galley.annotation {
                        Some(Annotation::Item(ListItem::Bulleted, _)) => {
                            mutation.push(SubMutation::Insert {
                                text: galley.head(buffer).to_string(),
                                advance_cursor: true,
                            });
                        }
                        Some(Annotation::Item(ListItem::Numbered(cur_number), indent_level)) => {
                            let head = galley.head(buffer);
                            let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                                .to_string()
                                + &(cur_number + 1).to_string()
                                + ". ";
                            mutation.push(SubMutation::Insert { text, advance_cursor: true });

                            mutation.extend(increment_numbered_list_items(
                                galley_idx,
                                indent_level,
                                1,
                                false,
                                galleys,
                                buffer,
                                cursor,
                            ));
                        }
                        Some(Annotation::Item(ListItem::Todo(_), _)) => {
                            let head = galley.head(buffer);
                            let text = head[0..head.len() - 6].to_string() + "* [ ] ";
                            mutation.push(SubMutation::Insert { text, advance_cursor: true });
                        }
                        Some(Annotation::Image(_, _, _)) => {}
                        Some(Annotation::HeadingRule) => {}
                        Some(Annotation::Rule) => {}
                        None => {}
                    }
                }
            } else if cursor.selection.1 == galley.range.start() + galley.head_size
                && !matches!(galley.annotation, Some(Annotation::Item(..)))
            {
                // cursor at start of non-list item -> insert newline before annotation
                mutation.push(SubMutation::Cursor { cursor: galley.range.start().into() });
                mutation.push(SubMutation::Insert { text: "\n".to_string(), advance_cursor: true });
                mutation.push(SubMutation::Cursor { cursor });
            } else if let Some(ast_text_range) = ast_text_range {
                if ast_text_range.range_type == AstTextRangeType::Tail
                    && ast_text_range.node(ast).node_type()
                        == MarkdownNodeType::Inline(InlineNodeType::Link)
                    && ast_text_range.range.end() != cursor.selection.1
                {
                    // cursor inside link url -> move cursor to end of link
                    mutation.push(SubMutation::Cursor {
                        cursor: (ast_text_range.range.end(), ast_text_range.range.end()).into(),
                    });
                } else {
                    mutation.push(SubMutation::Insert { text: "\n".to_string(), advance_cursor });
                }
            } else {
                mutation.push(SubMutation::Insert { text: "\n".to_string(), advance_cursor });
            }

            cursor.selection.0 = cursor.selection.1;
        }
        Modification::Indent { deindent } => {
            // if we're in a list item, tab/shift+tab will indent/de-indent
            // otherwise, tab will insert a tab and shift tab will do nothing
            let galley_idx = galleys.galley_at_char(current_cursor.selection.1);
            let galley = &galleys[galley_idx];
            if let Some(annotation) = &galley.annotation {
                match annotation {
                    Annotation::Item(item_type, indent_level) => {
                        // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                        let galley_text = &buffer[(galley.range.start(), galley.range.end())];
                        let indent_seq = if galley_text.starts_with('\t') {
                            "\t"
                        } else if galley_text.starts_with("    ") {
                            "    "
                        } else if galley_text.starts_with("  ") {
                            "  "
                        } else {
                            "\t"
                        };

                        // indent or de-indent if able
                        let new_indent_level = if deindent {
                            let mut can_deindent = true;
                            if *indent_level == 0 {
                                can_deindent = false; // cannot de-indent un-indented list item
                            } else if galley_idx != galleys.len() - 1 {
                                let next_galley = &galleys[galley_idx + 1];
                                if let Some(Annotation::Item(next_item_type, next_indent_level)) =
                                    &next_galley.annotation
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
                                mutation.push(SubMutation::Cursor {
                                    cursor: (
                                        galley.range.start(),
                                        galley.range.start() + indent_seq.len(),
                                    )
                                        .into(),
                                });
                                mutation.push(SubMutation::Delete(0.into()));
                                mutation.push(SubMutation::Cursor { cursor: current_cursor });

                                indent_level - 1
                            } else {
                                *indent_level
                            }
                        } else {
                            let mut can_indent = true;
                            if galley_idx == 0 {
                                can_indent = false; // first galley cannot be indented
                            }
                            let prior_galley = &galleys[galley_idx - 1];
                            if let Some(Annotation::Item(_, prior_indent_level)) =
                                &prior_galley.annotation
                            {
                                if prior_indent_level < indent_level {
                                    can_indent = false; // list item cannot be indented if already indented more than prior item
                                }
                            } else {
                                can_indent = false; // first list item of a list cannot be indented
                            }

                            if can_indent {
                                // indentation: set cursor to galley start, insert indentation sequence, restore cursor
                                mutation.push(SubMutation::Cursor {
                                    cursor: galley.range.start().into(),
                                });
                                mutation.push(SubMutation::Insert {
                                    text: indent_seq.to_string(),
                                    advance_cursor: true,
                                });
                                mutation.push(SubMutation::Cursor { cursor: current_cursor });

                                indent_level + 1
                            } else {
                                *indent_level
                            }
                        };

                        // re-number numbered lists
                        if new_indent_level != *indent_level {
                            if let ListItem::Numbered(cur_number) = item_type {
                                // assign a new_number to this item based on position in new nested list
                                let new_number = {
                                    let mut new_number = 1;
                                    let mut prior_galley_idx = galley_idx;
                                    while prior_galley_idx > 0 {
                                        prior_galley_idx -= 1;
                                        let prior_galley = &galleys[prior_galley_idx];
                                        if let Some(Annotation::Item(
                                            ListItem::Numbered(prior_number),
                                            prior_indent_level,
                                        )) = prior_galley.annotation
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
                                mutation.push(SubMutation::Cursor {
                                    cursor: (
                                        galley.range.start() + galley.head_size,
                                        galley.range.start() + galley.head_size
                                            - (cur_number).to_string().len()
                                            - 2,
                                    )
                                        .into(),
                                });
                                mutation.push(SubMutation::Insert {
                                    text: new_number.to_string() + ". ",
                                    advance_cursor: true,
                                });
                                mutation.push(SubMutation::Cursor { cursor: current_cursor });

                                if deindent {
                                    // decrement numbers in old list by this item's old number
                                    mutation.extend(increment_numbered_list_items(
                                        galley_idx,
                                        *indent_level,
                                        *cur_number,
                                        true,
                                        galleys,
                                        buffer,
                                        current_cursor,
                                    ));

                                    // increment numbers in new nested list by one
                                    mutation.extend(increment_numbered_list_items(
                                        galley_idx,
                                        new_indent_level,
                                        1,
                                        false,
                                        galleys,
                                        buffer,
                                        current_cursor,
                                    ));
                                } else {
                                    // decrement numbers in old list by one
                                    mutation.extend(increment_numbered_list_items(
                                        galley_idx,
                                        *indent_level,
                                        1,
                                        true,
                                        galleys,
                                        buffer,
                                        current_cursor,
                                    ));

                                    // increment numbers in new nested list by this item's new number
                                    mutation.extend(increment_numbered_list_items(
                                        galley_idx,
                                        new_indent_level,
                                        new_number,
                                        false,
                                        galleys,
                                        buffer,
                                        current_cursor,
                                    ));
                                }
                            }
                        }
                    }
                    Annotation::Image(..) => {}
                    Annotation::HeadingRule => {}
                    Annotation::Rule => {}
                }
            } else if !deindent {
                mutation.push(SubMutation::Insert { text: "\t".to_string(), advance_cursor: true });
            }
        }
        Modification::Undo => {
            return EditorMutation::Undo;
        }
        Modification::Redo => {
            return EditorMutation::Redo;
        }
        Modification::Cut => {
            mutation.push(SubMutation::ToClipboard {
                text: current_cursor.selection_text(buffer).to_string(),
            });
            mutation.push(SubMutation::Insert { text: "".to_string(), advance_cursor: true });
        }
        Modification::Copy => {
            mutation.push(SubMutation::ToClipboard {
                text: current_cursor.selection_text(buffer).to_string(),
            });
        }
        Modification::OpenUrl(url) => mutation.push(SubMutation::OpenedUrl { url }),
        Modification::ToggleDebug => mutation.push(SubMutation::DebugToggle),
        Modification::SetBaseFontSize(size) => mutation.push(SubMutation::SetBaseFontSize(size)),
        Modification::ToggleCheckbox(galley_idx) => {
            let galley = &galleys[galley_idx];
            if let Some(Annotation::Item(ListItem::Todo(checked), ..)) = galley.annotation {
                mutation.push(SubMutation::Cursor {
                    cursor: (
                        galley.range.start() + galley.head_size - 6,
                        galley.range.start() + galley.head_size,
                    )
                        .into(),
                });
                mutation.push(SubMutation::Insert {
                    text: if checked { "* [ ] " } else { "* [x] " }.to_string(),
                    advance_cursor: true,
                });
                mutation.push(SubMutation::Cursor { cursor: current_cursor });
            }
        }
    }
    EditorMutation::Buffer(mutation)
}

/// Returns true if all text in `cursor` has style `style`
fn region_completely_styled(cursor: Cursor, style: &MarkdownNode, ast: &Ast) -> bool {
    if cursor.selection.is_empty() {
        return false;
    }

    for text_range in ast.iter_text_ranges() {
        // skip ranges before or after the cursor
        if text_range.range.end() <= cursor.selection.start() {
            continue;
        }
        if cursor.selection.end() <= text_range.range.start() {
            break;
        }

        // look for at least one ancestor that applies the style
        let mut styled = false;
        for ancestor in text_range.ancestors {
            if &ast.nodes[ancestor].node_type == style {
                styled = true;
                break;
            }
        }

        if !styled {
            return false;
        }
    }

    true
}

/// Returns true if text in `cursor` has any style which should be removed before applying `style`
fn conflicting_styles(cursor: Cursor, style: &MarkdownNode, ast: &Ast) -> HashSet<MarkdownNode> {
    let mut result = HashSet::new();
    if cursor.selection.is_empty() {
        return result;
    }

    for text_range in ast.iter_text_ranges() {
        // skip ranges before or after the cursor
        if text_range.range.end() <= cursor.selection.start() {
            continue;
        }
        if cursor.selection.end() <= text_range.range.start() {
            break;
        }

        // look for at least one ancestor that applies a conflicting style
        for ancestor in text_range.ancestors {
            if ast.nodes[ancestor]
                .node_type
                .node_type()
                .conflicts_with(&style.node_type())
            {
                result.insert(ast.nodes[ancestor].node_type.clone());
            }
        }
    }

    result
}

/// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
fn apply_style(
    cursor: Cursor, style: MarkdownNode, unapply: bool, buffer: &SubBuffer, ast: &Ast,
    ast_ranges: &AstTextRanges, mutation: &mut Vec<SubMutation>,
) {
    if buffer.is_empty() {
        insert_head(cursor.selection.start(), style.clone(), mutation);
        insert_tail(cursor.selection.start(), style, mutation);
        return;
    }

    // find range containing cursor start and cursor end
    let mut start_range = None;
    let mut end_range = None;
    for text_range in ast.iter_text_ranges() {
        // when at bound, start prefers next
        if text_range
            .range
            .contains_inclusive(cursor.selection.start())
        {
            start_range = Some(text_range.clone());
        }
        // when at bound, end prefers previous unless selection is empty
        if (cursor.selection.is_empty() || end_range.is_none())
            && text_range.range.contains_inclusive(cursor.selection.end())
        {
            end_range = Some(text_range);
        }
    }

    // start always has next because if it were at doc end, selection would be empty (early return above)
    // end always has previous because if it were at doc start, selection would be empty (early return above)
    let start_range = start_range.unwrap();
    let end_range = end_range.unwrap();

    // modify head/tail for nodes containing cursor start and cursor end
    let mut last_start_ancestor: Option<usize> = None;
    if start_range.range_type == AstTextRangeType::Text {
        for &ancestor in &start_range.ancestors {
            // dehead and detail all but the last ancestor applying the style
            if let Some(prev_ancestor) = last_start_ancestor {
                dehead_ast_node(prev_ancestor, ast, mutation);
                detail_ast_node(prev_ancestor, ast, mutation);
            }
            if ast.nodes[ancestor].node_type == style {
                last_start_ancestor = Some(ancestor);
            }
        }
    }
    let mut last_end_ancestor: Option<usize> = None;
    if end_range.range_type == AstTextRangeType::Text {
        for &ancestor in &end_range.ancestors {
            // dehead and detail all but the last ancestor applying the style
            if let Some(prev_ancestor) = last_end_ancestor {
                dehead_ast_node(prev_ancestor, ast, mutation);
                detail_ast_node(prev_ancestor, ast, mutation);
            }
            if ast.nodes[ancestor].node_type == style {
                last_end_ancestor = Some(ancestor);
            }
        }
    }
    if last_start_ancestor != last_end_ancestor {
        // if start and end are in different nodes, detail start and dehead end (remove syntax characters inside selection)
        if let Some(last_start_ancestor) = last_start_ancestor {
            detail_ast_node(last_start_ancestor, ast, mutation);
        }
        if let Some(last_end_ancestor) = last_end_ancestor {
            dehead_ast_node(last_end_ancestor, ast, mutation);
        }
    }
    if unapply {
        // if unapplying, tail or dehead node containing start to crop styled region to selection
        if let Some(last_start_ancestor) = last_start_ancestor {
            if ast.nodes[last_start_ancestor].text_range.start() < cursor.selection.start() {
                insert_tail(cursor.selection.start(), style.clone(), mutation);
            } else {
                dehead_ast_node(last_start_ancestor, ast, mutation);
            }
        }
        // if unapplying, head or detail node containing end to crop styled region to selection
        if let Some(last_end_ancestor) = last_end_ancestor {
            if ast.nodes[last_end_ancestor].text_range.end() > cursor.selection.end() {
                insert_head(cursor.selection.end(), style.clone(), mutation);
            } else {
                detail_ast_node(last_end_ancestor, ast, mutation);
            }
        }
    } else {
        // if applying, head start and/or tail end to extend styled region to selection
        if last_start_ancestor.is_none() {
            insert_head(cursor.selection.start(), style.clone(), mutation)
        }
        if last_end_ancestor.is_none() {
            insert_tail(cursor.selection.end(), style.clone(), mutation)
        }
    }

    // remove head and tail for nodes between nodes containing start and end
    let mut found_start_range = false;
    for text_range in ast.iter_text_ranges() {
        // skip ranges until we pass the range containing the selection start (handled above)
        if text_range == start_range {
            found_start_range = true;
        }
        if !found_start_range {
            continue;
        }

        // stop when we find the range containing the selection end (handled above)
        if text_range == end_range {
            break;
        }

        // dehead and detail nodes with this style in the middle, aside from those already considered
        if text_range.node(ast) == style && text_range.range_type == AstTextRangeType::Text {
            let node_idx = text_range.ancestors.last().copied().unwrap();
            if start_range.ancestors.iter().any(|&a| a == node_idx) {
                continue;
            }
            if end_range.ancestors.iter().any(|&a| a == node_idx) {
                continue;
            }
            dehead_ast_node(node_idx, ast, mutation);
            detail_ast_node(node_idx, ast, mutation);
        }
    }
}

fn dehead_ast_node(node_idx: usize, ast: &Ast, mutation: &mut Vec<SubMutation>) {
    let node = &ast.nodes[node_idx];
    mutation
        .push(SubMutation::Cursor { cursor: (node.range.start(), node.text_range.start()).into() });
    mutation.push(SubMutation::Insert { text: "".to_string(), advance_cursor: true });
}

fn detail_ast_node(node_idx: usize, ast: &Ast, mutation: &mut Vec<SubMutation>) {
    let node = &ast.nodes[node_idx];
    mutation.push(SubMutation::Cursor { cursor: (node.text_range.end(), node.range.end()).into() });
    mutation.push(SubMutation::Insert { text: "".to_string(), advance_cursor: false });
}

fn insert_head(offset: DocCharOffset, style: MarkdownNode, mutation: &mut Vec<SubMutation>) {
    let text = style.node_type().head().to_string();
    mutation.push(SubMutation::Cursor { cursor: offset.into() });
    mutation.push(SubMutation::Insert { text, advance_cursor: true });
}

fn insert_tail(offset: DocCharOffset, style: MarkdownNode, mutation: &mut Vec<SubMutation>) {
    let text = style.node_type().tail().to_string();
    if style.node_type() == MarkdownNodeType::Inline(InlineNodeType::Link) {
        // leave cursor in link tail where you can type the link destination
        mutation.push(SubMutation::Cursor { cursor: offset.into() });
        mutation.push(SubMutation::Insert { text: text[0..2].to_string(), advance_cursor: true });
        mutation.push(SubMutation::Insert {
            text: text[2..text.len()].to_string(),
            advance_cursor: false,
        });
    } else {
        mutation.push(SubMutation::Cursor { cursor: offset.into() });
        mutation.push(SubMutation::Insert { text, advance_cursor: false });
    }
}

pub fn region_to_cursor(
    region: Region, current_cursor: Cursor, buffer: &SubBuffer, galleys: &Galleys, bounds: &Bounds,
) -> Cursor {
    match region {
        Region::Location(location) => {
            location_to_char_offset(location, current_cursor, galleys, &buffer.segs, &bounds.text)
                .into()
        }
        Region::ToLocation(location) => (
            current_cursor.selection.0,
            location_to_char_offset(location, current_cursor, galleys, &buffer.segs, &bounds.text),
        )
            .into(),
        Region::BetweenLocations { start, end } => (
            location_to_char_offset(start, current_cursor, galleys, &buffer.segs, &bounds.text),
            location_to_char_offset(end, current_cursor, galleys, &buffer.segs, &bounds.text),
        )
            .into(),
        Region::Selection => current_cursor,
        Region::SelectionOrOffset { offset, backwards } => {
            if current_cursor.selection().is_none() {
                let mut cursor = current_cursor;
                cursor.advance(offset, backwards, buffer, galleys, bounds);
                cursor.selection.0 = current_cursor.selection.1;
                cursor
            } else {
                current_cursor
            }
        }
        Region::ToOffset { offset, backwards, extend_selection } => {
            if extend_selection
                || current_cursor.selection.is_empty()
                || matches!(offset, Offset::To(..))
            {
                let mut cursor = current_cursor;
                cursor.advance(offset, backwards, buffer, galleys, bounds);
                if extend_selection {
                    cursor.selection.0 = current_cursor.selection.0;
                } else {
                    cursor.selection.0 = cursor.selection.1;
                }
                cursor
            } else if backwards {
                current_cursor.selection.start().into()
            } else {
                current_cursor.selection.end().into()
            }
        }
        Region::Bound { bound, backwards } => {
            let offset = current_cursor.selection.1;
            let range = offset
                .range_bound(bound, backwards, false, bounds)
                .unwrap_or((offset, offset));
            range.into()
        }
        Region::BoundAt { bound, location, backwards } => {
            let offset = location_to_char_offset(
                location,
                current_cursor,
                galleys,
                &buffer.segs,
                &bounds.text,
            );
            let range = offset
                .range_bound(bound, backwards, false, bounds)
                .unwrap_or((offset, offset));
            range.into()
        }
    }
}

pub fn location_to_char_offset(
    location: Location, current_cursor: Cursor, galleys: &Galleys, segs: &UnicodeSegs, text: &Text,
) -> DocCharOffset {
    match location {
        Location::CurrentCursor => current_cursor.selection.1,
        Location::DocCharOffset(o) => o,
        Location::Pos(pos) => pos_to_char_offset(pos, galleys, segs, text),
    }
}

pub fn pos_to_char_offset(
    mut pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs, text: &Text,
) -> DocCharOffset {
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
            if galley.galley_location.min.y <= pos.y {
                if pos.y <= galley.galley_location.max.y {
                    // click position is in a galley
                } else {
                    // click position is between galleys
                    pos.x = galley.galley.rect.max.x;
                }
                let relative_pos = pos - galley.text_location;
                let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                result = galleys.char_offset_by_galley_and_cursor(galley_idx, &new_cursor, text);
            }
        }
        result
    }
}

#[allow(clippy::too_many_arguments)]
pub fn increment_numbered_list_items(
    starting_galley_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    galleys: &Galleys, buffer: &SubBuffer, cursor: Cursor,
) -> Mutation {
    let mut modifications = Vec::new();

    let mut galley_idx = starting_galley_idx;
    loop {
        galley_idx += 1;
        if galley_idx == galleys.len() {
            break;
        }
        let galley = &galleys[galley_idx];
        if let Some(Annotation::Item(item_type, cur_indent_level)) = &galley.annotation {
            match cur_indent_level.cmp(&indent_level) {
                Ordering::Greater => {
                    continue; // skip nested list items
                }
                Ordering::Less => {
                    break; // end of nested list
                }
                Ordering::Equal => {
                    if let ListItem::Numbered(cur_number) = item_type {
                        // replace cur_number with next_number in head
                        modifications.push(SubMutation::Cursor {
                            cursor: (galley.range.start(), galley.range.start() + galley.head_size)
                                .into(),
                        });
                        let head = galley.head(buffer);
                        let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                            .to_string()
                            + &(if !decrement {
                                cur_number.saturating_add(amount)
                            } else {
                                cur_number.saturating_sub(amount)
                            })
                            .to_string()
                            + ". ";
                        modifications.push(SubMutation::Insert { text, advance_cursor: true });
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
