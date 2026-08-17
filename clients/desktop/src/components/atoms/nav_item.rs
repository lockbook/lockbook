//! Exclusive vertical nav row: icon + label, wash selection.
//!
//! Settings category rail (and similar). Generic atom — not product-shaped.
//! Lives on **surface** chrome; content pane is usually canvas.
//! Caller owns `selected`. Stack with **zero** vertical gap; wash is inset.
//!
//! ## Layout
//! Measure galleys → claim full-width band → place icon/label at absolute x
//! (no egui horizontal placer).

use egui::{
    Color32, Response, Ui, pos2, text::LayoutJob, text::TextFormat, text::TextWrapping, vec2,
};

use crate::components::foundation::chrome::{Radius, control_height, phosphor_ui_font_id};
use crate::components::foundation::color::{QUIET_PLATE_HOVER, QUIET_PLATE_PRESS, Theme};
use crate::components::foundation::interact::ControlFills;
use crate::components::foundation::layout::{inset, paint_control_pads};
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

/// Full-width exclusive nav row. Rest assumes surface ground (settings rail).
pub struct NavItem<'a> {
    tokens: &'a Theme,
    label: String,
    selected: bool,
    icon: &'static str,
}

impl<'a> NavItem<'a> {
    pub fn new(tokens: &'a Theme, label: impl Into<String>, icon: &'static str) -> Self {
        Self { tokens, label: label.into(), selected: false, icon }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        let pad_x = control_space::PAD_X.pts();
        let pad_y = control_space::PAD_Y.pts();
        let icon_gap = control_space::ICON_GAP.pts();
        let height = control_height();
        let width = crate::components::ui_width(ui).max(1.0);

        let (rect, response) =
            ui.allocate_at_least(vec2(width, height), crate::components::sense_click());

        let pointer_over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
        let hover_t = ui
            .ctx()
            .animate_bool(response.id.with("nav_hov"), pointer_over);

        let fills = self.fills();
        let fill = crate::components::foundation::interact::interact_fill(
            ui.ctx(),
            response.id,
            pointer_over,
            response.is_pointer_button_down_on(),
            response.clicked(),
            fills,
        );

        // Selected always fg; idle muted → fg on hover (choice UI).
        let ink = if self.selected {
            t.neutral_fg()
        } else {
            t.neutral_fg_secondary()
                .lerp_to_gamma(t.neutral_fg(), hover_t)
        };

        let radius = Radius::Control.corner();
        let wash = rect.shrink(crate::components::foundation::chrome::row_wash_inset());
        ui.painter().rect_filled(wash, radius, fill);

        let icon_galley = ui.painter().layout_no_wrap(
            self.icon.to_owned(),
            phosphor_ui_font_id(),
            Color32::PLACEHOLDER,
        );
        let icon_w = icon_galley.size().x;
        let label_max_w = (rect.width() - pad_x * 2.0 - icon_w - icon_gap).max(0.0);
        let label_galley = layout_truncate(ui, &self.label, label_max_w);

        paint_control_pads(ui, rect, control_space::PAD_X, control_space::PAD_Y);
        let mid = inset(rect, pad_x, pad_y);
        let row_h = mid.height().max(1.0);
        let mut x = mid.left();

        let ir = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(icon_w, row_h));
        paint_galley_layout_mid(ui.painter(), ir, icon_galley, ink);
        x += icon_w + icon_gap;

        let lr = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(label_max_w, row_h));
        paint_galley_layout_mid(ui.painter(), lr, label_galley, ink);

        response
    }

    fn fills(&self) -> ControlFills {
        let t = self.tokens;
        let rest = t.neutral_bg_secondary();
        // Selected uses hover-level wash (not press) so the settled state stays
        // calm on surface — press is only while the pointer is down.
        let hover = t.wash_toward_neutral_fg(rest, QUIET_PLATE_HOVER);
        let press = t.wash_toward_neutral_fg(rest, QUIET_PLATE_PRESS);

        if self.selected {
            ControlFills { rest: hover, hover, press }
        } else {
            ControlFills { rest, hover, press }
        }
    }
}

fn layout_truncate(ui: &Ui, label: &str, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: max_w.max(0.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..Default::default()
    };
    job.append(
        label,
        0.0,
        TextFormat {
            font_id: TypeRole::Body.font_id(),
            color: Color32::PLACEHOLDER,
            ..Default::default()
        },
    );
    ui.fonts(|f| f.layout_job(job))
}

/// Layout-box vertical mid in `slot` (no mesh_bounds — #21).
fn paint_galley_layout_mid(
    painter: &egui::Painter, slot: egui::Rect, galley: std::sync::Arc<egui::Galley>, color: Color32,
) {
    let pos = pos2(slot.left(), slot.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}
