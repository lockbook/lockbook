//! The nudge card above the sync footer: icon · sentence · X, then its button.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, FontFamily, FontId, Galley, Layout, Rect, Ui, pos2, vec2};
use workspace_rs::style::layout::PadContent;

use crate::components::{
    Button, Radius, Space, Theme, TypeRole, control_height, icon_button_hit, phosphor,
    phosphor_ui_font_id, place_at, tip_text,
};
use crate::shell::action::Action as A;

const PAD: Space = Space::Sm;
const GAP: Space = Space::Xs;
const TEXT: &str = "Sign in on **another device**.";

pub struct NudgeCard<'a> {
    pub t: &'a Theme,
    pub queue: &'a mut Vec<A>,
}

/// Where each part goes for a card `width` wide.
struct Metrics {
    text: Arc<Galley>,
    /// Square column for the icon and for the X, one line tall.
    mark: f32,
    height: f32,
}

impl NudgeCard<'_> {
    fn measure_parts(&self, ui: &Ui, width: f32) -> Metrics {
        let mark = TypeRole::Body.line_height();
        let (p, g) = (PAD.pts(), GAP.pts());
        let text_w = (width - 2.0 * p - 2.0 * mark - 2.0 * g).max(1.0);
        let text = ui.fonts_mut(|f| f.layout_job(sentence(TEXT, text_w, self.t)));
        let height = p + text.size().y.max(mark) + Space::Sm.pts() + control_height() + p;
        Metrics { text, mark, height }
    }
}

impl PadContent for NudgeCard<'_> {
    fn measure(&self, ui: &Ui, width: f32) -> f32 {
        self.measure_parts(ui, width).height
    }

    fn place(&mut self, ui: &mut Ui, rect: Rect) {
        let t = self.t;
        let m = self.measure_parts(ui, rect.width());
        let (p, g) = (PAD.pts(), GAP.pts());
        let ground = t.neutral_bg();
        ui.painter()
            .rect_filled(rect, Radius::Control.corner(), ground);

        let top = rect.top() + p;
        let left = rect.left() + p;
        let icon = ui.painter().layout_no_wrap(
            phosphor::QR_CODE.into(),
            phosphor_ui_font_id(),
            Color32::PLACEHOLDER,
        );
        let icon_at =
            pos2(left + (m.mark - icon.size().x) / 2.0, top + (m.mark - icon.size().y) / 2.0);
        ui.painter().galley(icon_at, icon, t.accent());

        let text_left = left + m.mark + g;
        ui.painter()
            .galley(pos2(text_left, top), m.text.clone(), t.neutral_fg());

        let x_rect =
            Rect::from_min_size(pos2(rect.right() - p - m.mark, top), vec2(m.mark, m.mark));
        place_at(ui, x_rect, Layout::top_down(Align::Min), |ui| {
            let x = icon_button_hit(ui, t, phosphor::X, false, ground, m.mark);
            tip_text(ui.ctx(), &x, "Dismiss");
            if x.clicked() {
                self.queue.push(A::NudgeDismiss);
            }
        });

        let button_top = top + m.text.size().y.max(m.mark) + Space::Sm.pts();
        let button = Rect::from_min_max(
            pos2(text_left, button_top),
            pos2(rect.right() - p, rect.bottom() - p),
        );
        place_at(ui, button, Layout::left_to_right(Align::Min), |ui| {
            if Button::secondary(t, "Show QR").show(ui).clicked() {
                self.queue.push(A::NudgeAccept);
            }
        });
    }
}

/// `text` wrapped to `width`, with `**…**` spans bold.
fn sentence(text: &str, width: f32, t: &Theme) -> LayoutJob {
    let mut job = LayoutJob {
        wrap: egui::text::TextWrapping { max_width: width, ..Default::default() },
        ..Default::default()
    };
    let size = TypeRole::Body.size();
    for (i, part) in text.split("**").enumerate() {
        let family = match i % 2 {
            0 => FontFamily::Proportional,
            _ => FontFamily::Name(Arc::from("Bold")),
        };
        let format = TextFormat {
            font_id: FontId::new(size, family),
            color: t.neutral_fg(),
            line_height: Some(TypeRole::Body.line_height()),
            ..Default::default()
        };
        job.append(part, 0.0, format);
    }
    job
}
