use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::{BoundCase, BoundExt as _};
use crate::tab::markdown_editor::input::Bound;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

/// Swift protocol for tokenizing text input:
/// https://developer.apple.com/documentation/uikit/uitextinputtokenizer
pub trait UITextInputTokenizer {
    /// Returns whether a text position is at a boundary of a text unit of a specified granularity in a specified direction.
    fn is_position_at_boundary(
        &self, text_position: DocCharOffset, at_boundary: Bound, in_backward_direction: bool,
    ) -> bool;

    /// Returns whether a text position is within a text unit of a specified granularity in a specified direction.
    fn is_position_within_text_unit(
        &self, text_position: DocCharOffset, within_text_unit: Bound, in_backward_direction: bool,
    ) -> bool;

    /// Returns the next text position at a boundary of a text unit of the given granularity in a given direction.
    fn position_from(
        &self, text_position: DocCharOffset, to_boundary: Bound, in_backward_direction: bool,
    ) -> Option<DocCharOffset>;

    /// Returns the range for the text enclosing a text position in a text unit of a given granularity in a given direction.
    fn range_enclosing_position(
        &self, text_position: DocCharOffset, with_granularity: Bound, in_backward_direction: bool,
    ) -> Option<(DocCharOffset, DocCharOffset)>;
}

impl UITextInputTokenizer for Editor {
    fn is_position_at_boundary(
        &self, text_position: DocCharOffset, at_boundary: Bound, in_backward_direction: bool,
    ) -> bool {
        let ranges = match at_boundary {
            Bound::Char => {
                return true;
            }
            Bound::Word => &self.bounds.words,
            Bound::Line => &self.bounds.wrap_lines,
            Bound::Paragraph => &self.bounds.paragraphs,
            Bound::Doc => {
                return text_position == DocCharOffset(0)
                    || text_position
                        == self
                            .bounds
                            .paragraphs
                            .last()
                            .copied()
                            .unwrap_or_default()
                            .end();
            }
        };
        match text_position.bound_case(ranges) {
            BoundCase::NoRanges => true,
            BoundCase::AtFirstRangeStart { first_range, .. } => {
                if !in_backward_direction {
                    text_position == first_range.start()
                } else {
                    text_position == first_range.end()
                }
            }
            BoundCase::AtLastRangeEnd { last_range, .. } => {
                if !in_backward_direction {
                    text_position == last_range.start()
                } else {
                    text_position == last_range.end()
                }
            }
            BoundCase::InsideRange { range } => {
                if !in_backward_direction {
                    text_position == range.start()
                } else {
                    text_position == range.end()
                }
            }
            BoundCase::AtEmptyRange { .. } => true,
            BoundCase::AtSharedBoundOfTouchingNonemptyRanges { .. } => true,
            BoundCase::AtEndOfNonemptyRange { .. } => in_backward_direction,
            BoundCase::AtStartOfNonemptyRange { .. } => !in_backward_direction,
            BoundCase::BetweenRanges { .. } => false,
        }
    }

    fn is_position_within_text_unit(
        &self, text_position: DocCharOffset, within_text_unit: Bound, in_backward_direction: bool,
    ) -> bool {
        let ranges = match within_text_unit {
            Bound::Char => {
                return true;
            }
            Bound::Word => &self.bounds.words,
            Bound::Line => &self.bounds.wrap_lines,
            Bound::Paragraph => &self.bounds.paragraphs,
            Bound::Doc => {
                return true;
            }
        };
        match text_position.bound_case(ranges) {
            BoundCase::NoRanges => false,
            BoundCase::AtFirstRangeStart { first_range, .. } => {
                if !in_backward_direction {
                    text_position == first_range.start()
                } else {
                    text_position == first_range.end()
                }
            }
            BoundCase::AtLastRangeEnd { last_range, .. } => {
                if !in_backward_direction {
                    text_position == last_range.start()
                } else {
                    text_position == last_range.end()
                }
            }
            BoundCase::InsideRange { .. } => true,
            BoundCase::AtEmptyRange { .. } => true,
            BoundCase::AtSharedBoundOfTouchingNonemptyRanges { .. } => true,
            BoundCase::AtEndOfNonemptyRange { .. } => in_backward_direction,
            BoundCase::AtStartOfNonemptyRange { .. } => !in_backward_direction,
            BoundCase::BetweenRanges { .. } => false,
        }
    }

    fn position_from(
        &self, text_position: DocCharOffset, to_boundary: Bound, in_backward_direction: bool,
    ) -> Option<DocCharOffset> {
        Some(text_position.advance_to_next_bound(to_boundary, in_backward_direction, &self.bounds))
    }

    fn range_enclosing_position(
        &self, text_position: DocCharOffset, with_granularity: Bound, in_backward_direction: bool,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let ranges = match with_granularity {
            Bound::Char => {
                unimplemented!()
            }
            Bound::Word => &self.bounds.words,
            Bound::Line => &self.bounds.words, // hack
            Bound::Paragraph => &self.bounds.paragraphs,
            Bound::Doc => {
                unimplemented!()
            }
        };
        let bound_case = text_position.bound_case(ranges);
        match bound_case {
            BoundCase::NoRanges => None,
            BoundCase::AtFirstRangeStart { first_range, .. } => {
                if in_backward_direction {
                    None
                } else {
                    Some(first_range)
                }
            }
            BoundCase::AtLastRangeEnd { last_range, .. } => {
                if in_backward_direction {
                    Some(last_range)
                } else {
                    None
                }
            }
            BoundCase::InsideRange { range } => {
                // // hack: don't include trailing whitespace in the range
                // if self.buffer[(text_position, range.end())]
                //     .trim_end()
                //     .is_empty()
                // {
                //     println!("  InsideRange -> AtEndOfNonemptyRange");
                //     if in_backward_direction { Some(range) } else { None }
                // } else {
                //     Some(range)
                // }
                Some(range)
            }
            BoundCase::AtEmptyRange { .. } => None,
            BoundCase::AtSharedBoundOfTouchingNonemptyRanges { range_before, range_after } => {
                Some(if in_backward_direction { range_before } else { range_after })
            }
            BoundCase::AtEndOfNonemptyRange { range_before, .. } => {
                if in_backward_direction {
                    Some(range_before)
                } else {
                    None
                }
            }
            BoundCase::AtStartOfNonemptyRange { range_after, .. } => {
                if in_backward_direction {
                    None
                } else {
                    Some(range_after)
                }
            }
            BoundCase::BetweenRanges { .. } => None,
        }
    }
}
