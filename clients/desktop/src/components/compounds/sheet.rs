//! Task-sheet chrome: dim, canvas panel, header, footer.
//!
//! Core plate/dim/title/footer live in [`workspace_rs::style::sheet`]. This
//! module re-exports them and keeps desktop band helpers.

use egui::{Layout, Ui, pos2, vec2};

use crate::components::foundation::layout::{claim, origin, place_at};
use crate::components::foundation::space::Space;

pub use workspace_rs::style::{
    SheetFooterOpts, sheet_dim, sheet_footer, sheet_panel_fit, sheet_panel_fixed, sheet_title_muted,
};

// ── Measure+place section helpers (Tier B) ──────────────────────────────────

/// Place content in a parent-owned band of height `h` (full current ui width).
///
/// Top-down Min — no residual `available_height` fill. Parent claims the band.
pub fn sheet_band(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui)) {
    let h = h.max(1.0);
    let w = crate::components::ui_width(ui);
    let top_left = origin(ui);
    let band = egui::Rect::from_min_size(top_left, vec2(w, h));
    let _ = place_at(ui, band, Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(w);
        ui.set_min_height(h);
        ui.set_height(h);
        ui.set_max_height(h);
        add(ui);
    });
    claim(ui, band);
}

/// Center `add` inside a fixed band (empty states, spinners) without
/// `vertical_centered` on an unconstrained Area.
pub fn sheet_band_centered(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui)) {
    let h = h.max(1.0);
    let w = crate::components::ui_width(ui);
    let top_left = origin(ui);
    let band = egui::Rect::from_min_size(top_left, vec2(w, h));
    let _ = place_at(ui, band, Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(w);
        ui.with_layout(Layout::top_down(egui::Align::Center), add);
    });
    claim(ui, band);
}

/// Place equal-width (or pre-measured) cells in one row at absolute x.
///
/// `widths[i]` is cell width; gaps use [`super::super::atoms::chip_layout::CHIP_GAP`]
/// via `gap_pts` / paint. Caller owns total claim via this helper.
pub fn sheet_equal_row(
    ui: &mut Ui, heights: f32, widths: &[f32], gap: Space, mut cell: impl FnMut(&mut Ui, usize),
) {
    use crate::components::foundation::spacer::Spacer;
    let h = heights.max(1.0);
    let gap_pts = gap.pts();
    let top_left = origin(ui);
    let mut x = top_left.x;
    let mut total_w = 0.0_f32;
    for (i, &w) in widths.iter().enumerate() {
        if i > 0 {
            Spacer::paint_at(
                ui,
                gap,
                egui::Rect::from_min_size(pos2(x, top_left.y), vec2(gap_pts, h)),
            );
            x += gap_pts;
            total_w += gap_pts;
        }
        let cell_r = egui::Rect::from_min_size(pos2(x, top_left.y), vec2(w.max(0.0), h));
        let _ = place_at(ui, cell_r, Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(w.max(0.0));
            ui.set_height(h);
            cell(ui, i);
        });
        x += w.max(0.0);
        total_w += w.max(0.0);
    }
    claim(ui, egui::Rect::from_min_size(top_left, vec2(total_w.max(1.0), h)));
}
