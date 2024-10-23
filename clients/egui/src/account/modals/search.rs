use egui::TextWrapMode;
use lb::blocking::Lb;
use lb::service::search::SearchResult;
use lb::Uuid;

use crate::model::DocType;

pub struct SearchModal {
    core: Lb,
    input: String,
    field_needs_focus: bool,
    results: Vec<SearchResult>,
    errors: Vec<String>,
    arrow_index: Option<usize>,
    arrowed_path: String,
}

impl SearchModal {
    pub fn new(core: Lb) -> Self {
        Self {
            core,
            input: String::new(),
            field_needs_focus: true,
            results: Vec::new(),
            errors: Vec::new(),
            arrow_index: None,
            arrowed_path: String::new(),
        }
    }

    pub fn focus_select_all(&mut self) {
        self.field_needs_focus = true;
    }

    fn process_keys(&mut self, etx: &egui::Context) {
        if etx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
            self.arrow_index = match self.arrow_index {
                Some(n) => {
                    if n == self.results.len() - 1 {
                        None
                    } else {
                        Some(n + 1)
                    }
                }
                None => Some(0),
            };
            self.ensure_arrowed_path();
        }

        if etx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
            self.arrow_index = match self.arrow_index {
                Some(0) => None,
                Some(n) => Some(n - 1),
                None => Some(self.results.len() - 1),
            };
            self.ensure_arrowed_path();
        }
    }

    fn ensure_arrowed_path(&mut self) {
        if let Some(i) = self.arrow_index {
            self.arrowed_path = self
                .results
                .get(i)
                .map(|res| res.path().to_string())
                .unwrap_or_default();
        }
    }

    fn show_search_result(
        &self, ui: &mut egui::Ui, res: &SearchResult, index: usize,
    ) -> egui::Response {
        let padding = egui::vec2(10.0, 20.0);
        let wrap_width = ui.available_width();

        let icon: egui::WidgetText = (&DocType::from_name(res.path())
            .to_icon()
            .size(30.0)
            .color(ui.visuals().text_color().gamma_multiply(0.5)))
            .into();
        let icon =
            icon.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);

        let name_text: egui::WidgetText = res.name().into();
        let name_text = name_text.into_galley(
            ui,
            Some(TextWrapMode::Extend),
            wrap_width,
            egui::TextStyle::Body,
        );

        let path_text: egui::WidgetText = res.path().into();
        let path_text = path_text
            .color(ui.visuals().text_color().gamma_multiply(0.7))
            .into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);

        let desired_size = egui::vec2(
            ui.available_size_before_wrap().x,
            name_text.size().y + path_text.size().y + padding.y * 2.0,
        );

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name_text.text())
        });

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&resp);

            let icon_pos =
                egui::pos2(rect.min.x + padding.x, rect.center().y - 0.5 * name_text.size().y);

            let name_text_pos = egui::pos2(
                rect.min.x + padding.x * 2.0 + icon.size().x,
                rect.min.y + name_text.size().y,
            );
            let path_text_pos = egui::pos2(
                rect.min.x + padding.x * 2.0 + icon.size().x,
                name_text_pos.y + path_text.size().y,
            );

            let maybe_fill =
                if self.arrow_index == Some(index) || (index == 0 && self.arrow_index.is_none()) {
                    Some(
                        ui.style()
                            .visuals
                            .widgets
                            .active
                            .bg_fill
                            .gamma_multiply(0.4),
                    )
                } else if resp.hovered() {
                    Some(
                        ui.style()
                            .visuals
                            .widgets
                            .active
                            .bg_fill
                            .gamma_multiply(0.1),
                    )
                } else {
                    None
                };

            if let Some(fill) = maybe_fill {
                ui.painter()
                    .rect(rect.expand(visuals.expansion), 0.0, fill, egui::Stroke::NONE);
            }

            let text_color = visuals.text_color();
            ui.painter().galley(icon_pos, icon, text_color);
            ui.painter().galley(name_text_pos, name_text, text_color);
            ui.painter().galley(path_text_pos, path_text, text_color);
        }

        resp
    }
}

impl super::Modal for SearchModal {
    const ANCHOR: egui::Align2 = egui::Align2::CENTER_TOP;
    const Y_OFFSET: f32 = 200.0;

    type Response = Option<SearchItemSelection>;

    /// Use a blank title so that the titlebar doesn't show.
    fn title(&self) -> &str {
        ""
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Option<SearchItemSelection> {
        if ui.input(|i| {
            i.events
                .iter()
                .any(|evt| matches!(evt, egui::Event::Text(_)))
        }) {
            self.arrow_index = None;
        }

        self.process_keys(ui.ctx());

        let mut resp = None;

        let buffer =
            if self.arrow_index.is_some() { &mut self.arrowed_path } else { &mut self.input };

        ui.set_width(600.0);

        let out = ui
            .vertical_centered(|ui| {
                egui::TextEdit::singleline(buffer)
                    .desired_width(f32::INFINITY)
                    .margin(egui::vec2(6.0, 6.0))
                    .show(ui)
            })
            .inner;

        if out.response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.results.is_empty()
        {
            let item = &self.results[self.arrow_index.unwrap_or(0)];
            resp = Some(SearchItemSelection { id: item.id(), close: true });
        } else {
            self.field_needs_focus = true;
        }

        if self.field_needs_focus {
            out.response.request_focus();
            ui.ctx().request_repaint();
            self.field_needs_focus = false;
        }

        if self.arrow_index.is_none() && out.response.changed() {
            match self
                .core
                .search_file_paths(&self.input)
                .map_err(|err| format!("{:?}", err))
            {
                Ok(results) => self.results = results,
                Err(err) => self.errors.push(err),
            }

            self.arrow_index = None;
        }

        if !self.errors.is_empty() {
            ui.add_space(5.0);

            for err in &self.errors {
                ui.label(err); // todo appear as error message, maybe icon plus red text
            }
        } else if !self.results.is_empty() {
            ui.add_space(5.0);

            egui::ScrollArea::vertical()
                .max_height(500.0)
                .show(ui, |ui| {
                    for (index, res) in self.results.iter().enumerate() {
                        if self.show_search_result(ui, res, index).clicked() {
                            let keep_open = {
                                let m = ui.input(|i| i.modifiers);
                                m.command && !m.alt && !m.shift
                            };
                            resp = Some(SearchItemSelection { id: res.id(), close: !keep_open });
                            self.field_needs_focus = true;
                            ui.ctx().request_repaint();
                        }
                    }
                });
        }

        resp
    }
}

pub struct SearchItemSelection {
    pub id: Uuid,
    pub close: bool,
}
