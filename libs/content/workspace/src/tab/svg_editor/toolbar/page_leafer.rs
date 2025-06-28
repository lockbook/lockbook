use core::f32;

use egui::{Response, WidgetText};

use crate::{
    tab::svg_editor::toolbar::{Toolbar, ToolbarContext, SCREEN_PADDING},
    theme::icons::Icon,
    widgets::Button,
};

#[derive(Default)]
pub struct PageLeafer {
    page_edit: Option<String>,
}

impl Toolbar {
    pub fn show_page_leafer(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<Response> {
        let bounded_rect = match tlbr_ctx.viewport_settings.bounded_rect {
            Some(val) => val,
            None => return None,
        };

        let mock_leafer_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 10.0));

        let overlay_toggle = self.layout.overlay_toggle?;
        let leafer_island_start_x = overlay_toggle.left()
            - self
                .layout
                .leafer_island
                .unwrap_or(mock_leafer_rect)
                .width()
            - SCREEN_PADDING;

        let leafer_island_start_y = overlay_toggle.top();

        let viewport_rect = egui::Rect {
            min: egui::pos2(leafer_island_start_x, leafer_island_start_y),
            max: egui::Pos2 { x: leafer_island_start_x, y: overlay_toggle.bottom() },
        };

        ui.painter()
            .rect_filled(viewport_rect, 0.0, egui::Color32::DEBUG_COLOR);

        let mut island_res = ui
            .allocate_ui_at_rect(viewport_rect, |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                    .show(ui, |ui| self.show_inner_leaf_island(ui, tlbr_ctx))
            })
            .inner
            .response;

        self.layout.leafer_island = Some(island_res.rect);
        Some(island_res)
    }

    fn show_inner_leaf_island(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Response {
        ui.set_height(ui.available_height());
        ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
            ui.spacing_mut().button_padding = egui::vec2(6.0, 5.0);

            ui.add_space(7.0);
            ui.label(egui::RichText::new("Page").color(egui::Color32::GRAY));

            let page_num = -(tlbr_ctx.viewport_settings.master_transform.ty
                / tlbr_ctx.viewport_settings.bounded_rect.unwrap().height())
            .ceil() as usize
                + 1;

            if Button::default()
                .icon(&Icon::ARROW_DOWN.size(14.0))
                .show(ui)
                .clicked()
            {}
            let page_num_label: WidgetText = page_num.to_string().into();
            let galley = page_num_label.into_galley(ui, None, f32::INFINITY, egui::TextStyle::Body);

            let (page_num_rect, page_num_res) =
                ui.allocate_exact_size(galley.size() + egui::vec2(10.0, 0.0), egui::Sense::click());
            if self.page_leafer.page_edit.is_none() {
                ui.painter().galley(
                    page_num_rect.center_top() - egui::vec2(galley.size().x / 2.0, 0.0),
                    galley,
                    ui.visuals().text_color(),
                );
                ui.painter().rect_stroke(
                    page_num_rect,
                    3.0,
                    egui::Stroke { width: 1.5, color: egui::Color32::GRAY.linear_multiply(0.8) },
                );
            }
            // ui.advance_cursor_after_rect(page_num_rect);

            // let border_length = page_num_rect.width() + 10.0;
            // let page_num_rect = egui::Rect::from_center_size(
            //     page_num_rect.center(),
            //     egui::vec2(border_length, border_length),
            // );

            if page_num_res.clicked() {
                self.page_leafer.page_edit = Some(page_num.to_string());
                let mut rename_edit_state = egui::text_edit::TextEditState::default();
                rename_edit_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange {
                        primary: egui::text::CCursor::new(0),
                        secondary: egui::text::CCursor::new(page_num.to_string().len()),
                    }));
                egui::TextEdit::store_state(
                    ui.ctx(),
                    egui::Id::new("rename_page_num"),
                    rename_edit_state,
                );
            }

            let mut dismiss_text_edit = false;
            if let Some(text) = &mut self.page_leafer.page_edit {
                let mut child_ui = ui.child_ui(page_num_rect, egui::Layout::default(), None);
                let text_edit = egui::TextEdit::singleline(text)
                    .desired_width(page_num_rect.width())
                    .frame(false)
                    .id("rename_page_num".into());

                let text_edit_res = child_ui.add(text_edit);
                text_edit_res.request_focus();
                if child_ui.input(|r| r.key_pressed(egui::Key::Enter)) || text_edit_res.lost_focus()
                {
                    dismiss_text_edit = true;
                }
                // ui.scope(|ui| {
                //     ui.allocate_ui_at_rect(page_num_rect, |ui| {
                //         let text_edit = egui::TextEdit::singleline(text)
                //             .desired_width(page_num_rect.width())
                //             .frame(false);
                //         let text_edit_res = ui.add(text_edit);
                //         if ui.input(|r| r.key_pressed(egui::Key::Enter))
                //             || text_edit_res.lost_focus()
                //         {
                //             dismiss_text_edit = true;
                //         }
                //     });
                // });
            }

            if dismiss_text_edit {
                self.page_leafer.page_edit = None;
            }

            if Button::default()
                .icon(&Icon::ARROW_UP.size(14.0))
                .show(ui)
                .clicked()
            {}

            // ui.add_space(5.0);
            // button
        })
        .response
    }

    /// given a page number, change the viewport by applying a transform such that a specific page 
    /// is focused. 
    fn scroll_to_page(
        &mut self, ui: &mut egui::Ui, page_number: usize, tlbr_ctx: &mut ToolbarContext,
    ) {
    }
}
