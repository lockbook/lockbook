use crate::tab::markdown_editor::{
    input::{
        canonical::{Bound, Location, Offset, Region},
        cursor::Cursor,
        mutation,
    },
    offset_types::{DocCharOffset, RangeExt as _},
    Editor,
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

impl UITextInputTokenizer for Editor {
    fn is_position_at_boundary(
        &self, text_position: DocCharOffset, at_boundary: Bound, in_backward_direction: bool,
    ) -> bool {
        if let Some(range) =
            text_position.range_bound(at_boundary, in_backward_direction, false, &self.bounds)
        {
            // forward: the provided position is at the end of the enclosing range
            // backward: the provided position is at the start of the enclosing range
            if !in_backward_direction && text_position == range.end()
                || in_backward_direction && text_position == range.start()
            {
                return true;
            }
        }
        false
    }

    fn is_position_within_text_unit(
        &self, text_position: DocCharOffset, within_text_unit: Bound, in_backward_direction: bool,
    ) -> bool {
        if let Some(range) =
            text_position.range_bound(within_text_unit, in_backward_direction, false, &self.bounds)
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
        let mut cursor: Cursor = text_position.into();
        cursor.advance(
            Offset::Next(to_boundary),
            in_backward_direction,
            &self.buffer.current,
            &self.galleys,
            &self.bounds,
        );
        Some(cursor.selection.1)
    }

    fn range_enclosing_position(
        &self, text_position: DocCharOffset, with_granularity: Bound, in_backward_direction: bool,
    ) -> Option<(DocCharOffset, DocCharOffset)> {
        let cursor = mutation::region_to_cursor(
            Region::BoundAt {
                bound: with_granularity,
                location: Location::DocCharOffset(text_position),
                backwards: in_backward_direction,
            },
            self.buffer.current.cursor,
            &self.buffer.current,
            &self.galleys,
            &self.bounds,
        );
        Some(cursor.selection)
    }
}
