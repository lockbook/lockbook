//! Type-safe text addressing — companion to [`offset_types`][parent].
//!
//! `offset_types` provides the position/count newtypes ([`Byte`],
//! [`Bytes`], [`Grapheme`], [`Graphemes`]) and their algebra. This module
//! adds the *unit-aware constructors* that make those types load-bearing
//! against the bug class we kept hitting:
//!
//! - [`Graphemes::measure_replace`] — actual graphemes contributed by a
//!   Replace, accounting for seam fusion (Devanagari spacing marks, ZWJ
//!   sequences). The OT-correct number; the only constructor for an
//!   OT-suitable `Graphemes`.
//! - [`Graphemes::from_isolated_str`] — in-isolation count (what
//!   `text.graphemes(true).count()` returns). Named `_isolated_` so misuse
//!   stands out in review; legitimate for display widths but **wrong** for
//!   OT or cursor placement.
//! - [`UnicodeSegs::byte_to_grapheme_strict`] / `_floor` / `_ceil` —
//!   conversions from a non-grapheme-aware byte source (cosmic-text glyphs,
//!   comrak sourcepos). Strict returns a `Result`; the snapping variants
//!   round to the nearest cluster boundary.
//!
//! [parent]: super::offset_types
//!
//! ## Codepoint type
//!
//! [`Codepoint`] / [`Codepoints`] live here rather than in `offset_types`
//! because the legacy code never had a codepoint unit — codepoints are
//! introduced as part of the type-safety pass to make conversions through
//! cosmic-text glyph positions and comrak sourcepos explicit.

use super::offset_types::{Byte, Grapheme, Graphemes};
use super::unicode_segs::UnicodeSegs;
use unicode_segmentation::UnicodeSegmentation;

// ─── Codepoint unit ──────────────────────────────────────────────────────

/// Unicode scalar value index. Each value corresponds to a Rust `char`
/// (U+0000 to U+10FFFF excluding surrogates).
#[repr(transparent)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Codepoint(pub usize);

/// A count of Unicode scalar values.
#[repr(transparent)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Codepoints(pub usize);

impl Codepoints {
    /// Codepoint count of `s`. Always correct — codepoints are local to
    /// the string, no fusion concerns.
    pub fn measure(s: &str) -> Self {
        Self(s.chars().count())
    }
}

// ─── `Graphemes` measurement (the load-bearing OT constructor) ──────────

impl Graphemes {
    /// Actual graphemes contributed by replacing `replaced` (in `old_segs`)
    /// with new text whose effect is captured by `new_segs`. Accounts for
    /// seam fusion at the boundaries of the replaced range — this is the
    /// number that OT position math requires.
    ///
    /// The math: new buffer's grapheme count = old count − replaced + actual,
    /// so actual = (new_total + replaced) − old_total.
    pub fn measure_replace(
        old_segs: &UnicodeSegs, new_segs: &UnicodeSegs, replaced: (Grapheme, Grapheme),
    ) -> Self {
        let old_total = old_segs.last_grapheme();
        let new_total = new_segs.last_grapheme();
        let replaced_len = replaced.1 - replaced.0;
        // (new_total + replaced_len) - old_total — order matters because
        // `Grapheme::Sub` is saturating; `new_total - old_total` could
        // underflow when the replace shrinks the buffer.
        Self((new_total.0 + replaced_len.0).saturating_sub(old_total.0))
    }

    /// Grapheme count of `s` *in isolation* — what `unicode_segmentation`
    /// reports without context. **Under-counts** when `s` is later spliced
    /// into a buffer where its boundary characters fuse with neighbors (a
    /// Devanagari spacing mark joining the preceding consonant; a ZWJ
    /// joining adjacent emoji into one cluster).
    ///
    /// Use only for purposes that genuinely want the in-isolation count
    /// (display widths, soft constraints). For OT or cursor placement, use
    /// [`Graphemes::measure_replace`] instead.
    pub fn from_isolated_str(s: &str) -> Self {
        Self(s.graphemes(true).count())
    }
}

// ─── Conversions on `UnicodeSegs` ────────────────────────────────────────

/// Returned when a strict byte→grapheme conversion is asked for a byte that
/// doesn't lie on a grapheme cluster boundary.
#[derive(Debug)]
pub enum BoundaryError {
    NotGraphemeAligned(Byte),
}

impl UnicodeSegs {
    /// Last valid grapheme position — i.e. the position one past the last
    /// grapheme cluster, where the cursor sits at end-of-buffer.
    pub fn last_grapheme(&self) -> Grapheme {
        Grapheme(self.grapheme_indexes.len().saturating_sub(1))
    }

    /// Strict: byte must be on a grapheme boundary.
    pub fn byte_to_grapheme_strict(&self, b: Byte) -> Result<Grapheme, BoundaryError> {
        match self.grapheme_indexes.binary_search(&b) {
            Ok(i) => Ok(Grapheme(i)),
            Err(_) => Err(BoundaryError::NotGraphemeAligned(b)),
        }
    }

    /// Snap down to the start of the cluster containing `b`. Use for
    /// inclusive boundaries from a non-grapheme-aware source.
    pub fn byte_to_grapheme_floor(&self, b: Byte) -> Grapheme {
        match self.grapheme_indexes.binary_search(&b) {
            Ok(i) => Grapheme(i),
            Err(i) => Grapheme(i.saturating_sub(1)),
        }
    }

    /// Snap up to the start of the next cluster. Use for exclusive
    /// boundaries from a non-grapheme-aware source — the cluster containing
    /// `b` ends up included in whatever range this byte terminates.
    pub fn byte_to_grapheme_ceil(&self, b: Byte) -> Grapheme {
        match self.grapheme_indexes.binary_search(&b) {
            Ok(i) => Grapheme(i),
            Err(i) => Grapheme(i.min(self.grapheme_indexes.len().saturating_sub(1))),
        }
    }

    /// Always-safe direction.
    pub fn grapheme_to_byte(&self, g: Grapheme) -> Byte {
        self.grapheme_indexes[g.0]
    }
}
