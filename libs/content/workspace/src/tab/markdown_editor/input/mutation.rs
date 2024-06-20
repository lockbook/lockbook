use crate::tab::markdown_editor::ast::{Ast, AstTextRangeType};
use crate::tab::markdown_editor::bounds::{AstTextRanges, Bounds, RangesExt, Text};
use crate::tab::markdown_editor::buffer::{EditorMutation, SubBuffer, SubMutation};
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input::canonical::{Location, Modification, Offset, Region};
use crate::tab::markdown_editor::input::cursor::Cursor;
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::offset_types::{DocCharOffset, RangeExt, RangeIterExt};
use crate::tab::markdown_editor::style::{
    BlockNode, BlockNodeType, InlineNodeType, ListItem, MarkdownNode, MarkdownNodeType,
};
use crate::tab::markdown_editor::unicode_segs::UnicodeSegs;
use egui::Pos2;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
        Modification::ToggleStyle { region, mut style } => {
            let cursor = region_to_cursor(region, current_cursor, buffer, galleys, bounds);
            let unapply = should_unapply(cursor, &style, ast, &bounds.ast);

            // unapply conflicting styles; if replacing a list item with a list item, preserve indentation level and
            // don't remove outer items in nested lists
            let mut removed_conflicting_list_item = false;
            let mut list_item_indent_level = 0;
            if !unapply {
                for conflict in conflicting_styles(cursor.selection, &style, ast, &bounds.ast) {
                    if let MarkdownNode::Block(BlockNode::ListItem(_, indent_level)) = conflict {
                        if !removed_conflicting_list_item {
                            list_item_indent_level = indent_level;
                            removed_conflicting_list_item = true;
                            apply_style(
                                cursor.selection,
                                conflict,
                                true,
                                buffer,
                                ast,
                                &bounds.ast,
                                &mut mutation,
                            );
                        }
                    } else {
                        apply_style(
                            cursor.selection,
                            conflict,
                            true,
                            buffer,
                            ast,
                            &bounds.ast,
                            &mut mutation,
                        );
                    }
                }
            }
            if let MarkdownNode::Block(BlockNode::ListItem(item_type, _)) = style {
                style = MarkdownNode::Block(BlockNode::ListItem(item_type, list_item_indent_level));
            };

            // apply style
            apply_style(
                cursor.selection,
                style.clone(),
                unapply,
                buffer,
                ast,
                &bounds.ast,
                &mut mutation,
            );

            // modify cursor
            if current_cursor.selection.is_empty() {
                // toggling style at end of styled range moves cursor to outside of styled range
                if let Some(text_range) = bounds
                    .ast
                    .find_containing(current_cursor.selection.1, true, true)
                    .iter()
                    .last()
                {
                    let text_range = &bounds.ast[text_range];
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
            let ast_text_range = bounds
                .ast
                .find_containing(current_cursor.selection.1, true, true)
                .iter()
                .last();
            let after_galley_head = current_cursor.selection.1 >= galley.text_range().start();

            'modification: {
                if let Some(ast_text_range) = ast_text_range {
                    let ast_text_range = &bounds.ast[ast_text_range];
                    if ast_text_range.range_type == AstTextRangeType::Tail
                        && ast_text_range.node(ast).node_type()
                            == MarkdownNodeType::Inline(InlineNodeType::Link)
                        && ast_text_range.range.end() != cursor.selection.1
                    {
                        // cursor inside link url -> move cursor to end of link
                        mutation.push(SubMutation::Cursor {
                            cursor: (ast_text_range.range.end(), ast_text_range.range.end()).into(),
                        });
                        break 'modification;
                    }
                }

                // insert new list item, remove current list item, or insert newline before current list item
                if matches!(galley.annotation, Some(Annotation::Item(..))) && after_galley_head {
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
                        mutation.push(SubMutation::Insert {
                            text: "\n".to_string(),
                            advance_cursor: true,
                        });

                        match galley.annotation {
                            Some(Annotation::Item(ListItem::Bulleted, _)) => {
                                mutation.push(SubMutation::Insert {
                                    text: galley.head(buffer).to_string(),
                                    advance_cursor: true,
                                });
                            }
                            Some(Annotation::Item(
                                ListItem::Numbered(cur_number),
                                indent_level,
                            )) => {
                                let head = galley.head(buffer);
                                let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                                    .to_string()
                                    + (&(cur_number + 1).to_string() as &str)
                                    + ". ";
                                mutation.push(SubMutation::Insert { text, advance_cursor: true });

                                let renumbered_galleys = {
                                    let mut this = HashMap::new();
                                    increment_numbered_list_items(
                                        galley_idx,
                                        indent_level,
                                        1,
                                        false,
                                        galleys,
                                        &mut this,
                                    );
                                    this
                                };
                                for (galley_idx, galley_new_number) in renumbered_galleys {
                                    let galley = &galleys[galley_idx];
                                    if let Some(Annotation::Item(
                                        ListItem::Numbered(galley_cur_number),
                                        ..,
                                    )) = galley.annotation
                                    {
                                        mutation.push(SubMutation::Cursor {
                                            cursor: (
                                                galley.range.start() + galley.head_size,
                                                galley.range.start() + galley.head_size
                                                    - (galley_cur_number).to_string().len()
                                                    - 2,
                                            )
                                                .into(),
                                        });
                                        mutation.push(SubMutation::Insert {
                                            text: galley_new_number.to_string() + ". ",
                                            advance_cursor: true,
                                        });
                                        mutation
                                            .push(SubMutation::Cursor { cursor: current_cursor });
                                    }
                                }
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
                    break 'modification;
                } else if cursor.selection.1 == galley.range.start() + galley.head_size
                    && !matches!(galley.annotation, Some(Annotation::Item(..)))
                {
                    // cursor at start of non-list item -> insert newline before annotation
                    mutation.push(SubMutation::Cursor { cursor: galley.range.start().into() });
                    mutation
                        .push(SubMutation::Insert { text: "\n".to_string(), advance_cursor: true });
                    mutation.push(SubMutation::Cursor { cursor });
                    break 'modification;
                }

                // if it's none of the other things, just insert a newline
                mutation.push(SubMutation::Insert { text: "\n".to_string(), advance_cursor });
            }

            cursor.selection.0 = cursor.selection.1;
        }
        Modification::Delete { region } => {
            let region_cursor = region_to_cursor(region, current_cursor, buffer, galleys, bounds);

            mutation.push(SubMutation::Cursor { cursor: region_cursor });
            mutation.push(SubMutation::Delete(0.into()));
            mutation.push(SubMutation::Cursor { cursor: current_cursor });

            // check if we deleted a numbered list annotation and renumber subsequent items
            let ast_text_ranges = bounds
                .ast
                .find_contained(region_cursor.selection, true, true);
            let mut unnumbered_galleys = HashSet::new();
            let mut renumbered_galleys = HashMap::new();
            for ast_text_range in ast_text_ranges.iter() {
                // skip non-head ranges; remaining ranges are head ranges contained by the selection
                if bounds.ast[ast_text_range].range_type != AstTextRangeType::Head {
                    continue;
                }

                // if the range is a list item annotation contained by the deleted region, renumber subsequent items
                let ast_node = bounds.ast[ast_text_range]
                    .ancestors
                    .last()
                    .copied()
                    .unwrap(); // ast text ranges always have themselves as the last ancestor
                let galley_idx = galleys.galley_at_char(ast.nodes[ast_node].text_range.start());
                if let Some(Annotation::Item(ListItem::Numbered(number), indent_level)) =
                    galleys[galley_idx].annotation
                {
                    renumbered_galleys = HashMap::new(); // only the last one matters; otherwise they stack
                    increment_numbered_list_items(
                        galley_idx,
                        indent_level,
                        number,
                        true,
                        galleys,
                        &mut renumbered_galleys,
                    );
                }

                unnumbered_galleys.insert(galley_idx);
            }

            // if we deleted the space between two numbered lists, renumber the second list to extend the first
            let start_galley_idx = galleys.galley_at_char(region_cursor.selection.start());
            let end_galley_idx = galleys.galley_at_char(region_cursor.selection.end());
            if start_galley_idx < end_galley_idx {
                // todo: account for indent levels
                if let Some(Annotation::Item(ListItem::Numbered(prev_number), _)) =
                    galleys[start_galley_idx].annotation
                {
                    if let Some(Annotation::Item(
                        ListItem::Numbered(next_number),
                        next_indent_level,
                    )) = galleys
                        .galleys
                        .get(end_galley_idx + 1)
                        .and_then(|g| g.annotation.as_ref())
                    {
                        let (amount, decrement) = if prev_number >= *next_number {
                            (prev_number - next_number + 1, false)
                        } else {
                            (next_number - prev_number - 1, true)
                        };

                        renumbered_galleys = HashMap::new(); // only the last one matters; otherwise they stack
                        increment_numbered_list_items(
                            end_galley_idx,
                            *next_indent_level,
                            amount,
                            decrement,
                            galleys,
                            &mut renumbered_galleys,
                        );
                    }
                }
            }

            // apply renumber operations once at the end because otherwise they stack up and clobber each other
            for (galley_idx, new_number) in renumbered_galleys {
                // don't number items that were deleted
                if unnumbered_galleys.contains(&galley_idx) {
                    continue;
                }

                let galley = &galleys[galley_idx];
                if let Some(Annotation::Item(ListItem::Numbered(cur_number), ..)) =
                    galley.annotation
                {
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
                }
            }
        }
        Modification::Indent { deindent } => {
            // if we're in a list item, tab/shift+tab will indent/de-indent
            // otherwise, tab will insert a tab and shift tab will do nothing
            let mut indentation_processed_galleys = HashSet::new();
            let mut renumbering_processed_galleys = HashSet::new();
            let mut indented_galleys = HashMap::new();
            let mut renumbered_galleys = HashMap::new();

            // determine galleys to (de)indent
            let ast_text_ranges = bounds.ast.find_intersecting(current_cursor.selection, true);
            for ast_text_range in ast_text_ranges.iter() {
                let ast_node = bounds.ast[ast_text_range]
                    .ancestors
                    .last()
                    .copied()
                    .unwrap(); // ast text ranges always have themselves as the last ancestor
                let galley_idx = galleys.galley_at_char(ast.nodes[ast_node].text_range.start());

                if bounds.ast[ast_text_range].range.start() >= current_cursor.selection.end() {
                    continue;
                }

                let cur_indent_level =
                    if let MarkdownNode::Block(BlockNode::ListItem(_, indent_level)) =
                        ast.nodes[ast_node].node_type
                    {
                        indent_level
                    } else {
                        continue; // only process list items
                    };
                if !indentation_processed_galleys.insert(galley_idx) {
                    continue; // only process each galley once
                }

                indented_galleys.insert(galley_idx, cur_indent_level);
            }

            // (de)indent identified galleys in order
            // iterate forwards for indent and backwards for de-indent because when indenting, the indentation of the
            // prior item constraints the indentation of the current item, and when de-indenting, the indentation of
            // the next item constraints the indentation of the current item
            let ordered_galleys = {
                let mut this = Vec::new();
                this.extend(indented_galleys.keys());
                this.sort();
                if deindent {
                    this.reverse();
                }
                this
            };
            for galley_idx in ordered_galleys {
                let galley = &galleys[galley_idx];
                let cur_indent_level = indented_galleys[&galley_idx];

                // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                // tracked by https://github.com/lockbook/lockbook/issues/1842
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
                    if cur_indent_level == 0 {
                        can_deindent = false; // cannot de-indent un-indented list item
                    } else if galley_idx != galleys.len() - 1 {
                        let next_galley = &galleys[galley_idx + 1];
                        if let Some(Annotation::Item(.., next_indent_level)) =
                            &next_galley.annotation
                        {
                            let next_indent_level = indented_galleys
                                .get(&(galley_idx + 1))
                                .copied()
                                .unwrap_or(*next_indent_level);
                            if next_indent_level > cur_indent_level {
                                can_deindent = false; // list item cannot be de-indented if already indented less than next item
                            }
                        }
                    }

                    if can_deindent {
                        // de-indentation: select text, delete selection, restore cursor
                        mutation.push(SubMutation::Cursor {
                            cursor: (galley.range.start(), galley.range.start() + indent_seq.len())
                                .into(),
                        });
                        mutation.push(SubMutation::Delete(0.into()));
                        mutation.push(SubMutation::Cursor { cursor: current_cursor });

                        cur_indent_level - 1
                    } else {
                        cur_indent_level
                    }
                } else {
                    let mut can_indent = true;
                    if galley_idx == 0 {
                        can_indent = false; // first galley cannot be indented
                    } else {
                        let prior_galley = &galleys[galley_idx - 1];
                        if let Some(Annotation::Item(_, prior_indent_level)) =
                            &prior_galley.annotation
                        {
                            let prior_indent_level = indented_galleys
                                .get(&(galley_idx - 1))
                                .copied()
                                .unwrap_or(*prior_indent_level);
                            if prior_indent_level < cur_indent_level {
                                can_indent = false; // list item cannot be indented if already indented more than prior item
                            }
                        } else {
                            can_indent = false; // first list item of a list cannot be indented
                        }
                    }

                    if can_indent {
                        // indentation: set cursor to galley start, insert indentation sequence, restore cursor
                        mutation.push(SubMutation::Cursor { cursor: galley.range.start().into() });
                        mutation.push(SubMutation::Insert {
                            text: indent_seq.to_string(),
                            advance_cursor: true,
                        });
                        mutation.push(SubMutation::Cursor { cursor: current_cursor });

                        cur_indent_level + 1
                    } else {
                        cur_indent_level
                    }
                };

                if new_indent_level != cur_indent_level {
                    indented_galleys.insert(galley_idx, new_indent_level);
                }
            }

            // always iterate forwards when renumbering because numbers are based on prior numbers for both indent
            // and deindent operations
            for ast_text_range in ast_text_ranges.iter() {
                let ast_node = bounds.ast[ast_text_range]
                    .ancestors
                    .last()
                    .copied()
                    .unwrap(); // ast text ranges always have themselves as the last ancestor
                let galley_idx = galleys.galley_at_char(ast.nodes[ast_node].text_range.start());

                let (cur_number, cur_indent_level) = if let MarkdownNode::Block(
                    BlockNode::ListItem(ListItem::Numbered(cur_number), indent_level),
                ) = ast.nodes[ast_node].node_type
                {
                    (cur_number, indent_level)
                } else {
                    continue; // only process numbered list items
                };
                let new_indent_level =
                    if let Some(new_indent_level) = indented_galleys.get(&galley_idx).copied() {
                        new_indent_level
                    } else {
                        continue; // only process indented galleys
                    };
                if !renumbering_processed_galleys.insert(galley_idx) {
                    continue; // only process each galley once
                }

                // re-number numbered lists
                let cur_number = renumbered_galleys
                    .get(&galley_idx)
                    .copied()
                    .unwrap_or(cur_number);

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
                            // if prior galley has already been processed, use its new indent level and number
                            let prior_indent_level = indented_galleys
                                .get(&prior_galley_idx)
                                .copied()
                                .unwrap_or(prior_indent_level);
                            let prior_number = renumbered_galleys
                                .get(&prior_galley_idx)
                                .copied()
                                .unwrap_or(prior_number);

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

                    renumbered_galleys.insert(galley_idx, new_number);

                    new_number
                };

                renumbered_galleys.insert(galley_idx, new_number);

                if deindent {
                    // decrement numbers in old list by this item's old number
                    increment_numbered_list_items(
                        galley_idx,
                        cur_indent_level,
                        cur_number,
                        true,
                        galleys,
                        &mut renumbered_galleys,
                    );

                    // increment numbers in new nested list by one
                    increment_numbered_list_items(
                        galley_idx,
                        new_indent_level,
                        1,
                        false,
                        galleys,
                        &mut renumbered_galleys,
                    );
                } else {
                    // decrement numbers in old list by one
                    increment_numbered_list_items(
                        galley_idx,
                        cur_indent_level,
                        1,
                        true,
                        galleys,
                        &mut renumbered_galleys,
                    );

                    // increment numbers in new nested list by this item's new number
                    increment_numbered_list_items(
                        galley_idx,
                        new_indent_level,
                        new_number,
                        false,
                        galleys,
                        &mut renumbered_galleys,
                    );
                }
            }

            // apply renumber operations once at the end because otherwise they stack up and clobber each other
            for (galley_idx, new_number) in renumbered_galleys {
                let galley = &galleys[galley_idx];
                if let Some(Annotation::Item(ListItem::Numbered(cur_number), ..)) =
                    galley.annotation
                {
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
                }
            }

            if indentation_processed_galleys.is_empty() && !deindent {
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
fn should_unapply(
    cursor: Cursor, style: &MarkdownNode, ast: &Ast, ast_ranges: &AstTextRanges,
) -> bool {
    if cursor.selection.is_empty() {
        return false;
    }

    for text_range in ast_ranges {
        // skip ranges before or after the cursor
        if text_range.range.end() <= cursor.selection.start() {
            continue;
        }
        if cursor.selection.end() <= text_range.range.start() {
            break;
        }

        // look for at least one ancestor that applies the style
        let mut found_list_item = false;
        for &ancestor in text_range.ancestors.iter().rev() {
            // only consider the innermost list item
            if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
                if found_list_item {
                    continue;
                } else {
                    found_list_item = true;
                }
            }

            // node type must match
            if ast.nodes[ancestor].node_type.node_type() != style.node_type() {
                continue;
            }

            return true;
        }
    }

    false
}

/// Returns list of nodes whose styles should be removed before applying `style`
fn conflicting_styles(
    selection: (DocCharOffset, DocCharOffset), style: &MarkdownNode, ast: &Ast,
    ast_ranges: &AstTextRanges,
) -> Vec<MarkdownNode> {
    let mut result = Vec::new();
    let mut dedup_set = HashSet::new();
    if selection.is_empty() {
        return result;
    }

    for text_range in ast_ranges {
        // skip ranges before or after the cursor
        if text_range.range.end() <= selection.start() {
            continue;
        }
        if selection.end() <= text_range.range.start() {
            break;
        }

        // look for ancestors that apply a conflicting style
        let mut found_list_item = false;
        for &ancestor in text_range.ancestors.iter().rev() {
            let node = &ast.nodes[ancestor].node_type;

            // only remove the innermost conflicting list item
            if matches!(node.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
                if found_list_item {
                    continue;
                } else {
                    found_list_item = true;
                }
            }

            if node.node_type().conflicts_with(&style.node_type()) && dedup_set.insert(node.clone())
            {
                result.push(node.clone());
            }
        }
    }

    result
}

/// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
fn apply_style(
    selection: (DocCharOffset, DocCharOffset), style: MarkdownNode, unapply: bool,
    buffer: &SubBuffer, ast: &Ast, ast_ranges: &AstTextRanges, mutation: &mut Vec<SubMutation>,
) {
    if buffer.is_empty() {
        insert_head(selection.start(), style.clone(), mutation);
        insert_tail(selection.start(), style, mutation);
        return;
    }

    // find range containing cursor start and cursor end
    let mut start_range = None;
    let mut end_range = None;
    for text_range in ast_ranges {
        // when at bound, start prefers next
        if text_range.range.contains_inclusive(selection.start()) {
            start_range = Some(text_range.clone());
        }
        // when at bound, end prefers previous unless selection is empty
        if (selection.is_empty() || end_range.is_none())
            && text_range.range.contains_inclusive(selection.end())
        {
            end_range = Some(text_range);
        }
    }

    // start always has next because if it were at doc end, selection would be empty (early return above)
    // end always has previous because if it were at doc start, selection would be empty (early return above)
    let start_range = start_range.unwrap();
    let end_range = end_range.unwrap();

    // find nodes applying given style containing cursor start and cursor end
    // consider only innermost list items
    let mut found_list_item = false;
    let mut last_start_ancestor: Option<usize> = None;
    for &ancestor in start_range.ancestors.iter().rev() {
        if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
            if found_list_item {
                continue;
            } else {
                found_list_item = true;
            }
        }

        if ast.nodes[ancestor].node_type.node_type() == style.node_type() {
            last_start_ancestor = Some(ancestor);
        }
    }
    found_list_item = false;
    let mut last_end_ancestor: Option<usize> = None;
    for &ancestor in end_range.ancestors.iter().rev() {
        if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
            if found_list_item {
                continue;
            } else {
                found_list_item = true;
            }
        }

        if ast.nodes[ancestor].node_type.node_type() == style.node_type() {
            last_end_ancestor = Some(ancestor);
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
            if ast.nodes[last_start_ancestor].text_range.start() < selection.start() {
                let offset =
                    adjust_for_whitespace(buffer, selection.start(), style.node_type(), true);
                insert_tail(offset, style.clone(), mutation);
            } else {
                dehead_ast_node(last_start_ancestor, ast, mutation);
            }
        }
        // if unapplying, head or detail node containing end to crop styled region to selection
        if let Some(last_end_ancestor) = last_end_ancestor {
            if ast.nodes[last_end_ancestor].text_range.end() > selection.end() {
                let offset =
                    adjust_for_whitespace(buffer, selection.end(), style.node_type(), false);
                insert_head(offset, style.clone(), mutation);
            } else {
                detail_ast_node(last_end_ancestor, ast, mutation);
            }
        }
    } else {
        // if applying, head start and/or tail end to extend styled region to selection
        if last_start_ancestor.is_none() {
            let offset = adjust_for_whitespace(buffer, selection.start(), style.node_type(), false)
                .min(selection.end());
            insert_head(offset, style.clone(), mutation)
        }
        if last_end_ancestor.is_none() {
            let offset = adjust_for_whitespace(buffer, selection.end(), style.node_type(), true)
                .max(selection.start());
            insert_tail(offset, style.clone(), mutation)
        }
    }

    // remove head and tail for nodes between nodes containing start and end
    let mut found_start_range = false;
    for text_range in ast_ranges {
        // skip ranges until we pass the range containing the selection start (handled above)
        if text_range == &start_range {
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

fn adjust_for_whitespace(
    buffer: &SubBuffer, mut offset: DocCharOffset, style: MarkdownNodeType, tail: bool,
) -> DocCharOffset {
    if matches!(style, MarkdownNodeType::Inline(..)) {
        loop {
            let c = if tail {
                if offset == 0 {
                    break;
                }
                &buffer[(offset - 1, offset)]
            } else {
                if offset == buffer.segs.last_cursor_position() {
                    break;
                }
                &buffer[(offset, offset + 1)]
            };
            if c == " " {
                if tail {
                    offset -= 1
                } else {
                    offset += 1
                }
            } else {
                break;
            }
        }
    }
    offset
}

fn insert_head(offset: DocCharOffset, style: MarkdownNode, mutation: &mut Vec<SubMutation>) {
    let text = style.head();
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
            if pos.y <= galley.galley_location.max.y {
                if galley.galley_location.min.y <= pos.y {
                    // click position is in a galley
                } else {
                    // click position is between galleys
                    pos.x = galley.galley.rect.max.x;
                }
                let relative_pos = pos - galley.text_location;
                let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                result = galleys.char_offset_by_galley_and_cursor(galley_idx, &new_cursor, text);
                break;
            }
        }
        result
    }
}

// appends modifications to `mutation` to renumber list items and returns numbers assigned to each galley
fn increment_numbered_list_items(
    starting_galley_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    galleys: &Galleys, renumbers: &mut HashMap<usize, usize>,
) {
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
                        // if galley has already been processed, use its most recently assigned number
                        let cur_number = renumbers.get(&galley_idx).unwrap_or(cur_number);

                        // replace cur_number with next_number in head
                        let new_number = if !decrement {
                            cur_number.saturating_add(amount)
                        } else {
                            cur_number.saturating_sub(amount)
                        };

                        renumbers.insert(galley_idx, new_number);
                    }
                }
            }
        } else {
            break;
        }
    }
}
