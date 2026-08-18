//! Measure / draw for equal-cell chip strips (action row, pin grid, similar).
//!
//! ## Measure / draw (one plan)
//!
//! Parents that need size without painting call [`EqualCells::measure`]. That is
//! the **only** place that turns `available_w` + cell count into cell widths and
//! that owns the inter-cell gap token ([`CHIP_GAP`]).
//!
//! Draw reuses that measure (`cell_w`) and inserts gaps **only** via
//! [`EqualCells::gap_spacer`] (or `item_spacing` set to [`EqualCells::gap_pts`]).
//! Never budget with `CHIP_GAP` and insert `Spacer::Sm` (or any other token) —
//! that is two sources of truth and breaks under SidePanel width walk.
//!
//! Expensive shaping can cache later; chip cells are cheap pts arithmetic.

use crate::components::foundation::space::Space;
use crate::components::foundation::spacer::Spacer;

/// Sole inter-cell gap for equal-cell chip rows / pin grids.
pub const CHIP_GAP: Space = Space::Xs;

/// Result of measuring an equal-width cell row (or one grid row of `n` columns).
#[derive(Clone, Copy, Debug)]
pub struct EqualCells {
    /// Cell count used in the measure (after `n.max(1)`).
    #[allow(dead_code)] // for parents that need count without re-stating `n`
    pub n: usize,
    /// Width of each cell.
    pub cell_w: f32,
}

impl EqualCells {
    /// Measure only — no draw. Gap count is `n.saturating_sub(1)` × [`CHIP_GAP`].
    pub fn measure(available_w: f32, n: usize) -> Self {
        let n = n.max(1);
        let gaps = Self::gap_pts() * (n - 1) as f32;
        let cell_w = ((available_w - gaps) / n as f32).max(0.0);
        Self { n, cell_w }
    }

    /// Gap width used in [`measure`] — for height math (row gaps) and wrap spacing.
    #[inline]
    pub fn gap_pts() -> f32 {
        CHIP_GAP.pts()
    }

    /// Gap token used in [`measure`] / [`gap_spacer`] (one plan).
    #[inline]
    pub fn gap_token() -> Space {
        CHIP_GAP
    }

    /// The only legal gap between cells of an [`EqualCells`] row.
    #[inline]
    pub fn gap_spacer() -> Spacer {
        Spacer::new(CHIP_GAP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_fills_available_width() {
        let m = EqualCells::measure(200.0, 3);
        let drawn = m.cell_w * 3.0 + EqualCells::gap_pts() * 2.0;
        assert!((drawn - 200.0).abs() < 1e-4);
    }
}
