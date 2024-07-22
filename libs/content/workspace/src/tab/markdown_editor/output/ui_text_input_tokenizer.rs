use crate::tab::markdown_editor::{
    bounds::{BoundCase, Bounds},
    input::canonical::Bound,
    offset_types::{DocCharOffset, RangeExt as _},
};

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

impl UITextInputTokenizer for Bounds {
    fn is_position_at_boundary(
        &self, text_position: DocCharOffset, at_boundary: Bound, in_backward_direction: bool,
    ) -> bool {
        let ranges = match at_boundary {
            Bound::Char => {
                return true;
            }
            Bound::Word => &self.words,
            Bound::Line => &self.lines,
            Bound::Paragraph => &self.paragraphs,
            Bound::Doc => {
                return text_position == DocCharOffset(0)
                    || text_position == self.text.last().copied().unwrap_or_default().end();
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
            Bound::Word => &self.words,
            Bound::Line => &self.lines,
            Bound::Paragraph => &self.paragraphs,
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
        Some(text_position.advance_to_next_bound(to_boundary, in_backward_direction, self))
    }

    fn range_enclosing_position(
        &self, text_position: DocCharOffset, with_granularity: Bound, in_backward_direction: bool,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let ranges = match with_granularity {
            Bound::Char => {
                unimplemented!()
            }
            Bound::Word => &self.words,
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
                &self.words
            }
            Bound::Paragraph => &self.paragraphs,
            Bound::Doc => {
                unimplemented!()
            }
        };
        match text_position.bound_case(ranges) {
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
            BoundCase::BetweenRanges { .. } => None,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::tab::markdown_editor::{
        ast::{AstTextRange, AstTextRangeType},
        bounds::Bounds,
        input::canonical::Bound,
        offset_types::RangeExt as _,
        output::ui_text_input_tokenizer::UITextInputTokenizer as _,
    };

    #[test]
    fn is_position_at_boundary_char() {
        // "hey"
        let bounds = Bounds {
            ast: [(0, 3)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Char, false), true);

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Char, true), true);
    }

    #[test]
    fn is_position_at_boundary_word() {
        // "a word"
        let bounds = Bounds {
            ast: [(0, 6)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Word, false), false);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Word, false), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Word, false), false);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Word, false), false);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Word, false), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Word, false), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Word, false), true);

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Word, true), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Word, true), false);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Word, true), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Word, true), false);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Word, true), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Word, true), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Word, true), false);
    }

    #[test]
    fn is_position_within_text_unit_char() {
        // "hey"
        let bounds = Bounds {
            ast: [(0, 3)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_within_text_unit(0.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(1.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(2.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(3.into(), Bound::Char, false), true);

        assert_eq!(bounds.is_position_within_text_unit(0.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(1.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(2.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(3.into(), Bound::Char, true), true);
    }

    #[test]
    fn is_position_within_text_unit_word() {
        // "a word"
        let bounds = Bounds {
            ast: [(0, 6)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_within_text_unit(0.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(1.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(2.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(3.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(4.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(5.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_within_text_unit(6.into(), Bound::Char, false), true);

        assert_eq!(bounds.is_position_within_text_unit(0.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(1.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(2.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(3.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(4.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(5.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_within_text_unit(6.into(), Bound::Char, true), true);
    }

    #[test]
    fn position_from_char() {
        // "hey"
        let bounds = Bounds {
            ast: [(0, 3)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.position_from(0.into(), Bound::Char, false), Some(1.into()));
        assert_eq!(bounds.position_from(1.into(), Bound::Char, false), Some(2.into()));
        assert_eq!(bounds.position_from(2.into(), Bound::Char, false), Some(3.into()));
        assert_eq!(bounds.position_from(3.into(), Bound::Char, false), Some(3.into()));

        assert_eq!(bounds.position_from(0.into(), Bound::Char, true), Some(0.into()));
        assert_eq!(bounds.position_from(1.into(), Bound::Char, true), Some(0.into()));
        assert_eq!(bounds.position_from(2.into(), Bound::Char, true), Some(1.into()));
        assert_eq!(bounds.position_from(3.into(), Bound::Char, true), Some(2.into()));
    }

    #[test]
    fn position_from_word() {
        // "a word"
        let bounds = Bounds {
            ast: [(0, 6)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.position_from(0.into(), Bound::Word, false), Some(1.into()));
        assert_eq!(bounds.position_from(1.into(), Bound::Word, false), Some(6.into()));
        assert_eq!(bounds.position_from(2.into(), Bound::Word, false), Some(6.into()));
        assert_eq!(bounds.position_from(3.into(), Bound::Word, false), Some(6.into()));
        assert_eq!(bounds.position_from(4.into(), Bound::Word, false), Some(6.into()));
        assert_eq!(bounds.position_from(5.into(), Bound::Word, false), Some(6.into()));
        assert_eq!(bounds.position_from(6.into(), Bound::Word, false), Some(6.into()));

        assert_eq!(bounds.position_from(0.into(), Bound::Word, true), Some(0.into()));
        assert_eq!(bounds.position_from(1.into(), Bound::Word, true), Some(0.into()));
        assert_eq!(bounds.position_from(2.into(), Bound::Word, true), Some(0.into()));
        assert_eq!(bounds.position_from(3.into(), Bound::Word, true), Some(2.into()));
        assert_eq!(bounds.position_from(4.into(), Bound::Word, true), Some(2.into()));
        assert_eq!(bounds.position_from(5.into(), Bound::Word, true), Some(2.into()));
        assert_eq!(bounds.position_from(6.into(), Bound::Word, true), Some(2.into()));
    }

    #[test]
    fn range_enclosing_position_char() {
        // "hey"
        let bounds = Bounds {
            ast: [(0, 3)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 3)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(
            bounds.range_enclosing_position(0.into(), Bound::Char, false),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(1.into(), Bound::Char, false),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(2.into(), Bound::Char, false),
            Some((1.into(), 2.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(3.into(), Bound::Char, false),
            Some((2.into(), 3.into()))
        );

        assert_eq!(
            bounds.range_enclosing_position(0.into(), Bound::Char, true),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(1.into(), Bound::Char, true),
            Some((1.into(), 2.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(2.into(), Bound::Char, true),
            Some((2.into(), 3.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(3.into(), Bound::Char, true),
            Some((2.into(), 3.into()))
        );
    }

    #[test]
    fn range_enclosing_position_word() {
        // "a word"
        let bounds = Bounds {
            ast: [(0, 6)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(
            bounds.range_enclosing_position(0.into(), Bound::Word, false),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(1.into(), Bound::Word, false),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(2.into(), Bound::Word, false),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(3.into(), Bound::Word, false),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(4.into(), Bound::Word, false),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(5.into(), Bound::Word, false),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(6.into(), Bound::Word, false),
            Some((2.into(), 6.into()))
        );

        assert_eq!(
            bounds.range_enclosing_position(0.into(), Bound::Word, true),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(1.into(), Bound::Word, true),
            Some((0.into(), 1.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(2.into(), Bound::Word, true),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(3.into(), Bound::Word, true),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(4.into(), Bound::Word, true),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(5.into(), Bound::Word, true),
            Some((2.into(), 6.into()))
        );
        assert_eq!(
            bounds.range_enclosing_position(6.into(), Bound::Word, true),
            Some((2.into(), 6.into()))
        );
    }
}
