use std::sync::{mpsc, Arc, RwLock};
use std::thread;

use eframe::egui;

use crate::model::DocType;

struct SearchResultItem {
    id: lb::Uuid,
    path: String,
    name: String,
}
pub struct SearchModal {
    requests: mpsc::Sender<String>,
    responses: mpsc::Receiver<Result<Vec<SearchResultItem>, String>>,
    input: String,
    field_needs_focus: bool,
    is_searching: Arc<RwLock<bool>>,
    results: Vec<SearchResultItem>,
    errors: Vec<String>,
    arrow_index: Option<usize>,
    arrowed_path: String,
}

impl SearchModal {
    pub fn new(core: &lb::Core, etx: &egui::Context) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<String>();
        let (response_tx, response_rx) = mpsc::channel();

        let is_searching = Arc::new(RwLock::new(false));

        thread::spawn({
            let is_searching = is_searching.clone();
            let core = core.clone();
            let etx = etx.clone();

            move || {
                while let Ok(input) = request_rx.recv() {
                    *is_searching.write().unwrap() = true;
                    etx.request_repaint();

                    let res = core
                        .search_file_paths(&input)
                        .map_err(|err| format!("{:?}", err))
                        .map(|search_results| {
                            search_results
                                .iter()
                                .map(|sr| SearchResultItem {
                                    id: sr.id,
                                    path: sr.path.clone(),
                                    name: core.get_file_by_id(sr.id).unwrap().name,
                                })
                                .collect()
                        });
                    response_tx.send(res).unwrap();

                    *is_searching.write().unwrap() = false;
                    etx.request_repaint();
                }
            }
        });

        Self {
            requests: request_tx,
            responses: response_rx,
            input: String::new(),
            field_needs_focus: true,
            is_searching,
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
                .map(|res| res.path.clone())
                .unwrap_or_default();
        }
    }

    fn draw_search_result(
        &self, ui: &mut egui::Ui, res: &SearchResultItem, index: usize,
    ) -> egui::Response {
        let padding = egui::vec2(10.0, 20.0);
        let wrap_width = ui.available_width();

        let icon: egui::WidgetText = (&DocType::from_name(res.path.as_str())
            .to_icon()
            .size(30.0)
            .color(ui.visuals().text_color().gamma_multiply(0.5)))
            .into();
        let icon = icon.into_galley(ui, Some(true), wrap_width, egui::TextStyle::Body);

        let name_text: egui::WidgetText = (&res.name).into();
        let name_text = name_text.into_galley(ui, Some(true), wrap_width, egui::TextStyle::Body);

        let path_text: egui::WidgetText = (&res.path).into();
        let path_text = path_text
            .color(ui.visuals().text_color().gamma_multiply(0.7))
            .into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

        let desired_size = egui::vec2(
            ui.available_size_before_wrap().x,
            name_text.size().y + path_text.size().y + padding.y * 2.0,
        );

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, name_text.text()));

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
                if self.arrow_index == Some(index) || (index == 0 && self.arrow_index == None) {
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

            icon.paint_with_visuals(ui.painter(), icon_pos, visuals);
            name_text.paint_with_visuals(ui.painter(), name_text_pos, visuals);
            path_text.paint_with_visuals(ui.painter(), path_text_pos, visuals);
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
        while let Ok(res) = self.responses.try_recv() {
            match res {
                Ok(results) => {
                    self.arrow_index = None;
                    self.results = results;
                }
                Err(msg) => self.errors.push(msg),
            }
        }

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

        let out = egui::TextEdit::singleline(buffer)
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(6.0, 6.0))
            .show(ui);

        if out.response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.results.is_empty()
        {
            let item = &self.results[self.arrow_index.unwrap_or(0)];
            resp = Some(SearchItemSelection { id: item.id, close: true });
        } else {
            self.field_needs_focus = true;
        }

        if self.field_needs_focus {
            out.response.request_focus();
            ui.ctx().request_repaint();
            self.field_needs_focus = false;
        }

        if self.arrow_index.is_none() && out.response.changed() {
            self.requests.send(self.input.clone()).unwrap();
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
                        if self.draw_search_result(ui, res, index).clicked() {
                            let keep_open = {
                                let m = ui.input(|i| i.modifiers);
                                m.ctrl && !m.alt && !m.shift
                            };
                            resp = Some(SearchItemSelection { id: res.id, close: !keep_open });
                            self.field_needs_focus = true;
                            ui.ctx().request_repaint();
                        }
                    }
                });
        }

        if *self.is_searching.read().unwrap() {
            ui.allocate_ui_at_rect(out.response.rect, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.spinner();
                })
            });
        }

        resp
    }
}

pub struct SearchItemSelection {
    pub id: lb::Uuid,
    pub close: bool,
}
