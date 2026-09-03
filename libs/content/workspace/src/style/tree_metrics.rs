//! File-tree geometry from [`Space`] + [`TypeRole`].
//!
//! A row is a small container around body type: line box + vertical pad.
//! Virtualized pitch uses [`ROW_H`] (not [`super::Spacer`]).

use super::space::Space;
use super::typography::TypeRole;

/// Vertical pad above/below the name line inside a tree row.
const ROW_PAD_Y: Space = Space::Xs;

/// Uniform row height: body line + pad top + pad bottom.
///
/// Same recipe as control height (`PAD_Y` = Xs around body line).
pub const ROW_H: f32 = TypeRole::Body.line_height() + ROW_PAD_Y.pts() * 2.0;

/// Leading inset before depth indent.
pub const INDENT_BASE: f32 = Space::Sm.pts();

/// Per-depth step.
pub const INDENT_STEP: f32 = Space::Md.pts();

/// Type icon column: body glyph + icon gap ([`Space::Xs`], control energy).
pub const ICON_SLOT: f32 = TypeRole::Body.size() + Space::Xs.pts();

// Chip-row gap + measure live in [`super::chip_layout`] (measure/draw one plan).
