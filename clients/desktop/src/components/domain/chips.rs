//! Files-pane action chips: Create / Import / Search.
//!
//! Equal-width quiet chips ([`quiet_chip`]) on canvas.
//! Widths from [`EqualCells::measure`]; cells placed at absolute x.

use egui::{Align, Layout};
use egui::{Ui, pos2, vec2};

use crate::components::{
    EqualCells, QuietChipAlign, QuietChipLabel, Spacer, Theme, claim, origin, phosphor, place_at,
    quiet_chip, quiet_chip_height, quiet_chip_labeled_min_width, tip_text, ui_width,
};

/// Equal-width Create / Import / Search.
///
/// Returns click flags + each chip’s allocated rect (Create, Import, Search).
pub fn action_chip_row(ui: &mut Ui, t: &Theme) -> (bool, bool, bool, [egui::Rect; 3]) {
    let specs = [
        (phosphor::NOTE_PENCIL, "Create"),
        (phosphor::DOWNLOAD_SIMPLE, "Import"),
        (phosphor::SEARCH, "Search"),
    ];
    let n = specs.len();
    let row_w = ui_width(ui);
    let cells = EqualCells::measure(row_w, n);
    let labels: Vec<&str> = specs.iter().map(|(_, l)| *l).collect();
    let icon_only = cells.cell_w < quiet_chip_labeled_min_width(ui, t, &labels);
    let mut hits = [false; 3];
    let mut rects = [egui::Rect::NOTHING; 3];
    let row_h = quiet_chip_height();
    let gap = EqualCells::gap_pts();
    let top_left = origin(ui);
    let mut x = top_left.x;

    for (i, (icon, label)) in specs.iter().enumerate() {
        if i > 0 {
            Spacer::paint_at(
                ui,
                EqualCells::gap_token(),
                egui::Rect::from_min_size(pos2(x, top_left.y), vec2(gap, row_h)),
            );
            x += gap;
        }
        let cell = egui::Rect::from_min_size(pos2(x, top_left.y), vec2(cells.cell_w, row_h));
        let (clicked, used) = place_at(ui, cell, Layout::top_down(Align::Min), |ui| {
            ui.set_width(cells.cell_w);
            ui.set_min_height(row_h);
            ui.set_height(row_h);
            let label_arg = if icon_only { None } else { Some(QuietChipLabel::Plain(label)) };
            let resp = quiet_chip(ui, t, icon, t.neutral_fg(), label_arg, QuietChipAlign::Center);
            if icon_only {
                tip_text(ui.ctx(), &resp, *label);
            }
            resp.clicked()
        });
        hits[i] = clicked;
        rects[i] = used;
        x += cells.cell_w;
    }

    claim(ui, egui::Rect::from_min_size(top_left, vec2(row_w, row_h)));
    (hits[0], hits[1], hits[2], rects)
}
