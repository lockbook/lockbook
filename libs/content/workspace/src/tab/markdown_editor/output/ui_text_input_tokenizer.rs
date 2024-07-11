use crate::tab::markdown_editor::{
    bounds::Bounds,
    input::canonical::Bound,
    offset_types::{DocCharOffset, RangeExt as _},
};

// Swift protocol for tokenizing text input.
// https://developer.apple.com/documentation/uikit/uitextinputtokenizer
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
        text_position == text_position.advance_to_bound(at_boundary, in_backward_direction, self)
    }

    fn is_position_within_text_unit(
        &self, text_position: DocCharOffset, within_text_unit: Bound, in_backward_direction: bool,
    ) -> bool {
        if let Some(range) =
            text_position.range_bound(within_text_unit, in_backward_direction, false, self)
        {
            // this implementation doesn't meet the specification in apple's docs, but the implementation that does creates word jumping bugs
            if range.contains_inclusive(text_position) {
                return true;
            }
        }
        false
    }

    fn position_from(
        &self, text_position: DocCharOffset, to_boundary: Bound, in_backward_direction: bool,
    ) -> Option<DocCharOffset> {
        Some(text_position.advance_to_next_bound(to_boundary, in_backward_direction, self))
    }

    fn range_enclosing_position(
        &self, text_position: DocCharOffset, with_granularity: Bound, in_backward_direction: bool,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        Some(
            text_position
                .range_bound(with_granularity, in_backward_direction, true, self)
                .unwrap_or((text_position, text_position)),
        )
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

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Char, false), false);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Char, false), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Char, false), true);

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Char, true), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Char, true), false);
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
    fn is_position_at_boundary_line() {
        // "a\nline"
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
            lines: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 1), (2, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 6)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Line, false), false);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Line, false), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Line, false), false);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Line, false), false);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Line, false), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Line, false), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Line, false), true);

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Line, true), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Line, true), false);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Line, true), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Line, true), false);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Line, true), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Line, true), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Line, true), false);
    }

    #[test]
    fn is_position_at_boundary_paragraph() {
        // "a\n\nparagraph"
        let bounds = Bounds {
            ast: [(0, 12)]
                .into_iter()
                .map(|r| AstTextRange {
                    range_type: AstTextRangeType::Text,
                    range: (r.start().into(), r.end().into()),
                    ancestors: vec![],
                })
                .collect(),
            words: [(0, 1), (3, 12)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            lines: [(0, 1), (2, 2), (3, 12)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            paragraphs: [(0, 1), (2, 2), (3, 12)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            text: [(0, 12)]
                .into_iter()
                .map(|r| (r.start().into(), r.end().into()))
                .collect(),
            links: vec![],
        };

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Paragraph, false), true);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Paragraph, false), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(7.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(8.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(9.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(10.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(11.into(), Bound::Paragraph, false), false);
        assert_eq!(bounds.is_position_at_boundary(12.into(), Bound::Paragraph, false), true);

        assert_eq!(bounds.is_position_at_boundary(0.into(), Bound::Paragraph, true), true);
        assert_eq!(bounds.is_position_at_boundary(1.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(2.into(), Bound::Paragraph, true), true);
        assert_eq!(bounds.is_position_at_boundary(3.into(), Bound::Paragraph, true), true);
        assert_eq!(bounds.is_position_at_boundary(4.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(5.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(6.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(7.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(8.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(9.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(10.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(11.into(), Bound::Paragraph, true), false);
        assert_eq!(bounds.is_position_at_boundary(12.into(), Bound::Paragraph, true), false);
    }
}
