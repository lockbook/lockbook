use std::{
    sync::{mpsc, Arc, RwLock},
    thread,
};

use eframe::egui;
use lb::service::search_service::SearchResult::*;
use lb::service::search_service::{SearchRequest, SearchResult};

use crate::{model::DocType, theme::Icon};

pub struct FullDocSearch {
    requests: mpsc::Sender<String>,
    responses: mpsc::Receiver<Vec<SearchResult>>,
    pub query: String,
    pub results: Vec<SearchResult>,
    err_msg: String,
    pub is_searching: Arc<RwLock<bool>>,
}

impl FullDocSearch {
    pub fn new(core: &lb::Core, ctx: &egui::Context) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<String>();
        let (response_tx, response_rx) = mpsc::channel();

        let is_searching = Arc::new(RwLock::new(false));

        thread::spawn({
            let is_searching = is_searching.clone();
            let core = core.clone();
            let ctx = ctx.clone();

            move || {
                while let Ok(input) = request_rx.recv() {
                    *is_searching.write().unwrap() = true;
                    ctx.request_repaint();

                    let start_search = core.start_search().unwrap();

                    start_search
                        .search_tx
                        .send(SearchRequest::Search { input: input.to_string() })
                        .unwrap();

                    let res = vec![
                        start_search.results_rx.recv().unwrap(),
                        start_search.results_rx.recv().unwrap(),
                        start_search.results_rx.recv().unwrap(),
                        start_search.results_rx.recv().unwrap(),
                    ];

                    response_tx.send(res).unwrap();

                    *is_searching.write().unwrap() = false;
                    ctx.request_repaint();
                }
            }
        });

        Self {
            requests: request_tx,
            responses: response_rx,
            query: String::new(),
            err_msg: String::new(),
            results: Vec::new(),
            is_searching,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, core: &lb::Core) -> Option<lb::Uuid> {
        while let Ok(res) = self.responses.try_recv() {
            println!("got results");
            self.results = res;
        }

        ui.vertical_centered(|ui| {
            let v_margin = 15.0;

            let output = egui::TextEdit::singleline(&mut self.query)
                .desired_width(ui.available_size_before_wrap().x - 5.0)
                .hint_text("Search")
                .margin(egui::vec2(15.0, 9.0))
                .show(ui);

            let search_icon_width = 15.0; // approximation
            let is_text_clipped = output.galley.rect.width() + v_margin * 2.0 + search_icon_width
                > output.response.rect.width();

            if !is_text_clipped {
                ui.allocate_ui_at_rect(output.response.rect, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        Icon::SEARCH.color(egui::Color32::GRAY).show(ui);
                    })
                });
            }

            if output.response.changed() && !self.query.is_empty() {
                println!("request search ");
                self.requests.send(self.query.clone()).unwrap();
            }

            if self.query.is_empty() {
                self.results = vec![];
            }
            if !self.results.is_empty() {
                return self.show_results(ui, core);
            };
            None
        })
        .inner
    }

    pub fn show_results(&mut self, ui: &mut egui::Ui, core: &lb::Core) -> Option<lb::Uuid> {
        ui.add_space(20.0);

        // sort by descending score magnitude
        self.results.sort_by(|a, b| {
            let score_a = match a {
                FileNameMatch { id: _, path: _, matched_indices: _, score } => Some(*score),
                FileContentMatches { id: _, path: _, content_matches } => {
                    Some(content_matches[0].score)
                }
                _ => None,
            };
            let score_b = match b {
                FileNameMatch { id: _, path: _, matched_indices: _, score } => Some(*score),
                FileContentMatches { id: _, path: _, content_matches } => {
                    Some(content_matches[0].score)
                }
                _ => None,
            };
            score_b
                .unwrap_or_default()
                .cmp(&score_a.unwrap_or_default())
        });

        for (_, sr) in self.results.iter().enumerate() {
            let result_response = ui.vertical(|ui| {
                match sr {
                    Error(err) => self.err_msg = err.msg.clone(),
                    FileNameMatch { id, path, matched_indices: _, score: _ } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                    }

                    FileContentMatches { id, path, content_matches } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                        ui.horizontal(|ui| {
                            ui.add_space(15.0);
                            ui.horizontal_wrapped(|ui| {
                                let content_match = &content_matches[0]; // rarely, there exits more than  1 content match

                                let pre =
                                    &content_match.paragraph[0..content_match.matched_indices[0]];
                                let highlighted = &content_match.paragraph[content_match
                                    .matched_indices[0]
                                    ..*content_match.matched_indices.last().unwrap() + 1];
                                let post = &content_match.paragraph
                                    [*content_match.matched_indices.last().unwrap() + 1..];

                                ui.label(egui::RichText::new(pre).size(15.0));
                                ui.label(
                                    egui::RichText::new(highlighted)
                                        .size(15.0)
                                        .background_color(
                                            ui.visuals().widgets.active.bg_fill.gamma_multiply(0.5),
                                        ),
                                );
                                ui.label(egui::RichText::new(post).size(15.0));
                            });
                        });
                    }

                    NoMatch => todo!(),
                };
                ui.add_space(10.0);
            });

            let a =
                ui.interact(result_response.response.rect, ui.next_auto_id(), egui::Sense::click());
            if a.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand)
            }

            if a.clicked() {
                let id = match sr {
                    FileNameMatch { id, .. } => Some(id),
                    FileContentMatches { id, .. } => Some(id),
                    _ => None,
                };
                return Some(*id.unwrap());
            };

            ui.separator();
            ui.add_space(10.0);
        }
        None
    }

    fn show_file(ui: &mut egui::Ui, file: &lb::File, path: &str) {
        let sidebar_vertical_margin = 15.0;
        ui.horizontal_wrapped(|ui| {
            ui.add_space(sidebar_vertical_margin);

            DocType::from_name(file.name.as_str()).to_icon().show(ui);

            ui.add_space(7.0);

            ui.label(egui::RichText::new(&file.name));
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(sidebar_vertical_margin);

            let mut job = egui::text::LayoutJob::single_section(
                path.to_owned(),
                egui::TextFormat::simple(egui::FontId::proportional(15.0), egui::Color32::GRAY),
            );

            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
    }
}
