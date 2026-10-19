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
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("workspace_control_insets"), self));
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
    ctx.data(|data| data.get_temp::<ControlInsets>(egui::Id::new("workspace_control_insets")))
        .unwrap_or_default()
        .inset(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserves_all_edges_without_changing_the_content_rect() {
        let content = egui::Rect::from_min_max(egui::pos2(5., 10.), egui::pos2(405., 810.));
        let controls =
            ControlInsets { top: 12., right: 16., bottom: 80., left: 20. }.inset(content);
        assert_eq!(controls.min, egui::pos2(25., 22.));
        assert_eq!(controls.max, egui::pos2(389., 730.));
        assert_eq!(content.size(), egui::vec2(400., 800.));
        assert_eq!(ControlInsets::default().inset(content), content);
    }

    #[test]
    fn oversized_insets_never_invert_the_controls_rect() {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(50., 60.));
        let controls =
            ControlInsets { top: 100., right: 100., bottom: 100., left: 100. }.inset(bounds);
        assert_eq!(controls.size(), egui::Vec2::ZERO);
        assert!(bounds.contains_rect(controls));
    }
}
