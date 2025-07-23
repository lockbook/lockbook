use std::ops::{Deref as _, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use egui::TextWrapMode;
use lb::Uuid;
use lb::blocking::Lb;
use lb::subscribers::search::{SearchConfig, SearchResult};

use workspace_rs::show::DocType;

pub struct SearchModal {
    core: Lb,
    is_searching: Arc<AtomicBool>,
    input: Arc<Mutex<String>>,
    results: Arc<Mutex<Vec<SearchResult>>>,
    field_needs_focus: bool,
    errors: Vec<String>,
    arrow_index: Option<usize>,
}

impl SearchModal {
    pub fn new(core: Lb) -> Self {
        Self {
            core,
            is_searching: Default::default(),
            input: Default::default(),
            results: Default::default(),
            field_needs_focus: true,
            errors: Vec::new(),
            arrow_index: None,
        }
    }

    pub fn focus_select_all(&mut self) {
        self.field_needs_focus = true;
    }

    fn process_keys(&mut self, etx: &egui::Context) {
        let results_len = self.results.lock().unwrap().len();

        if etx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
            self.arrow_index = match self.arrow_index {
                Some(n) => {
                    if n == results_len - 1 {
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
                None => Some(results_len - 1),
            };
            self.ensure_arrowed_path();
        }
    }

    fn ensure_arrowed_path(&mut self) {
        if let Some(i) = self.arrow_index {
            *self.input.lock().unwrap() = self
                .results
                .lock()
                .unwrap()
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
            .color(ui.visuals().text_color().linear_multiply(0.5)))
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
            .color(ui.visuals().text_color().linear_multiply(0.7))
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
                            .linear_multiply(0.4),
                    )
                } else if resp.hovered() {
                    Some(
                        ui.style()
                            .visuals
                            .widgets
                            .active
                            .bg_fill
                            .linear_multiply(0.1),
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

        if self.arrow_index.is_some() {
            self.ensure_arrowed_path();
        }

        ui.set_width(600.0);

        let mut input = self.input.lock().unwrap();
        let results = self.results.lock().unwrap();

        let out = ui
            .vertical_centered(|ui| {
                egui::TextEdit::singleline(input.deref_mut())
                    .desired_width(f32::INFINITY)
                    .margin(egui::vec2(6.0, 6.0))
                    .show(ui)
            })
            .inner;

        if out.response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !results.is_empty()
        {
            let item = &results[self.arrow_index.unwrap_or(0)];
            resp = Some(SearchItemSelection { id: item.id(), close: true });
        } else {
            self.field_needs_focus = true;
        }

        if self.field_needs_focus {
            out.response.request_focus();
            ui.ctx().request_repaint();
            self.field_needs_focus = false;
        }

        // launch search if query changed
        if self.arrow_index.is_none() && out.response.changed() {
            let core = self.core.clone();
            let is_searching = self.is_searching.clone();
            let query = self.input.clone();
            let results = self.results.clone();
            let ctx = ui.ctx().clone();
            thread::spawn(move || {
                // get the query
                let this_query = query.lock().unwrap().clone();

                // run the search (no locks held)
                is_searching.store(true, Ordering::Relaxed);
                let these_results = core
                    .search(&this_query, SearchConfig::Paths)
                    .unwrap_or_default();

                // update the results only if they are for the current query
                let query = query.lock().unwrap();
                let mut results = results.lock().unwrap();

                if query.deref() == &this_query {
                    is_searching.store(false, Ordering::Relaxed);
                    *results = these_results;
                    ctx.request_repaint();
                }
            });

            self.arrow_index = None;
        }

        if !self.errors.is_empty() {
            ui.add_space(5.0);

            for err in &self.errors {
                ui.label(err); // todo appear as error message, maybe icon plus red text
            }
        } else if !results.is_empty() {
            ui.add_space(5.0);

            // clippy thinks the ui closure is an 'if' statement because it returns a bool
            #[allow(clippy::blocks_in_conditions)]
            if egui::ScrollArea::vertical()
                .max_height(500.0)
                .show(ui, |ui| {
                    let mut set_field_needs_focus = false;
                    for (index, res) in results.iter().enumerate() {
                        if self.show_search_result(ui, res, index).clicked() {
                            let keep_open = {
                                let m = ui.input(|i| i.modifiers);
                                m.command && !m.alt && !m.shift
                            };
                            resp = Some(SearchItemSelection { id: res.id(), close: !keep_open });
                            set_field_needs_focus = true;
                            ui.ctx().request_repaint();
                        }
                    }
                    set_field_needs_focus
                })
                .inner
            {
                self.field_needs_focus = true;
            }
        }

        resp
    }
}

pub struct SearchItemSelection {
    pub id: Uuid,
    pub close: bool,
}
