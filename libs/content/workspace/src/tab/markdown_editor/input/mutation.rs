use crate::tab::markdown_editor;
use crate::tab::markdown_editor::bounds::Bounds;
use crate::tab::markdown_editor::style::InlineNode;
use egui::Pos2;
use lb_rs::text::buffer;
use lb_rs::text::buffer::Buffer;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _, ToRangeExt as _};
use lb_rs::text::operation_types::{Operation, Replace};
use lb_rs::text::unicode_segs::UnicodeSegs;
use markdown_editor::ast::{Ast, AstTextRangeType};
use markdown_editor::bounds::{AstTextRanges, RangesExt};
use markdown_editor::bounds::{BoundExt as _, Text};
use markdown_editor::editor::Editor;
use markdown_editor::galleys::Galleys;
use markdown_editor::input::{Event, Location, Offset, Region};
use markdown_editor::layouts::Annotation;
use markdown_editor::style::{
    BlockNode, BlockNodeType, InlineNodeType, ListItem, MarkdownNode, MarkdownNodeType,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::advance::AdvanceExt as _;
use super::Bound;

impl Editor {
    /// Translates editor events into buffer operations by interpreting them in the context of the current editor state.
    /// Dispatches events that aren't buffer operations. Returns a (text_updated, selection_updated) pair.
    pub fn calc_operations(
        &mut self, ctx: &egui::Context, modification: Event, operations: &mut Vec<Operation>,
    ) -> buffer::Response {
        let current_selection = self.buffer.current.selection;
        let mut response = buffer::Response::default();
        match modification {
            Event::Select { region } => {
                operations.push(Operation::Select(self.region_to_range(region)))
            }
            Event::Replace { region, text } => {
                let range = self.region_to_range(region);
                operations.push(Operation::Replace(Replace { range, text }));
                operations.push(Operation::Select(range.start().to_range()))
            }
            Event::ToggleStyle { region, mut style } => {
                let range = self.region_to_range(region);
                let unapply = self.should_unapply(&style);

                // unapply conflicting styles; if replacing a list item with a list item, preserve indentation level and
                // don't remove outer items in nested lists
                let mut removed_conflicting_list_item = false;
                let mut list_item_indent_level = 0;
                if !unapply {
                    for conflict in conflicting_styles(range, &style, &self.ast, &self.bounds.ast) {
                        if let MarkdownNode::Block(BlockNode::ListItem(_, indent_level)) = conflict
                        {
                            if !removed_conflicting_list_item {
                                list_item_indent_level = indent_level;
                                removed_conflicting_list_item = true;
                                self.apply_style(range, conflict, true, operations);
                            }
                        } else {
                            self.apply_style(range, conflict, true, operations);
                        }
                    }
                }
                if let MarkdownNode::Block(BlockNode::ListItem(item_type, _)) = style {
                    style =
                        MarkdownNode::Block(BlockNode::ListItem(item_type, list_item_indent_level));
                };

                // apply style
                self.apply_style(range, style.clone(), unapply, operations);

                // modify cursor
                let mut cursor_modified = false;
                if current_selection.is_empty() {
                    // toggling style at end of styled range moves cursor to outside of styled range
                    if let Some(text_range) = self
                        .bounds
                        .ast
                        .find_containing(current_selection.1, true, true)
                        .iter()
                        .last()
                    {
                        let text_range = &self.bounds.ast[text_range];
                        if text_range.node(&self.ast).node_type() == style.node_type()
                            && text_range.range_type == AstTextRangeType::Tail
                        {
                            operations.push(Operation::Select(text_range.range.end().to_range()));
                            cursor_modified = true;
                        }
                    }
                }
                if !cursor_modified
                    && style.node_type() != MarkdownNodeType::Inline(InlineNodeType::Link)
                {
                    // toggling link style leaves cursor where you can type link destination
                    operations.push(Operation::Select(current_selection));
                }
            }
            Event::Newline { advance_cursor } => {
                let galley_idx = self.galleys.galley_at_char(current_selection.1);
                let galley = &self.galleys[galley_idx];
                let ast_text_range = self
                    .bounds
                    .ast
                    .find_containing(current_selection.1, true, true)
                    .iter()
                    .last();
                let after_galley_head = current_selection.1 >= galley.text_range().start();

                'modification: {
                    if let Some(ast_text_range) = ast_text_range {
                        let ast_text_range = &self.bounds.ast[ast_text_range];
                        if ast_text_range.range_type == AstTextRangeType::Tail
                            && ast_text_range.node(&self.ast).node_type()
                                == MarkdownNodeType::Inline(InlineNodeType::Link)
                            && ast_text_range.range.end() != current_selection.1
                        {
                            // cursor inside link url -> move cursor to end of link
                            operations
                                .push(Operation::Select(ast_text_range.range.end().to_range()));
                            break 'modification;
                        }
                    }

                    // insert new list item, remove current list item, or insert newline before current list item
                    if matches!(galley.annotation, Some(Annotation::Item(..))) && after_galley_head
                    {
                        // cursor at end of list item
                        if galley.size() - galley.head_size - galley.tail_size == 0 {
                            // empty list item -> delete current annotation
                            let range =
                                (galley.range.start(), galley.range.start() + galley.size());
                            let text = "".into();
                            operations.push(Operation::Replace(Replace { range, text }));
                        } else {
                            // nonempty list item -> insert new list item
                            operations.push(Operation::Replace(Replace {
                                range: current_selection,
                                text: "\n".into(),
                            }));

                            match galley.annotation {
                                Some(Annotation::Item(ListItem::Bulleted, _)) => {
                                    operations.push(Operation::Replace(Replace {
                                        range: current_selection,
                                        text: galley.head(&self.buffer).to_string(),
                                    }));
                                }
                                Some(Annotation::Item(
                                    ListItem::Numbered(cur_number),
                                    indent_level,
                                )) => {
                                    let head = galley.head(&self.buffer);
                                    let text = head
                                        [0..head.len() - (cur_number).to_string().len() - 2]
                                        .to_string()
                                        + (&(cur_number + 1).to_string() as &str)
                                        + ". ";
                                    operations.push(Operation::Replace(Replace {
                                        range: current_selection,
                                        text,
                                    }));

                                    let renumbered_galleys = {
                                        let mut this = HashMap::new();
                                        increment_numbered_list_items(
                                            galley_idx,
                                            indent_level,
                                            1,
                                            false,
                                            &self.galleys,
                                            &mut this,
                                        );
                                        this
                                    };
                                    for (galley_idx, galley_new_number) in renumbered_galleys {
                                        let galley = &self.galleys[galley_idx];
                                        if let Some(Annotation::Item(
                                            ListItem::Numbered(galley_cur_number),
                                            ..,
                                        )) = galley.annotation
                                        {
                                            operations.push(Operation::Replace(Replace {
                                                range: (
                                                    galley.range.start() + galley.head_size,
                                                    galley.range.start() + galley.head_size
                                                        - (galley_cur_number).to_string().len()
                                                        - 2,
                                                ),
                                                text: galley_new_number.to_string() + ". ",
                                            }));
                                        }
                                    }
                                }
                                Some(Annotation::Item(ListItem::Todo(_), _)) => {
                                    let head = galley.head(&self.buffer);
                                    operations.push(Operation::Replace(Replace {
                                        range: current_selection,
                                        text: head[0..head.len() - 6].to_string() + "* [ ] ",
                                    }));
                                }
                                Some(Annotation::Image(_, _, _)) => {}
                                Some(Annotation::HeadingRule) => {}
                                Some(Annotation::Rule) => {}
                                None => {}
                            }

                            operations.push(Operation::Select(current_selection));
                        }
                        break 'modification;
                    }

                    // if it's none of the other things, just insert a newline
                    operations.push(Operation::Replace(Replace {
                        range: current_selection,
                        text: "\n".into(),
                    }));
                    if advance_cursor {
                        operations.push(Operation::Select(current_selection.start().to_range()));
                    }
                }
            }
            Event::Delete { region } => {
                let range = self.region_to_range(region);
                operations.push(Operation::Replace(Replace { range, text: "".into() }));
                operations.push(Operation::Select(range.start().to_range()));

                // check if we deleted a numbered list annotation and renumber subsequent items
                let ast_text_ranges = self.bounds.ast.find_contained(range, true, true);
                let mut unnumbered_galleys = HashSet::new();
                let mut renumbered_galleys = HashMap::new();
                for ast_text_range in ast_text_ranges.iter() {
                    // skip non-head ranges; remaining ranges are head ranges contained by the selection
                    if self.bounds.ast[ast_text_range].range_type != AstTextRangeType::Head {
                        continue;
                    }

                    // if the range is a list item annotation contained by the deleted region, renumber subsequent items
                    let ast_node = self.bounds.ast[ast_text_range]
                        .ancestors
                        .last()
                        .copied()
                        .unwrap(); // ast text ranges always have themselves as the last ancestor
                    let galley_idx = self
                        .galleys
                        .galley_at_char(self.ast.nodes[ast_node].text_range.start());
                    if let Some(Annotation::Item(ListItem::Numbered(number), indent_level)) =
                        self.galleys[galley_idx].annotation
                    {
                        renumbered_galleys = HashMap::new(); // only the last one matters; otherwise they stack
                        increment_numbered_list_items(
                            galley_idx,
                            indent_level,
                            number,
                            true,
                            &self.galleys,
                            &mut renumbered_galleys,
                        );
                    }

                    unnumbered_galleys.insert(galley_idx);
                }

                // if we deleted the space between two numbered lists, renumber the second list to extend the first
                let start_galley_idx = self.galleys.galley_at_char(range.start());
                let end_galley_idx = self.galleys.galley_at_char(range.end());
                if start_galley_idx < end_galley_idx {
                    // todo: account for indent levels
                    if let Some(Annotation::Item(ListItem::Numbered(prev_number), _)) =
                        self.galleys[start_galley_idx].annotation
                    {
                        if let Some(Annotation::Item(
                            ListItem::Numbered(next_number),
                            next_indent_level,
                        )) = self
                            .galleys
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
                                &self.galleys,
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

                    let galley = &self.galleys[galley_idx];
                    if let Some(Annotation::Item(ListItem::Numbered(cur_number), ..)) =
                        galley.annotation
                    {
                        operations.push(Operation::Replace(Replace {
                            range: (
                                galley.range.start() + galley.head_size,
                                galley.range.start() + galley.head_size
                                    - (cur_number).to_string().len()
                                    - 2,
                            ),
                            text: new_number.to_string() + ". ",
                        }));
                    }
                }
            }
            Event::Indent { deindent } => {
                // if we're in a list item, tab/shift+tab will indent/de-indent
                // otherwise, tab will insert a tab and shift tab will do nothing
                let mut indentation_processed_galleys = HashSet::new();
                let mut renumbering_processed_galleys = HashSet::new();
                let mut indented_galleys = HashMap::new();
                let mut renumbered_galleys = HashMap::new();

                // determine galleys to (de)indent
                let ast_text_ranges = self.bounds.ast.find_intersecting(current_selection, true);
                for ast_text_range in ast_text_ranges.iter() {
                    let ast_node = self.bounds.ast[ast_text_range]
                        .ancestors
                        .last()
                        .copied()
                        .unwrap(); // ast text ranges always have themselves as the last ancestor
                    let galley_idx = self
                        .galleys
                        .galley_at_char(self.ast.nodes[ast_node].text_range.start());

                    if self.bounds.ast[ast_text_range].range.start() >= current_selection.end() {
                        continue;
                    }

                    let cur_indent_level =
                        if let MarkdownNode::Block(BlockNode::ListItem(_, indent_level)) =
                            self.ast.nodes[ast_node].node_type
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
                    let galley = &self.galleys[galley_idx];
                    let cur_indent_level = indented_galleys[&galley_idx];

                    // todo: this needs more attention e.g. list items doubly indented using 2-space indents
                    // tracked by https://github.com/lockbook/lockbook/issues/1842
                    let galley_text = &(&self.buffer)[(galley.range.start(), galley.range.end())];
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
                        } else if galley_idx != self.galleys.len() - 1 {
                            let next_galley = &self.galleys[galley_idx + 1];
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
                            operations.push(Operation::Replace(Replace {
                                range: (
                                    galley.range.start(),
                                    galley.range.start() + indent_seq.len(),
                                ),
                                text: "".into(),
                            }));

                            cur_indent_level - 1
                        } else {
                            cur_indent_level
                        }
                    } else {
                        let mut can_indent = true;
                        if galley_idx == 0 {
                            can_indent = false; // first galley cannot be indented
                        } else {
                            let prior_galley = &self.galleys[galley_idx - 1];
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
                            operations.push(Operation::Replace(Replace {
                                range: galley.range.start().to_range(),
                                text: indent_seq.to_string(),
                            }));

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
                    let ast_node = self.bounds.ast[ast_text_range]
                        .ancestors
                        .last()
                        .copied()
                        .unwrap(); // ast text ranges always have themselves as the last ancestor
                    let galley_idx = self
                        .galleys
                        .galley_at_char(self.ast.nodes[ast_node].text_range.start());

                    let (cur_number, cur_indent_level) = if let MarkdownNode::Block(
                        BlockNode::ListItem(ListItem::Numbered(cur_number), indent_level),
                    ) = self.ast.nodes[ast_node].node_type
                    {
                        (cur_number, indent_level)
                    } else {
                        continue; // only process numbered list items
                    };
                    let new_indent_level = if let Some(new_indent_level) =
                        indented_galleys.get(&galley_idx).copied()
                    {
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
                            let prior_galley = &self.galleys[prior_galley_idx];
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
                            &self.galleys,
                            &mut renumbered_galleys,
                        );

                        // increment numbers in new nested list by one
                        increment_numbered_list_items(
                            galley_idx,
                            new_indent_level,
                            1,
                            false,
                            &self.galleys,
                            &mut renumbered_galleys,
                        );
                    } else {
                        // decrement numbers in old list by one
                        increment_numbered_list_items(
                            galley_idx,
                            cur_indent_level,
                            1,
                            true,
                            &self.galleys,
                            &mut renumbered_galleys,
                        );

                        // increment numbers in new nested list by this item's new number
                        increment_numbered_list_items(
                            galley_idx,
                            new_indent_level,
                            new_number,
                            false,
                            &self.galleys,
                            &mut renumbered_galleys,
                        );
                    }
                }

                // apply renumber operations once at the end because otherwise they stack up and clobber each other
                for (galley_idx, new_number) in renumbered_galleys {
                    let galley = &self.galleys[galley_idx];
                    if let Some(Annotation::Item(ListItem::Numbered(cur_number), ..)) =
                        galley.annotation
                    {
                        operations.push(Operation::Replace(Replace {
                            range: (
                                galley.range.start() + galley.head_size,
                                galley.range.start() + galley.head_size
                                    - (cur_number).to_string().len()
                                    - 2,
                            ),
                            text: new_number.to_string() + ". ",
                        }));
                    }
                }

                if indentation_processed_galleys.is_empty() && !deindent {
                    operations.push(Operation::Replace(Replace {
                        range: current_selection,
                        text: "\t".into(),
                    }));
                }
            }
            Event::Undo => {
                response |= self.buffer.undo();
            }
            Event::Redo => {
                response |= self.buffer.redo();
            }
            Event::Cut => {
                ctx.output_mut(|o| o.copied_text = self.buffer[current_selection].into());
                operations.push(Operation::Replace(Replace {
                    range: current_selection,
                    text: "".into(),
                }));
            }
            Event::Copy => {
                ctx.output_mut(|o| o.copied_text = self.buffer[current_selection].into());
            }
            Event::OpenUrl(url) => {
                // assume https for urls without a scheme
                let url = if !url.contains("://") { format!("https://{}", url) } else { url };
                ctx.output_mut(|o| o.open_url = Some(egui::output::OpenUrl::new_tab(url)));
            }
            Event::ToggleDebug => self.debug.draw_enabled = !self.debug.draw_enabled,
            Event::IncrementBaseFontSize => {
                self.appearance.base_font_size =
                    self.appearance.base_font_size.map(|size| size + 1.)
            }
            Event::DecrementBaseFontSize => {
                if self.appearance.font_size() > 2. {
                    self.appearance.base_font_size =
                        self.appearance.base_font_size.map(|size| size - 1.)
                }
            }
            Event::ToggleCheckbox(galley_idx) => {
                let galley = &self.galleys[galley_idx];
                if let Some(Annotation::Item(ListItem::Todo(checked), ..)) = galley.annotation {
                    operations.push(Operation::Replace(Replace {
                        range: (
                            galley.range.start() + galley.head_size - 6,
                            galley.range.start() + galley.head_size,
                        ),
                        text: if checked { "* [ ] " } else { "* [x] " }.into(),
                    }));
                }
            }
        }

        response
    }

    /// Returns true if all text in the current selection has style `style`
    fn should_unapply(&self, style: &MarkdownNode) -> bool {
        let current_selection = self.buffer.current.selection;
        if current_selection.is_empty() {
            return false;
        }

        for text_range in &self.bounds.ast {
            // skip ranges before or after the cursor
            if text_range.range.end() <= current_selection.start() {
                continue;
            }
            if current_selection.end() <= text_range.range.start() {
                break;
            }

            // look for at least one ancestor that applies the style
            let mut found_list_item = false;
            for &ancestor in text_range.ancestors.iter().rev() {
                // only consider the innermost list item
                if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..)))
                {
                    if found_list_item {
                        continue;
                    } else {
                        found_list_item = true;
                    }
                }

                // node type must match
                if self.ast.nodes[ancestor].node_type.node_type() != style.node_type() {
                    continue;
                }

                return true;
            }
        }

        false
    }

    /// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
    fn apply_style(
        &self, selection: (DocCharOffset, DocCharOffset), style: MarkdownNode, unapply: bool,
        operations: &mut Vec<Operation>,
    ) {
        if self.buffer.current.text.is_empty() {
            insert_head(selection.start(), style.clone(), operations);
            insert_tail(selection.start(), style, operations);
            return;
        }

        // find range containing cursor start and cursor end
        let mut start_range = None;
        let mut end_range = None;
        for text_range in &self.bounds.ast {
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

            if self.ast.nodes[ancestor].node_type.node_type() == style.node_type() {
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

            if self.ast.nodes[ancestor].node_type.node_type() == style.node_type() {
                last_end_ancestor = Some(ancestor);
            }
        }
        if last_start_ancestor != last_end_ancestor {
            // if start and end are in different nodes, detail start and dehead end (remove syntax characters inside selection)
            if let Some(last_start_ancestor) = last_start_ancestor {
                detail_ast_node(last_start_ancestor, &self.ast, operations);
            }
            if let Some(last_end_ancestor) = last_end_ancestor {
                dehead_ast_node(last_end_ancestor, &self.ast, operations);
            }
        }
        if unapply {
            // if unapplying, tail or dehead node containing start to crop styled region to selection
            if let Some(last_start_ancestor) = last_start_ancestor {
                if self.ast.nodes[last_start_ancestor].text_range.start() < selection.start() {
                    let offset = adjust_for_whitespace(
                        &self.buffer,
                        selection.start(),
                        style.node_type(),
                        true,
                    );
                    insert_tail(offset, style.clone(), operations);
                } else {
                    dehead_ast_node(last_start_ancestor, &self.ast, operations);
                }
            }
            // if unapplying, head or detail node containing end to crop styled region to selection
            if let Some(last_end_ancestor) = last_end_ancestor {
                if self.ast.nodes[last_end_ancestor].text_range.end() > selection.end() {
                    let offset = adjust_for_whitespace(
                        &self.buffer,
                        selection.end(),
                        style.node_type(),
                        false,
                    );
                    insert_head(offset, style.clone(), operations);
                } else {
                    detail_ast_node(last_end_ancestor, &self.ast, operations);
                }
            }
        } else {
            // if applying, head start and/or tail end to extend styled region to selection
            if last_start_ancestor.is_none() {
                let offset = adjust_for_whitespace(
                    &self.buffer,
                    selection.start(),
                    style.node_type(),
                    false,
                )
                .min(selection.end());
                insert_head(offset, style.clone(), operations)
            }
            if last_end_ancestor.is_none() {
                let offset =
                    adjust_for_whitespace(&self.buffer, selection.end(), style.node_type(), true)
                        .max(selection.start());
                insert_tail(offset, style.clone(), operations)
            }
        }

        // remove head and tail for nodes between nodes containing start and end
        let mut found_start_range = false;
        for text_range in &self.bounds.ast {
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
            if text_range.node(&self.ast) == style
                && text_range.range_type == AstTextRangeType::Text
            {
                let node_idx = text_range.ancestors.last().copied().unwrap();
                if start_range.ancestors.iter().any(|&a| a == node_idx) {
                    continue;
                }
                if end_range.ancestors.iter().any(|&a| a == node_idx) {
                    continue;
                }
                dehead_ast_node(node_idx, &self.ast, operations);
                detail_ast_node(node_idx, &self.ast, operations);
            }
        }
    }

    // todo: self by shared reference
    pub fn region_to_range(&mut self, region: Region) -> (DocCharOffset, DocCharOffset) {
        let mut current_selection = self.buffer.current.selection;
        match region {
            Region::Location(location) => self.location_to_char_offset(location).to_range(),
            Region::ToLocation(location) => {
                (current_selection.0, self.location_to_char_offset(location))
            }
            Region::BetweenLocations { start, end } => {
                (self.location_to_char_offset(start), self.location_to_char_offset(end))
            }
            Region::Selection => current_selection,
            Region::SelectionOrOffset { offset, backwards } => {
                if current_selection.is_empty() {
                    current_selection.0 = current_selection.0.advance(
                        &mut self.cursor.x_target,
                        offset,
                        backwards,
                        &self.buffer.current.segs,
                        &self.galleys,
                        &self.bounds,
                    );
                }
                current_selection
            }
            Region::ToOffset { offset, backwards, extend_selection } => {
                if extend_selection
                    || current_selection.is_empty()
                    || matches!(offset, Offset::To(..))
                {
                    let mut selection = current_selection;
                    selection.1 = selection.1.advance(
                        &mut self.cursor.x_target,
                        offset,
                        backwards,
                        &self.buffer.current.segs,
                        &self.galleys,
                        &self.bounds,
                    );
                    if extend_selection {
                        selection.0 = current_selection.0;
                    } else {
                        selection.0 = selection.1;
                    }
                    selection
                } else if backwards {
                    current_selection.start().to_range()
                } else {
                    current_selection.end().to_range()
                }
            }
            Region::Bound { bound, backwards } => {
                let offset = current_selection.1;
                offset
                    .range_bound(bound, backwards, false, &self.bounds)
                    .unwrap_or((offset, offset))
            }
            Region::BoundAt { bound, location, backwards } => {
                let offset = self.location_to_char_offset(location);
                offset
                    .range_bound(bound, backwards, true, &self.bounds)
                    .unwrap_or((offset, offset))
            }
        }
    }

    pub fn location_to_char_offset(&self, location: Location) -> DocCharOffset {
        match location {
            Location::CurrentCursor => self.buffer.current.selection.1,
            Location::DocCharOffset(o) => o,
            Location::Pos(pos) => {
                pos_to_char_offset(pos, &self.galleys, &self.buffer.current.segs, &self.bounds.text)
            }
        }
    }
}

// todo: find a better home along with text & link functions
pub fn pos_to_char_offset(
    mut pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs, text: &Text,
) -> DocCharOffset {
    if !galleys.is_empty() && pos.y < galleys[0].rect.min.y {
        // click position is above first galley
        0.into()
    } else if !galleys.is_empty() && pos.y >= galleys[galleys.len() - 1].rect.max.y {
        // click position is below last galley
        segs.last_cursor_position()
    } else {
        let mut result = 0.into();
        for galley_idx in 0..galleys.len() {
            let galley = &galleys[galley_idx];
            if pos.y <= galley.rect.max.y {
                if galley.rect.min.y <= pos.y {
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

pub fn pos_to_galley(
    pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs, bounds: &Bounds,
) -> Option<usize> {
    for (galley_idx, galley) in galleys.galleys.iter().enumerate() {
        if galley.rect.contains(pos) {
            // galleys stretch across the screen, so we need to check if we're to the right of the text
            // use a tolerance of 10.0 for x and a tolerance of one line for y (supports noncapture when pointer is over a code block)
            let offset = pos_to_char_offset(pos, galleys, segs, &bounds.text);

            let prev_line_end_pos_x = {
                let line_start_offset = offset
                    .advance_to_bound(Bound::Line, true, bounds)
                    .advance_to_next_bound(Bound::Line, true, bounds);
                let line_end_offset =
                    line_start_offset.advance_to_bound(Bound::Line, false, bounds);
                let (_, egui_cursor) =
                    galleys.galley_and_cursor_by_char_offset(line_end_offset, &bounds.text);
                galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
            };
            let curr_line_end_pos_x = {
                let line_end_offset = offset.advance_to_bound(Bound::Line, false, bounds);
                let (_, egui_cursor) =
                    galleys.galley_and_cursor_by_char_offset(line_end_offset, &bounds.text);
                galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
            };
            let next_line_end_pos_x = {
                let line_end_offset = offset
                    .advance_to_bound(Bound::Line, false, bounds)
                    .advance_to_next_bound(Bound::Line, false, bounds);
                let (_, egui_cursor) =
                    galleys.galley_and_cursor_by_char_offset(line_end_offset, &bounds.text);
                galley.galley.pos_from_cursor(&egui_cursor).max.x + galley.text_location.x
            };

            let max_pos_x = prev_line_end_pos_x
                .max(curr_line_end_pos_x)
                .max(next_line_end_pos_x);
            let tolerance = 10.0;
            return if max_pos_x + tolerance > pos.x { Some(galley_idx) } else { None };
        }
    }
    None
}

pub fn pos_to_link(
    pos: Pos2, galleys: &Galleys, buffer: &Buffer, bounds: &Bounds, ast: &Ast,
) -> Option<String> {
    pos_to_galley(pos, galleys, &buffer.current.segs, bounds)?;
    let offset = pos_to_char_offset(pos, galleys, &buffer.current.segs, &bounds.text);

    // todo: binary search
    for ast_node in &ast.nodes {
        if let MarkdownNode::Inline(InlineNode::Link(_, url, _)) = &ast_node.node_type {
            if ast_node.range.contains_inclusive(offset) {
                return Some(url.to_string());
            }
        }
    }
    for plaintext_link in &bounds.links {
        if plaintext_link.contains_inclusive(offset) {
            return Some(buffer[*plaintext_link].to_string());
        }
    }

    None
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

// appends operations to `mutation` to renumber list items and returns numbers assigned to each galley
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

fn dehead_ast_node(node_idx: usize, ast: &Ast, operations: &mut Vec<Operation>) {
    let node = &ast.nodes[node_idx];
    operations.push(Operation::Replace(Replace {
        range: (node.range.start(), node.text_range.start()),
        text: "".into(),
    }));
}

fn detail_ast_node(node_idx: usize, ast: &Ast, operations: &mut Vec<Operation>) {
    let node = &ast.nodes[node_idx];
    operations.push(Operation::Replace(Replace {
        range: (node.text_range.end(), node.range.end()),
        text: "".into(),
    }));
}

fn adjust_for_whitespace(
    buffer: &Buffer, mut offset: DocCharOffset, style: MarkdownNodeType, tail: bool,
) -> DocCharOffset {
    if matches!(style, MarkdownNodeType::Inline(..)) {
        loop {
            let c = if tail {
                if offset == 0 {
                    break;
                }
                &buffer[(offset - 1, offset)]
            } else {
                if offset == buffer.current.segs.last_cursor_position() {
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

fn insert_head(offset: DocCharOffset, style: MarkdownNode, operations: &mut Vec<Operation>) {
    let text = style.head();
    operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
}

fn insert_tail(offset: DocCharOffset, style: MarkdownNode, operations: &mut Vec<Operation>) {
    let text = style.node_type().tail().to_string();
    if style.node_type() == MarkdownNodeType::Inline(InlineNodeType::Link) {
        operations
            .push(Operation::Replace(Replace { range: offset.to_range(), text: text[..2].into() }));
        operations.push(Operation::Select(offset.to_range()));
        operations
            .push(Operation::Replace(Replace { range: offset.to_range(), text: text[2..].into() }));
    } else {
        operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
    }
}
