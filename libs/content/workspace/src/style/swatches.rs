//! Exclusive row of color dots — the accent picker.

use egui::{Color32, Rect, Response, Ui, pos2, vec2};

use crate::style::chrome::control_height;
use crate::style::color::{FG_HOVER, FG_PRESS, Theme};
use crate::style::interact::sense_click;
use crate::style::space::Space;

const DOT: Space = Space::Md;
const GAP: Space = Space::Sm;
/// The selected dot's center mark.
const MARK: Space = Space::Xs;

/// Natural width of [`swatches`] for `n` colors.
pub fn swatches_width(n: usize) -> f32 {
    n as f32 * DOT.pts() + n.saturating_sub(1) as f32 * GAP.pts()
}

/// One dot per color; `selected` is the index of the chosen one, if any.
/// Marks the response `.changed()` when `selected` updates.
pub fn swatches(
    ui: &mut Ui, t: &Theme, colors: &[Color32], selected: &mut Option<usize>,
) -> Response {
    let (row, mut resp) =
        ui.allocate_exact_size(vec2(swatches_width(colors.len()), control_height()), sense_click());
    let pitch = DOT.pts() + GAP.pts();
    let cell = |i: usize| {
        Rect::from_min_size(
            pos2(row.left() + i as f32 * pitch, row.top()),
            vec2(DOT.pts(), row.height()),
        )
    };
    let pointer = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|_| resp.hovered())
        .and_then(|p| {
            (0..colors.len()).find(|&i| cell(i).expand2(vec2(GAP.pts() / 2.0, 0.0)).contains(p))
        });

    if resp.clicked() {
        if let Some(i) = pointer.filter(|&i| Some(i) != *selected) {
            *selected = Some(i);
            resp.mark_changed();
        }
    }

    for (i, &color) in colors.iter().enumerate() {
        let wash = match pointer == Some(i) {
            true if resp.is_pointer_button_down_on() => FG_PRESS,
            true => FG_HOVER,
            false => 0.0,
        };
        let center = cell(i).center();
        ui.painter()
            .circle_filled(center, DOT.pts() / 2.0, t.wash_toward_neutral_fg(color, wash));
        if Some(i) == *selected {
            ui.painter()
                .circle_filled(center, MARK.pts() / 2.0, t.neutral_bg());
        }
    }
    resp
}
