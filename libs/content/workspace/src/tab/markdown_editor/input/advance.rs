use std::mem;

use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::tab::markdown_editor::input::{Advance, Increment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

impl MdEdit {
    pub fn advance(&mut self, offset: Grapheme, advance: Advance, backwards: bool) -> Grapheme {
        let maybe_x_target_value = mem::take(&mut self.cursor.x_target);
        match advance {
            Advance::To(bound) => offset.advance_to_bound(bound, backwards, &self.renderer.bounds),
            Advance::Next(bound) => {
                offset.advance_to_next_bound(bound, backwards, &self.renderer.bounds)
            }
            Advance::By(Increment::Char) => {
                let mut result = offset;
                if backwards {
                    if result.0 > 0 {
                        result -= 1;
                    }
                } else {
                    result += 1;
                    result = result.min(self.renderer.buffer.current.segs.last_cursor_position());
                }
                result
            }
            Advance::By(Increment::Lines(n)) => {
                let mut result = offset;
                for _ in 0..n {
                    let Some(result_x) = self.x(result) else {
                        break;
                    };
                    let x_target = maybe_x_target_value.unwrap_or(result_x);
                    result = self.advance_by_line(result, x_target, backwards);
                    if result != 0
                        && result != self.renderer.buffer.current.segs.last_cursor_position()
                    {
                        self.cursor.x_target = Some(x_target);
                    }
                }
                result
            }
        }
    }

    fn advance_by_line(&self, offset: Grapheme, x_target: f32, backwards: bool) -> Grapheme {
        let fragments = &self.renderer.fragments;
        // Last-match wins (mirrors `fragment_at_offset`): at a wrap
        // boundary, cursor rendering uses the row-N+1 fragment, so
        // navigation must too — otherwise down-arrow would target row
        // N+1 from a cur_idx anchored on row N's glue and no-op.
        let cur_idx = fragments.iter().enumerate().rev().find_map(|(i, f)| {
            let (s, e) = f.source_range;
            (s <= offset && offset <= e).then_some(i)
        });
        let Some(cur_idx) = cur_idx else { return offset };
        self.advance_by_line_from(cur_idx, x_target, backwards, offset)
    }

    /// Inner helper: jump to the closest fragment on the row above
    /// (or below) the fragment at `cur_idx`, snapping x to `x_target`.
    fn advance_by_line_from(
        &self, cur_idx: usize, x_target: f32, backwards: bool, fallback_offset: Grapheme,
    ) -> Grapheme {
        let fragments = &self.renderer.fragments;
        let cur_frag = &fragments[cur_idx];
        // (x-distance, decorative-bit) — empty-range fragments
        // (pill pads, scope-boundary pads, anchors) lose ties to real
        // glyph fragments but win when they're the only candidate
        // (e.g. blank-line rows).
        let score = |f: &crate::tab::markdown_editor::widget::utils::wrap_layout::Fragment| -> (f32, u8) {
            let distance =
                (f.rect.left().max(x_target).min(f.rect.right()) - x_target).abs();
            let kind = if f.source_range.start() == f.source_range.end() { 1 } else { 0 };
            (distance, kind)
        };
        if backwards {
            let mut best: Option<(usize, (f32, u8))> = None;
            let mut row_above_top: Option<f32> = None;
            for idx in (0..cur_idx).rev() {
                let f = &fragments[idx];
                let is_above = f.rect.bottom() < cur_frag.rect.top();
                let too_above = row_above_top.is_some_and(|t| f.rect.bottom() < t);
                if too_above {
                    break;
                }
                if !is_above {
                    continue;
                }
                row_above_top = Some(f.rect.top());
                let s = score(f);
                if best.is_none_or(|(_, bs)| s < bs) {
                    best = Some((idx, s));
                }
            }
            let Some((idx, _)) = best else { return fallback_offset };
            let f = &fragments[idx];
            let new_x = x_target.clamp(f.rect.left(), f.rect.right());
            self.renderer.fragment_offset(f, new_x)
        } else {
            let mut best: Option<(usize, (f32, u8))> = None;
            let mut row_below_bottom: Option<f32> = None;
            for (idx, f) in fragments.iter().enumerate().skip(cur_idx + 1) {
                let is_below = f.rect.top() > cur_frag.rect.bottom();
                let too_below = row_below_bottom.is_some_and(|b| f.rect.top() > b);
                if too_below {
                    break;
                }
                if !is_below {
                    continue;
                }
                row_below_bottom = Some(f.rect.bottom());
                let s = score(f);
                if best.is_none_or(|(_, bs)| s < bs) {
                    best = Some((idx, s));
                }
            }
            if let Some((idx, _)) = best {
                let f = &fragments[idx];
                let new_x = x_target.clamp(f.rect.left(), f.rect.right());
                self.renderer.fragment_offset(f, new_x)
            } else if !self
                .renderer
                .bounds
                .source_lines
                .find_containing(fallback_offset, true, true)
                .contains(self.renderer.bounds.source_lines.len() - 1, true, false)
            {
                // The cursor is in the last fragment but not the last
                // source line — likely the last fragment is hidden (folded).
                self.renderer.buffer.current.segs.last_cursor_position()
            } else {
                fallback_offset
            }
        }
    }

    /// Cursor screen x for `offset` (used by line-advance to lock
    /// `x_target` and by `x_target` initialization).
    fn x(&self, offset: Grapheme) -> Option<f32> {
        let frag = self.renderer.fragment_at_offset(offset)?;
        Some(self.renderer.fragment_x(frag, offset))
    }
}
