use crate::{theme::icons::Icon, widgets::Button};

use super::{Toolbar, ToolbarContext, SCREEN_PADDING};

impl Toolbar {
    pub fn show_history_island(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> egui::Response {
        let history_island_x_start =
            tlbr_ctx.viewport_settings.container_rect.left() + SCREEN_PADDING;
        let history_island_y_start =
            tlbr_ctx.viewport_settings.container_rect.top() + SCREEN_PADDING;

        let history_rect = egui::Rect {
            min: egui::pos2(history_island_x_start, history_island_y_start),
            max: egui::Pos2 { x: history_island_x_start, y: history_island_y_start },
        };

        let res = ui.allocate_ui_at_rect(history_rect, |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let undo_btn = ui
                            .add_enabled_ui(tlbr_ctx.history.has_undo(), |ui| {
                                Button::default().icon(&Icon::UNDO).show(ui)
                            })
                            .inner;
                        if undo_btn.clicked() || undo_btn.drag_started() {
                            tlbr_ctx.history.undo(tlbr_ctx.buffer);
                        }

                        let redo_btn = ui
                            .add_enabled_ui(tlbr_ctx.history.has_redo(), |ui| {
                                Button::default().icon(&Icon::REDO).show(ui)
                            })
                            .inner;

                        if redo_btn.clicked() || redo_btn.drag_started() {
                            tlbr_ctx.history.redo(tlbr_ctx.buffer);
                        }
                    })
                })
        });
        self.layout.history_island = Some(res.response.rect);
        res.inner.response
    }
}
