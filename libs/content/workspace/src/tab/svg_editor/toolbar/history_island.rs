use egui::UiBuilder;

use crate::style::{ThemeExt as _, island, phosphor};

use super::{SCREEN_PADDING, Toolbar, ToolbarContext, island_icon};

impl Toolbar {
    pub fn show_history_island(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> (egui::Response, bool) {
        let mut dirty = false;

        let history_island_x_start =
            tlbr_ctx.viewport_settings.container_rect.left() + SCREEN_PADDING.x;
        let history_island_y_start =
            tlbr_ctx.viewport_settings.container_rect.top() + SCREEN_PADDING.y;

        let history_rect = egui::Rect {
            min: egui::pos2(history_island_x_start, history_island_y_start),
            max: egui::Pos2 { x: history_island_x_start, y: history_island_y_start },
        };

        let t = ui.ctx().get_lb_theme();
        let res = ui.scope_builder(UiBuilder::new().max_rect(history_rect), |ui| {
            island::frame(&t).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    let can_undo = tlbr_ctx.history.has_undo();
                    let undo_btn =
                        island_icon(ui, &t, phosphor::ARROW_COUNTER_CLOCKWISE, can_undo, "Undo");
                    if can_undo && (undo_btn.clicked() || undo_btn.drag_started()) {
                        tlbr_ctx.history.undo(tlbr_ctx.buffer);
                        dirty = true;
                    }

                    let can_redo = tlbr_ctx.history.has_redo();
                    let redo_btn = island_icon(ui, &t, phosphor::ARROW_CLOCKWISE, can_redo, "Redo");
                    if can_redo && (redo_btn.clicked() || redo_btn.drag_started()) {
                        tlbr_ctx.history.redo(tlbr_ctx.buffer);
                        dirty = true;
                    }
                })
            })
        });
        self.layout.history_island = Some(res.response.rect);
        (res.inner.response, dirty)
    }
}
