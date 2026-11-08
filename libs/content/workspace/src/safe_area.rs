//! Safe areas reserve room for platform chrome around overlay controls, never document content.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl ControlInsets {
    pub fn set(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::unique("workspace_control_insets"), self));
    }

    pub fn inset(self, bounds: egui::Rect) -> egui::Rect {
        let min = egui::pos2(
            (bounds.left() + self.left.max(0.)).min(bounds.right()),
            (bounds.top() + self.top.max(0.)).min(bounds.bottom()),
        );
        let max = egui::pos2(
            (bounds.right() - self.right.max(0.)).max(min.x),
            (bounds.bottom() - self.bottom.max(0.)).max(min.y),
        );
        egui::Rect::from_min_max(min, max)
    }
}

pub fn controls_rect(ui: &egui::Ui) -> egui::Rect {
    inset_rect(ui.ctx(), ui.available_rect_before_wrap())
}

pub fn inset_rect(ctx: &egui::Context, bounds: egui::Rect) -> egui::Rect {
    ctx.data(|data| data.get_temp::<ControlInsets>(egui::Id::unique("workspace_control_insets")))
        .unwrap_or_default()
        .inset(bounds)
}
