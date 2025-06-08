use crate::tab::markdown_plusplus::bounds::{BoundExt as _, RangesExt as _, Text};
use crate::tab::markdown_plusplus::galleys::Galleys;
use crate::tab::markdown_plusplus::input::Event;
use crate::tab::markdown_plusplus::widget::ROW_SPACING;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;
use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Rangef, Vec2};
use lb_rs::model::text::buffer::{self};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt, RangeExt as _, RangeIterExt, ToRangeExt as _,
};
use lb_rs::model::text::operation_types::{Operation, Replace};

use super::advance::AdvanceExt as _;
use super::{Bound, Location, Offset, Region};

/// tracks editor state necessary to support translating input events to buffer operations
#[derive(Default)]
pub struct EventState {
    prev_event: Option<Event>,
    pub internal_events: Vec<Event>,
}

impl<'ast> MarkdownPlusPlus {
    /// Translates editor events into buffer operations by interpreting them in the context of the current editor state.
    /// Dispatches events that aren't buffer operations. Returns a (text_updated, selection_updated) pair.
    pub fn calc_operations(
        &mut self, ctx: &egui::Context, root: &'ast AstNode<'ast>, event: Event,
        operations: &mut Vec<Operation>,
    ) -> buffer::Response {
        let current_selection = self.buffer.current.selection;
        let mut response = buffer::Response::default();
        // let prev_event_eq = self.event.prev_event.as_ref() == Some(&event);
        self.event.prev_event = Some(event.clone());
        match event {
            Event::Select { region } => {
                operations.push(Operation::Select(self.region_to_range(region)));
            }
            Event::Replace { region, text } => {
                let range = self.region_to_range(region);
                operations.push(Operation::Replace(Replace { range, text }));
                operations.push(Operation::Select(range.start().to_range()));
            }
            Event::ToggleStyle { region, style } => {
                // let range = self.region_to_range(region);
                // let unapply = self.should_unapply(&style);

                // if !unapply && prev_event_eq {
                //     // Markdown doesn't recognize empty list items on a line after text or empty styled text ranges.
                //     // It's annoying to hit cmd+b repeatedly and have it continuously add symbols that aren't parsed.
                //     // This hack makes a second consecutive style toggle just undo the first.
                //     // This hack has less-than-ideal implications when toggling style for a selection that includes
                //     // some text already having that style and some not, where toggling the style twice shouldn't
                //     // neccesarily undo the first toggle.
                //     // We can remove this when we implement our own markdown parser.
                //     response |= self.buffer.undo();
                //     self.event.prev_event = None;
                // } else {
                //     // unapply conflicting styles; if replacing a list item with a list item, preserve indentation level and
                //     // don't remove outer items in nested lists
                //     let mut removed_conflicting_list_item = false;
                //     let mut list_item_indent_level = 0;
                //     if !unapply {
                //         for conflict in
                //             conflicting_styles(range, &style, &self.ast, &self.bounds.ast)
                //         {
                //             if let MarkdownNode::Block(BlockNode::ListItem(_, indent_level)) =
                //                 conflict
                //             {
                //                 if !removed_conflicting_list_item {
                //                     list_item_indent_level = indent_level;
                //                     removed_conflicting_list_item = true;
                //                     self.apply_style(range, conflict, true, operations);
                //                 }
                //             } else {
                //                 self.apply_style(range, conflict, true, operations);
                //             }
                //         }
                //     }
                //     if let MarkdownNode::Block(BlockNode::ListItem(item_type, _)) = style {
                //         style = MarkdownNode::Block(BlockNode::ListItem(
                //             item_type,
                //             list_item_indent_level,
                //         ));
                //     };

                //     // apply style
                //     self.apply_style(range, style.clone(), unapply, operations);

                //     // modify cursor
                //     if current_selection.is_empty() {
                //         // toggling style at end of styled range moves cursor to outside of styled range
                //         if let Some(text_range) = self
                //             .bounds
                //             .ast
                //             .find_containing(current_selection.1, true, true)
                //             .iter()
                //             .last()
                //         {
                //             let text_range = &self.bounds.ast[text_range];
                //             if text_range.node(&self.ast).node_type() == style.node_type()
                //                 && text_range.range_type == AstTextRangeType::Tail
                //             {
                //                 operations
                //                     .push(Operation::Select(text_range.range.end().to_range()));
                //             }
                //         }
                //     }
                // }
            }
            Event::Newline { shift } => {
                // insert/extend/terminate container blocks
                let mut handled = || {
                    // selection must be empty
                    let offset = if current_selection.is_empty() {
                        current_selection.0
                    } else {
                        return false;
                    };

                    let node = self.container_block_descendant_at_offset(root, offset);

                    if matches!(
                        node.data.borrow().value,
                        NodeValue::List(_) | NodeValue::Table(_) | NodeValue::TableRow(_)
                    ) {
                        return false;
                    }

                    let (line_idx, _) =
                        self.bounds.source_lines.find_containing(offset, true, true);
                    let line = self.bounds.source_lines[line_idx];

                    let prefix_len = self.line_prefix_len(node, line);
                    let prefix = (line.start(), line.start() + prefix_len);
                    let content = (prefix.end(), line.end());

                    if shift {
                        // shift -> extend
                        let extension_prefix = match &node.data.borrow().value {
                            NodeValue::FrontMatter(_) | NodeValue::Raw(_) => {
                                unreachable!("not a container block")
                            }

                            // container_block
                            NodeValue::Alert(_) => self.buffer[prefix].into(),
                            NodeValue::BlockQuote => self.buffer[prefix].into(),
                            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
                            NodeValue::DescriptionList => unimplemented!("extension disabled"),
                            NodeValue::Document => "".into(),
                            NodeValue::FootnoteDefinition(_) => "  ".into(),

                            NodeValue::Item(_) => " ".repeat(prefix.len().0),
                            NodeValue::List(_) => unreachable!("skipped"),
                            NodeValue::MultilineBlockQuote(_) => {
                                unimplemented!("extension disabled")
                            }
                            NodeValue::Table(_) => unreachable!("skipped"),
                            NodeValue::TableRow(_) => unreachable!("skipped"),
                            NodeValue::TaskItem(_) => " ".repeat(prefix.len().0),

                            // inline
                            NodeValue::Image(_)
                            | NodeValue::Code(_)
                            | NodeValue::Emph
                            | NodeValue::Escaped
                            | NodeValue::EscapedTag(_)
                            | NodeValue::FootnoteReference(_)
                            | NodeValue::HtmlInline(_)
                            | NodeValue::LineBreak
                            | NodeValue::Link(_)
                            | NodeValue::Math(_)
                            | NodeValue::SoftBreak
                            | NodeValue::SpoileredText
                            | NodeValue::Strikethrough
                            | NodeValue::Strong
                            | NodeValue::Subscript
                            | NodeValue::Superscript
                            | NodeValue::Text(_)
                            | NodeValue::Underline
                            | NodeValue::WikiLink(_) => unreachable!("not a container block"),

                            // leaf_block
                            NodeValue::CodeBlock(_)
                            | NodeValue::DescriptionDetails
                            | NodeValue::DescriptionTerm
                            | NodeValue::Heading(_)
                            | NodeValue::HtmlBlock(_)
                            | NodeValue::Paragraph
                            | NodeValue::TableCell
                            | NodeValue::ThematicBreak => unreachable!("not a container block"),
                        };

                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: "\n".into(),
                        }));
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: extension_prefix,
                        }));
                    } else if content.is_empty() {
                        // empty container block -> terminate
                        let Some(parent) = node.parent() else {
                            return false;
                        };

                        let parent_prefix_len = self.line_prefix_len(parent, line);
                        let own_prefix_len = prefix_len - parent_prefix_len;
                        let own_prefix = (offset - own_prefix_len, offset);

                        operations.push(Operation::Replace(Replace {
                            range: own_prefix,
                            text: "".into(),
                        }));
                    } else {
                        // nonempty container block -> insert
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: "\n".into(),
                        }));
                        operations.push(Operation::Replace(Replace {
                            range: current_selection,
                            text: self.buffer[prefix].into(),
                        }));
                    }
                    true
                };
                if !handled() {
                    // default -> insert newline
                    operations.push(Operation::Replace(Replace {
                        range: current_selection,
                        text: "\n".into(),
                    }));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Delete { region } => {
                // delete container block prefix
                let mut handled = || {
                    // must be vanilla backspace
                    if !matches!(
                        region,
                        Region::SelectionOrOffset {
                            offset: Offset::Next(Bound::Char), // todo: consider other bound types
                            backwards: true,
                        }
                    ) {
                        return false;
                    }

                    // selection must be empty
                    let offset = if current_selection.is_empty() {
                        current_selection.0
                    } else {
                        return false;
                    };

                    let node = self.container_block_descendant_at_offset(root, offset);

                    if matches!(
                        node.data.borrow().value,
                        NodeValue::List(_) | NodeValue::Table(_) | NodeValue::TableRow(_)
                    ) {
                        return false;
                    }

                    let (line_idx, _) =
                        self.bounds.source_lines.find_containing(offset, true, true);
                    let line = self.bounds.source_lines[line_idx];

                    let prefix_len = self.line_prefix_len(node, line);
                    let prefix = (line.start(), line.start() + prefix_len);
                    let content = (prefix.end(), line.end());

                    // content must be empty
                    if !content.is_empty() {
                        return false;
                    }

                    // node must not be document
                    let Some(parent) = node.parent() else {
                        return false;
                    };

                    // empty container block -> terminate
                    let parent_prefix_len = self.line_prefix_len(parent, line);
                    let own_prefix_len = prefix_len - parent_prefix_len;
                    let own_prefix = (offset - own_prefix_len, offset);

                    operations
                        .push(Operation::Replace(Replace { range: own_prefix, text: "".into() }));

                    true
                };
                if !handled() {
                    // default -> delete region
                    let range = self.region_to_range(region);
                    operations.push(Operation::Replace(Replace { range, text: "".into() }));
                    operations.push(Operation::Select(range.start().to_range()));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Indent { deindent } => {
                let lines = self
                    .bounds
                    .source_lines
                    .find_intersecting(current_selection, true);
                let first_line_idx = lines.0;

                // indent into prior list item
                let indent = || {
                    // must not be first line
                    if first_line_idx == 0 {
                        // default -> four space indent
                        return 4.into();
                    }

                    let prior_line_idx = first_line_idx - 1;
                    let prior_line = self.bounds.source_lines[prior_line_idx];
                    let prior_node =
                        self.container_block_descendant_at_offset(root, prior_line.start());

                    // prior line must be in list item
                    if !matches!(
                        prior_node.data.borrow().value,
                        NodeValue::Item(_) | NodeValue::TaskItem(_)
                    ) {
                        // default -> four space indent
                        return 4.into();
                    }

                    let prefix_len = self.line_prefix_len(prior_node, prior_line);
                    let parent_prefix_len =
                        self.line_prefix_len(prior_node.parent().unwrap(), prior_line);
                    prefix_len - parent_prefix_len
                };
                let indent = indent();

                for line_idx in lines.iter() {
                    let line = self.bounds.source_lines[line_idx];

                    operations.push(Operation::Replace(Replace {
                        range: line.start().into_range(),
                        text: " ".repeat(indent.0),
                    }));
                }

                // advance cursor
                operations.push(Operation::Select(current_selection.start().to_range()));
            }
            Event::Find { term, backwards } => {
                // if let Some(result) = self.find(term, backwards) {
                //     operations.push(Operation::Select(result));
                // }
            }
            Event::Undo => {
                response |= self.buffer.undo();
            }
            Event::Redo => {
                response |= self.buffer.redo();
            }
            Event::Cut => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_paragraph()
                };

                ctx.output_mut(|o| o.copied_text = self.buffer[range].into());
                operations.push(Operation::Replace(Replace { range, text: "".into() }));
            }
            Event::Copy => {
                let range = if !current_selection.is_empty() {
                    current_selection
                } else {
                    self.clipboard_current_paragraph()
                };

                ctx.output_mut(|o| o.copied_text = self.buffer[range].into());
            }
            Event::ToggleDebug => {
                // self.debug.draw_enabled = !self.debug.draw_enabled;
            }
            Event::IncrementBaseFontSize => {
                // self.appearance.base_font_size =
                //     self.appearance.base_font_size.map(|size| size + 1.)
            }
            Event::DecrementBaseFontSize => {
                // if self.appearance.font_size() > 2. {
                //     self.appearance.base_font_size =
                //         self.appearance.base_font_size.map(|size| size - 1.)
                // }
            }
            Event::ToggleCheckbox(galley_idx) => {
                // let galley = &self.galleys[galley_idx];
                // if let Some(Annotation::Item(ListItem::Todo(checked), ..)) = galley.annotation {
                //     operations.push(Operation::Replace(Replace {
                //         range: (
                //             galley.range.start() + galley.head_size - 6,
                //             galley.range.start() + galley.head_size,
                //         ),
                //         text: if checked { "* [ ] " } else { "* [x] " }.into(),
                //     }));
                // }
            }
        }

        response
    }

    // /// Returns true if all text in the current selection has style `style`
    // fn should_unapply(&self, style: &MarkdownNode) -> bool {
    //     let current_selection = self.buffer.current.selection;

    //     // look for at least one ancestor that applies the style for each of selection start and end
    //     let mut style_applying_ancestor_start = None;
    //     let mut style_applying_ancestor_end = None;
    //     for text_range in &self.bounds.ast {
    //         // skip ranges before or after the selection
    //         if text_range.range.end() < current_selection.start() {
    //             continue;
    //         }
    //         if current_selection.end() <= text_range.range.start() {
    //             break;
    //         }

    //         // skip ranges that do not contain the selection start or end
    //         let start_contained = text_range
    //             .range
    //             .contains(current_selection.start(), false, true);
    //         let end_contained = text_range
    //             .range
    //             .contains(current_selection.end(), false, true);
    //         if !start_contained && !end_contained {
    //             continue;
    //         }

    //         let mut found_list_item = false;
    //         for &ancestor in text_range.ancestors.iter().rev() {
    //             // only consider the innermost list item
    //             // text in a bulleted sublist of a numbered superlist is not considered having a numbered list style
    //             if matches!(
    //                 self.ast.nodes[ancestor].node_type.node_type(),
    //                 MarkdownNodeType::Block(BlockNodeType::ListItem(..))
    //             ) {
    //                 if found_list_item {
    //                     continue;
    //                 } else {
    //                     found_list_item = true;
    //                 }
    //             }

    //             let style_matches =
    //                 self.ast.nodes[ancestor].node_type.node_type() == style.node_type();
    //             if style_matches {
    //                 if start_contained {
    //                     style_applying_ancestor_start = Some(ancestor);
    //                 }
    //                 if end_contained {
    //                     style_applying_ancestor_end = Some(ancestor);
    //                 }
    //                 break;
    //             }
    //         }
    //     }

    //     // style-applying ancestor must be the same for both
    //     style_applying_ancestor_start.is_some()
    //         && style_applying_ancestor_start == style_applying_ancestor_end
    // }

    // /// Applies or unapplies `style` to `cursor`, splitting or joining surrounding styles as necessary.
    // fn apply_style(
    //     &self, range: (DocCharOffset, DocCharOffset), style: MarkdownNode, unapply: bool,
    //     operations: &mut Vec<Operation>,
    // ) {
    //     let selection = self.buffer.current.selection;
    //     if self.buffer.current.text.is_empty() {
    //         insert_head(range.start(), style.clone(), operations);
    //         operations.push(Operation::Select(selection));
    //         insert_tail(range.start(), style, operations);
    //         return;
    //     }

    //     // find range containing cursor start and cursor end
    //     let mut start_range = None;
    //     let mut end_range = None;
    //     for text_range in &self.bounds.ast {
    //         // when at bound, start prefers next
    //         if text_range.range.contains_inclusive(range.start()) {
    //             start_range = Some(text_range.clone());
    //         }
    //         // when at bound, end prefers previous unless selection is empty
    //         if (range.is_empty() || end_range.is_none())
    //             && text_range.range.contains_inclusive(range.end())
    //         {
    //             end_range = Some(text_range);
    //         }
    //     }

    //     // start always has next because if it were at doc end, selection would be empty (early return above)
    //     // end always has previous because if it were at doc start, selection would be empty (early return above)
    //     let start_range = start_range.unwrap();
    //     let end_range = end_range.unwrap();

    //     // find nodes applying given style containing cursor start and cursor end
    //     // consider only innermost list items
    //     let mut found_list_item = false;
    //     let mut last_start_ancestor: Option<usize> = None;
    //     for &ancestor in start_range.ancestors.iter().rev() {
    //         if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
    //             if found_list_item {
    //                 continue;
    //             } else {
    //                 found_list_item = true;
    //             }
    //         }

    //         if self.ast.nodes[ancestor].node_type.node_type() == style.node_type() {
    //             last_start_ancestor = Some(ancestor);
    //         }
    //     }
    //     found_list_item = false;
    //     let mut last_end_ancestor: Option<usize> = None;
    //     for &ancestor in end_range.ancestors.iter().rev() {
    //         if matches!(style.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
    //             if found_list_item {
    //                 continue;
    //             } else {
    //                 found_list_item = true;
    //             }
    //         }

    //         if self.ast.nodes[ancestor].node_type.node_type() == style.node_type() {
    //             last_end_ancestor = Some(ancestor);
    //         }
    //     }
    //     if last_start_ancestor != last_end_ancestor {
    //         // if start and end are in different nodes, detail start and dehead end (remove syntax characters inside selection)
    //         if let Some(last_start_ancestor) = last_start_ancestor {
    //             detail_ast_node(last_start_ancestor, &self.ast, operations);
    //         }
    //         if let Some(last_end_ancestor) = last_end_ancestor {
    //             dehead_ast_node(last_end_ancestor, &self.ast, operations);
    //         }
    //     }
    //     if unapply {
    //         // if unapplying, tail or dehead node containing start to crop styled region to selection
    //         if let Some(last_start_ancestor) = last_start_ancestor {
    //             if self.ast.nodes[last_start_ancestor].text_range.start() < range.start() {
    //                 if !matches!(style.node_type(), MarkdownNodeType::Block(..)) {
    //                     let offset = adjust_for_whitespace(
    //                         &self.buffer,
    //                         range.start(),
    //                         style.node_type(),
    //                         true,
    //                     );
    //                     insert_tail(offset, style.clone(), operations);
    //                 }
    //             } else {
    //                 dehead_ast_node(last_start_ancestor, &self.ast, operations);
    //             }
    //         }

    //         // selection must be updated after between changes to start and end to avoid selecting new head/tail
    //         operations.push(Operation::Select(selection));

    //         // if unapplying, head or detail node containing end to crop styled region to selection
    //         if let Some(last_end_ancestor) = last_end_ancestor {
    //             if self.ast.nodes[last_end_ancestor].text_range.end() > range.end() {
    //                 if !matches!(style.node_type(), MarkdownNodeType::Block(..)) {
    //                     let offset = adjust_for_whitespace(
    //                         &self.buffer,
    //                         range.end(),
    //                         style.node_type(),
    //                         false,
    //                     );
    //                     insert_head(offset, style.clone(), operations);
    //                 }
    //             } else {
    //                 detail_ast_node(last_end_ancestor, &self.ast, operations);
    //             }
    //         }
    //     } else {
    //         // if applying, head start and/or tail end to extend styled region to selection
    //         if last_start_ancestor.is_none() {
    //             let offset =
    //                 adjust_for_whitespace(&self.buffer, range.start(), style.node_type(), false)
    //                     .min(range.end());
    //             insert_head(offset, style.clone(), operations)
    //         }

    //         // selection must be updated after between changes to start and end to avoid selecting new head/tail
    //         operations.push(Operation::Select(selection));

    //         if last_end_ancestor.is_none() {
    //             let offset =
    //                 adjust_for_whitespace(&self.buffer, range.end(), style.node_type(), true)
    //                     .max(range.start());
    //             insert_tail(offset, style.clone(), operations)
    //         }
    //     }

    //     // remove head and tail for nodes between nodes containing start and end
    //     let mut found_start_range = false;
    //     for text_range in &self.bounds.ast {
    //         // skip ranges until we pass the range containing the selection start (handled above)
    //         if text_range == &start_range {
    //             found_start_range = true;
    //         }
    //         if !found_start_range {
    //             continue;
    //         }

    //         // stop when we find the range containing the selection end (handled above)
    //         if text_range == end_range {
    //             break;
    //         }

    //         // dehead and detail nodes with this style in the middle, aside from those already considered
    //         if text_range.node(&self.ast) == style
    //             && text_range.range_type == AstTextRangeType::Text
    //         {
    //             let node_idx = text_range.ancestors.last().copied().unwrap();
    //             if start_range.ancestors.iter().any(|&a| a == node_idx) {
    //                 continue;
    //             }
    //             if end_range.ancestors.iter().any(|&a| a == node_idx) {
    //                 continue;
    //             }
    //             dehead_ast_node(node_idx, &self.ast, operations);
    //             detail_ast_node(node_idx, &self.ast, operations);
    //         }
    //     }
    // }

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
            Location::Pos(pos) => pos_to_char_offset(pos, &self.galleys, &self.bounds.text),
        }
    }

    fn clipboard_current_paragraph(&self) -> (DocCharOffset, DocCharOffset) {
        let current_selection = self.buffer.current.selection;
        let paragraph_idx = self
            .bounds
            .paragraphs
            .find_containing(current_selection.1, true, true)
            .0;

        let mut result = self.bounds.paragraphs[paragraph_idx];

        // capture leading newline, if any
        if paragraph_idx != 0 {
            let paragraph = self.bounds.paragraphs[paragraph_idx];
            let prev_paragraph = self.bounds.paragraphs[paragraph_idx - 1];
            let range_between_paragraphs = (prev_paragraph.1, paragraph.0);
            let rbp_text = &self.buffer[range_between_paragraphs];
            if rbp_text.ends_with("\r\n") {
                result.0 -= 2;
            } else if rbp_text.ends_with('\n') || rbp_text.ends_with('\r') {
                result.0 -= 1;
            }
        }

        result
    }
}

// todo: find a better home along with text & link functions
pub fn pos_to_char_offset(pos: Pos2, galleys: &Galleys, text: &Text) -> DocCharOffset {
    let galley_idx = pos_to_galley(pos, galleys);
    let galley = &galleys[galley_idx];

    let expanded_rect = galley.rect.expand(ROW_SPACING / 2.);

    if pos.y < expanded_rect.min.y {
        // click position is above galley
        galley.range.start()
    } else if pos.y > expanded_rect.max.y {
        // click position is below galley
        galley.range.end()
    } else {
        let relative_pos = pos - expanded_rect.min;

        // clamp y coordinate for forgiving cursor placement clicks
        let relative_pos =
            Vec2::new(relative_pos.x, relative_pos.y.clamp(0.0, galley.rect.height()));

        if galley.range.is_empty() {
            // hack: empty galley range means every position in the galley maps to
            // that location
            galley.range.start()
        } else {
            let new_cursor = galley.galley.cursor_from_pos(relative_pos);
            galleys.char_offset_by_galley_and_cursor(galley_idx, new_cursor, text)
        }
    }
}

pub fn pos_to_galley(pos: Pos2, galleys: &Galleys) -> usize {
    let mut closest_galley = None;
    let mut closest_distance = (f32::INFINITY, f32::INFINITY);
    for (galley_idx, galley) in galleys.galleys.iter().enumerate() {
        if galley.rect.contains(pos) {
            return galley_idx; // galleys do not overlap
        }

        // this ain't yo mama's distance metric
        let x_distance = distance(pos.x, galley.rect.x_range());
        let y_distance = distance(pos.y, galley.rect.y_range());
        if (y_distance, x_distance) < closest_distance {
            closest_galley = Some(galley_idx);
            closest_distance = (y_distance, x_distance);
        }
    }
    closest_galley.expect("there must always be a galley")
}

pub fn distance(coord: f32, range: Rangef) -> f32 {
    if range.contains(coord) {
        0.
    } else {
        (coord - range.min).abs().min((coord - range.max).abs())
    }
}

// pub fn pos_to_link(
//     pos: Pos2, galleys: &Galleys, buffer: &Buffer, bounds: &Bounds, ast: &Ast,
// ) -> Option<String> {
//     pos_to_galley(pos, galleys, &buffer.current.segs, bounds)?;
//     let offset = pos_to_char_offset(pos, galleys, &buffer.current.segs, &bounds.text);

//     // todo: binary search
//     for ast_node in &ast.nodes {
//         if let MarkdownNode::Inline(InlineNode::Link(_, url, _)) = &ast_node.node_type {
//             if ast_node.range.contains_inclusive(offset) {
//                 return Some(url.to_string());
//             }
//         }
//     }
//     for plaintext_link in &bounds.links {
//         if plaintext_link.contains_inclusive(offset) {
//             return Some(buffer[*plaintext_link].to_string());
//         }
//     }

//     None
// }

// /// Returns list of nodes whose styles should be removed before applying `style`
// fn conflicting_styles(
//     range: (DocCharOffset, DocCharOffset), style: &MarkdownNode, ast: &Ast,
//     ast_ranges: &AstTextRanges,
// ) -> Vec<MarkdownNode> {
//     let mut result = Vec::new();
//     let mut dedup_set = HashSet::new();
//     if range.is_empty() {
//         return result;
//     }

//     for text_range in ast_ranges {
//         // skip ranges before or after the parameter range
//         if text_range.range.end() < range.start() {
//             continue;
//         }
//         if range.end() <= text_range.range.start() {
//             break;
//         }

//         // look for ancestors that apply a conflicting style
//         let mut found_list_item = false;
//         for &ancestor in text_range.ancestors.iter().rev() {
//             let node = &ast.nodes[ancestor].node_type;

//             // only remove the innermost conflicting list item
//             if matches!(node.node_type(), MarkdownNodeType::Block(BlockNodeType::ListItem(..))) {
//                 if found_list_item {
//                     continue;
//                 } else {
//                     found_list_item = true;
//                 }
//             }

//             if node.node_type().conflicts_with(&style.node_type()) && dedup_set.insert(node.clone())
//             {
//                 result.push(node.clone());
//             }
//         }
//     }

//     result
// }

// // appends operations to `mutation` to renumber list items and returns numbers assigned to each galley
// fn increment_numbered_list_items(
//     starting_galley_idx: usize, indent_level: u8, amount: usize, decrement: bool,
//     galleys: &Galleys, renumbers: &mut HashMap<usize, usize>,
// ) {
//     let mut galley_idx = starting_galley_idx;
//     loop {
//         galley_idx += 1;
//         if galley_idx == galleys.len() {
//             break;
//         }
//         let galley = &galleys[galley_idx];
//         if let Some(Annotation::Item(item_type, cur_indent_level)) = &galley.annotation {
//             match cur_indent_level.cmp(&indent_level) {
//                 Ordering::Greater => {
//                     continue; // skip nested list items
//                 }
//                 Ordering::Less => {
//                     break; // end of nested list
//                 }
//                 Ordering::Equal => {
//                     if let ListItem::Numbered(cur_number) = item_type {
//                         // if galley has already been processed, use its most recently assigned number
//                         let cur_number = renumbers.get(&galley_idx).unwrap_or(cur_number);

//                         // replace cur_number with next_number in head
//                         let new_number = if !decrement {
//                             cur_number.saturating_add(amount)
//                         } else {
//                             cur_number.saturating_sub(amount)
//                         };

//                         renumbers.insert(galley_idx, new_number);
//                     }
//                 }
//             }
//         } else {
//             break;
//         }
//     }
// }

// fn dehead_ast_node(node_idx: usize, ast: &Ast, operations: &mut Vec<Operation>) {
//     let node = &ast.nodes[node_idx];
//     operations.push(Operation::Replace(Replace {
//         range: (node.range.start(), node.text_range.start()),
//         text: "".into(),
//     }));
// }

// fn detail_ast_node(node_idx: usize, ast: &Ast, operations: &mut Vec<Operation>) {
//     let node = &ast.nodes[node_idx];
//     operations.push(Operation::Replace(Replace {
//         range: (node.text_range.end(), node.range.end()),
//         text: "".into(),
//     }));
// }

// fn adjust_for_whitespace(
//     buffer: &Buffer, mut offset: DocCharOffset, style: MarkdownNodeType, tail: bool,
// ) -> DocCharOffset {
//     if matches!(style, MarkdownNodeType::Inline(..)) {
//         loop {
//             let c = if tail {
//                 if offset == 0 {
//                     break;
//                 }
//                 &buffer[(offset - 1, offset)]
//             } else {
//                 if offset == buffer.current.segs.last_cursor_position() {
//                     break;
//                 }
//                 &buffer[(offset, offset + 1)]
//             };
//             if c == " " {
//                 if tail {
//                     offset -= 1
//                 } else {
//                     offset += 1
//                 }
//             } else {
//                 break;
//             }
//         }
//     }
//     offset
// }

// fn insert_head(offset: DocCharOffset, style: MarkdownNode, operations: &mut Vec<Operation>) {
//     let text = style.head();
//     operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
// }

// fn insert_tail(offset: DocCharOffset, style: MarkdownNode, operations: &mut Vec<Operation>) {
//     let text = style.node_type().tail().to_string();
//     if style.node_type() == MarkdownNodeType::Inline(InlineNodeType::Link) {
//         operations
//             .push(Operation::Replace(Replace { range: offset.to_range(), text: text[..2].into() }));
//         operations.push(Operation::Select(offset.to_range()));
//         operations
//             .push(Operation::Replace(Replace { range: offset.to_range(), text: text[2..].into() }));
//     } else {
//         operations.push(Operation::Replace(Replace { range: offset.to_range(), text }));
//     }
// }

// todo: this needs more attention e.g. list items indented using 4-space indents
// tracked by https://github.com/lockbook/lockbook/issues/1842
fn indent_seq(s: &str) -> String {
    if s.starts_with('\t') {
        "\t"
    } else if s.starts_with(' ') {
        "  "
    } else {
        "\t"
    }
    .into()
}

fn indent_level(s: &str) -> u8 {
    (if s.starts_with('\t') {
        s.chars().take_while(|c| c == &'\t').count()
    } else if s.starts_with(' ') {
        s.chars().take_while(|c| c == &' ').count() / 2
    } else {
        0
    }) as _
}

#[cfg(test)]
mod test {
    #[test]
    fn indent_seq() {
        assert_eq!(super::indent_seq(""), "\t");
        assert_eq!(super::indent_seq("text"), "\t");
        assert_eq!(super::indent_seq("\ttext"), "\t");
        assert_eq!(super::indent_seq("  text"), "  ");
        assert_eq!(super::indent_seq("    text"), "  ");
        assert_eq!(super::indent_seq("\t  text"), "\t");
        assert_eq!(super::indent_seq("  \ttext"), "  ");
        assert_eq!(super::indent_seq("\t\ttext"), "\t");
        assert_eq!(super::indent_seq("  \t  text"), "  ");
        assert_eq!(super::indent_seq("\t  \ttext"), "\t");
    }

    #[test]
    fn indent_level() {
        assert_eq!(super::indent_level(""), 0);
        assert_eq!(super::indent_level("text"), 0);
        assert_eq!(super::indent_level("\ttext"), 1);
        assert_eq!(super::indent_level("  text"), 1);
        assert_eq!(super::indent_level("    text"), 2);
        assert_eq!(super::indent_level("\t  text"), 1);
        assert_eq!(super::indent_level("  \ttext"), 1);
        assert_eq!(super::indent_level("\t\ttext"), 2);
        assert_eq!(super::indent_level("  \t  text"), 1);
        assert_eq!(super::indent_level("\t  \ttext"), 1);
    }
}
