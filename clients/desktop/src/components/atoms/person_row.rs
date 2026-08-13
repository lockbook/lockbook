//! Two-line person / result row — share typeahead, roster, invite status.
//!
//! ## Product roles (keep signals aligned)
//! | Role | Leading icon | Status |
//! |------|--------------|--------|
//! | **Suggest / stage** | `USER_PLUS` | “Recent”, or none |
//! | **Lookup sticky** | spinner / check / x | “Checking…”, “User found”, “Not found” |
//! | **On this file** | `USER` / `PENCIL_CIRCLE` / `EYE` | “Owner”, “Can edit”, “Can view” [· Via …] |
//! | **Pending (staged)** | chip (no icon) | username; dismiss × |
//!
//! Access list: icon = **permission** (owner / edit / view). Inheritance is
//! status copy only — not a folder glyph.
//!
//! Title stays full `fg`. Status + icon use [`PersonTone`].

use egui::{
    Color32, Response, Ui, pos2, text::LayoutJob, text::TextFormat, text::TextWrapping, vec2,
};

use crate::components::foundation::chrome::{
    Radius, control_height, phosphor, phosphor_ui_font_id, row_wash_inset,
};
use crate::components::foundation::color::Theme;
use crate::components::foundation::interact::{interact_fill, quiet_canvas_fills, sense_click};
use crate::components::foundation::layout::{inset, paint_control_pads};
use crate::components::foundation::space::Space;
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

/// Semantic weight for leading icon and status line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PersonTone {
    /// Default meta — muted.
    #[default]
    Neutral,
    /// Lookup in flight.
    Progress,
    /// Found / ok / already known.
    Ok,
}

impl PersonTone {
    pub fn ink(self, t: &Theme) -> Color32 {
        match self {
            Self::Neutral | Self::Progress => t.neutral_fg_secondary(),
            Self::Ok => t.accent(),
        }
    }
}

/// Full-width list row for people / invite hits. Canvas ground (sheet body).
pub struct PersonRow<'a> {
    tokens: &'a Theme,
    title: String,
    status: Option<String>,
    icon: Option<&'static str>,
    tone: PersonTone,
}

/// Measured pitch for a person row — **same stack as [`PersonRow::show`]**.
///
/// Use this for list viewports (`fixed_height_list` × N). Do not guess with
/// `control_height() + line_height`; galley metrics differ from em-box line height.
pub fn person_row_height(ui: &Ui, with_status: bool) -> f32 {
    let pad_y = control_space::PAD_Y.pts();
    let title_h = ui
        .painter()
        .layout_no_wrap("Ag".into(), TypeRole::Body.font_id(), Color32::WHITE)
        .size()
        .y;
    let stack_h = if with_status {
        let status_h = ui
            .painter()
            .layout_no_wrap("Ag".into(), TypeRole::Mono.font_id(), Color32::WHITE)
            .size()
            .y;
        title_h + Space::Xxs.pts() + status_h
    } else {
        title_h
    };
    (pad_y * 2.0 + stack_h).max(control_height())
}

impl<'a> PersonRow<'a> {
    pub fn new(tokens: &'a Theme, title: impl Into<String>) -> Self {
        Self {
            tokens,
            title: title.into(),
            status: None,
            icon: Some(phosphor::USER),
            tone: PersonTone::Neutral,
        }
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn icon(mut self, glyph: Option<&'static str>) -> Self {
        self.icon = glyph;
        self
    }

    pub fn tone(mut self, tone: PersonTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        let pad_x = control_space::PAD_X;
        let pad_y = control_space::PAD_Y;
        let icon_gap = control_space::ICON_GAP;
        let detail_gap = Space::Xxs;

        let title_font = TypeRole::Body.font_id();
        let status_font = TypeRole::Mono.font_id();
        let title_g = ui.painter().layout_no_wrap(
            self.title.clone(),
            title_font.clone(),
            Color32::PLACEHOLDER,
        );
        let status_g = self.status.as_ref().map(|s| {
            ui.painter()
                .layout_no_wrap(s.clone(), status_font.clone(), Color32::PLACEHOLDER)
        });

        let stack_h = if let Some(ref sg) = status_g {
            title_g.size().y + detail_gap.pts() + sg.size().y
        } else {
            title_g.size().y
        };
        // Keep in lockstep with [`person_row_height`].
        let height = (pad_y.pts() * 2.0 + stack_h).max(control_height());
        let width = crate::components::ui_width(ui).max(1.0);

        // sense_click (no FOCUSABLE) — must not steal sticky text-edit focus
        // when used under Share / other sheets (sticky text focus).
        let (rect, response) = ui.allocate_at_least(vec2(width, height), sense_click());
        let pointer_over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
        let over = pointer_over;

        let fills = quiet_canvas_fills(t);
        let fill = interact_fill(
            ui.ctx(),
            response.id,
            over,
            response.is_pointer_button_down_on(),
            response.clicked(),
            fills,
        );
        // Same 1 px all-sides inset as file rows (separation without side gutters).
        let wash = rect.shrink(row_wash_inset());
        ui.painter()
            .rect_filled(wash, Radius::Control.corner(), fill);

        let tone_ink = self.tone.ink(t);
        let title_ink = t.neutral_fg();

        let icon_g = self.icon.map(|icon| {
            ui.painter().layout_no_wrap(
                icon.to_owned(),
                phosphor_ui_font_id(),
                Color32::PLACEHOLDER,
            )
        });
        let icon_block = icon_g
            .as_ref()
            .map(|g| g.size().x + icon_gap.pts())
            .unwrap_or(0.0);

        // Clip to row only via painter — never set_clip_rect on a child Ui that
        // could grow past a ScrollArea viewport (clip shrink).
        paint_control_pads(ui, rect, pad_x, pad_y);
        let mid = inset(rect, pad_x.pts(), pad_y.pts());
        let mid_h = stack_h.max(title_g.size().y).max(mid.height());
        let stack_top = mid.center().y - mid_h / 2.0;
        let mut x = mid.left();

        if let Some(ig) = icon_g {
            let ir = egui::Rect::from_min_size(pos2(x, stack_top), vec2(ig.size().x, mid_h));
            let icon_pos = pos2(ir.left(), ir.center().y - ig.size().y / 2.0);
            if self.tone == PersonTone::Progress {
                let angle =
                    (ui.input(|i| i.time) as f32 * std::f32::consts::TAU) % std::f32::consts::TAU;
                let shape = egui::epaint::TextShape::new(icon_pos, ig, tone_ink)
                    .with_override_text_color(tone_ink)
                    .with_angle_and_anchor(angle, egui::Align2::CENTER_CENTER);
                ui.painter().add(shape);
                ui.ctx().request_repaint();
            } else {
                ui.painter().galley(icon_pos, ig, tone_ink);
            }
            x += icon_block;
        }

        let text_w = (mid.right() - x).max(1.0);
        let title_trunc = layout_truncate(ui, &self.title, text_w, &title_font);
        let mut y = stack_top;
        ui.painter()
            .galley(pos2(x, y), title_trunc.clone(), title_ink);
        y += title_trunc.size().y;
        if status_g.is_some() {
            y += detail_gap.pts();
            let st = self.status.as_deref().unwrap_or("");
            let status_trunc = layout_truncate(ui, st, text_w, &status_font);
            ui.painter().galley(pos2(x, y), status_trunc, tone_ink);
        }

        response
    }
}

fn layout_truncate(
    ui: &Ui, text: &str, max_w: f32, font: &egui::FontId,
) -> std::sync::Arc<egui::Galley> {
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
        text,
        0.0,
        TextFormat { font_id: font.clone(), color: Color32::PLACEHOLDER, ..Default::default() },
    );
    ui.fonts(|f| f.layout_job(job))
}
