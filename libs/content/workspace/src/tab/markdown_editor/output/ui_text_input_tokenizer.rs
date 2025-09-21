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
                if in_backward_direction {
                    text_position == first_range.start()
                } else {
                    text_position == first_range.end()
                }
            }
            BoundCase::AtLastRangeEnd { last_range, .. } => {
                if in_backward_direction {
                    text_position == last_range.start()
                } else {
                    text_position == last_range.end()
                }
            }
            BoundCase::InsideRange { range } => {
                if in_backward_direction {
                    text_position == range.start()
                } else {
                    text_position == range.end()
                }
            }
            BoundCase::AtEmptyRange { .. } => true,
            BoundCase::AtSharedBoundOfTouchingNonemptyRanges { .. } => true,
            BoundCase::AtEndOfNonemptyRange { .. } => !in_backward_direction,
            BoundCase::AtStartOfNonemptyRange { .. } => in_backward_direction,
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
            Bound::Line => {
                // note: lines handled as words
                //
                // I assume there's a mistake in the implementation of Apple's virtual keyboard. Documentation and
                // examples are sparse - we do not have a single working implementation, even using the built-in
                // tokenizer or a reference implementation crafted by an Apple engineer for us.
                //
                // I inspected the keyboard behavior by logging all fn calls and their results. The keyboard iterates
                // the returned ranges until it receives a nil value, which is more consistent with a text granularity
                // that is non-contiguous the way words are and lines are not. This seems to be what it takes to get
                // the correct undeline behavior after autocorrecting a word.
                &self.bounds.words
            }
            Bound::Paragraph => &self.bounds.paragraphs,
            Bound::Doc => {
                unimplemented!()
            }
        };
        let result = match text_position.bound_case(ranges) {
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
            BoundCase::InsideRange { range } => Some(range),
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
            BoundCase::BetweenRanges { range_before, range_after } => {
                if with_granularity == Bound::Word {
                    // hack: treat space between words as words
                    Some((range_before.end(), range_after.start()))
                } else {
                    None
                }
            }
        };
        if let Some(result) = result {
            // this can happen if we are beyond the last range e.g. asking about word at a position after a document's trailing space
            if !result.contains(text_position, !in_backward_direction, in_backward_direction) {
                return None;
            }
        }

        result
    }
}
