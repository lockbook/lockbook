use std::{
    sync::{atomic::AtomicBool, mpsc, Arc},
    thread,
};

use eframe::egui;
use lb::service::search_service::{SearchRequest, SearchResult, SearchType};
use lb::{
    service::search_service::{ContentMatch, SearchResult::*},
    StartSearchInfo,
};
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

use crate::model::DocType;

pub struct FullDocSearch {
    results_rx: mpsc::Receiver<SearchResult>,
    results_tx: mpsc::Sender<SearchResult>,
    is_searching: Arc<AtomicBool>,
    pub search_channel: Option<StartSearchInfo>,
    x_margin: f32,
    pub query: String,
    pub results: Vec<SearchResult>,
}

impl FullDocSearch {
    pub fn new() -> Self {
        let (results_tx, results_rx) = mpsc::channel();
        let is_searching = Arc::new(AtomicBool::new(false));

        Self {
            results_rx,
            results_tx,
            is_searching,
            search_channel: None,
            x_margin: 15.0,
            query: String::new(),
            results: Vec::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, core: &lb::Core) -> Option<&lb::Uuid> {
        while let Ok(res) = self.results_rx.try_recv() {
            match res {
                SearchResult::StartOfSearch => {
                    self.results.clear();
                    self.is_searching
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
                SearchResult::FileContentMatches { id, path, content_matches } => {
                    content_matches.into_iter().for_each(|cm| {
                        let expanded_res = SearchResult::FileContentMatches {
                            id,
                            path: path.clone(),
                            content_matches: vec![cm],
                        };
                        self.results.push(expanded_res);
                    })
                }
                SearchResult::EndOfSearch => {
                    self.is_searching
                        .store(false, std::sync::atomic::Ordering::Relaxed);
                }
                SearchResult::FileNameMatch { .. } | SearchResult::Error(_) => {
                    self.results.push(res);
                }
            }

            self.results.sort_by(|a, b| {
                b.get_score()
                    .unwrap_or_default()
                    .cmp(&a.get_score().unwrap_or_default())
            });
        }

        ui.vertical_centered(|ui| {
            let output = egui::TextEdit::singleline(&mut self.query)
                .desired_width(ui.available_size_before_wrap().x - 5.0)
                .hint_text("Search")
                .margin(egui::vec2(self.x_margin, 9.0))
                .show(ui);

            let search_icon_width = 15.0; // approximation
            let is_text_clipped =
                output.galley.rect.width() + self.x_margin * 2.0 + search_icon_width
                    > output.response.rect.width();

            if !is_text_clipped {
                ui.allocate_ui_at_rect(output.response.rect, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);

                        if self.query.is_empty() {
                            Icon::SEARCH.color(egui::Color32::GRAY).show(ui);
                        } else {
                            ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
                            if Button::default().icon(&Icon::CLOSE).show(ui).clicked() {
                                self.search_channel
                                    .as_ref()
                                    .unwrap()
                                    .search_tx
                                    .send(SearchRequest::EndSearch)
                                    .unwrap();
                                self.search_channel = None;

                                self.query = "".to_string();
                                self.results = vec![];
                            }
                        }
                    });
                });
            };

            if output.response.changed() && !self.query.is_empty() {
                self.results = vec![];
                self.send_search_results(core);
            }

            if self.is_searching.load(std::sync::atomic::Ordering::Relaxed) {
                ui.add_space(20.0);
                ui.spinner();
            }

            if self.query.is_empty() {
                self.results = vec![];
                if self.search_channel.is_some() {
                    self.search_channel
                        .as_ref()
                        .unwrap()
                        .search_tx
                        .send(SearchRequest::EndSearch)
                        .unwrap();
                    self.search_channel = None;
                }
            }

            if self.search_channel.is_some() {
                return egui::ScrollArea::vertical()
                    .show(ui, |ui| self.show_results(ui, core))
                    .inner;
            };

            None
        })
        .inner
    }

    fn send_search_results(&mut self, core: &lb::Core) {
        if self.search_channel.is_none() {
            self.search_channel = Some(core.start_search(SearchType::PathAndContentSearch));
        }

        let search_tx = self.search_channel.as_ref().unwrap().search_tx.clone();
        let results_tx = self.results_tx.clone();
        let results_rx = self.search_channel.as_ref().unwrap().results_rx.clone();
        let query = self.query.clone();
        let is_searching = self.is_searching.clone();
        is_searching.store(true, std::sync::atomic::Ordering::Relaxed);

        thread::spawn(move || {
            search_tx
                .send(SearchRequest::Search { input: query })
                .unwrap();

            while let Ok(sr) = results_rx.recv() {
                results_tx.send(sr).unwrap();
                match results_rx.try_recv() {
                    Err(e) if e.is_empty() => {
                        is_searching.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    _ => (),
                }
            }
        });
    }

    pub fn show_results(&mut self, ui: &mut egui::Ui, core: &lb::Core) -> Option<&lb::Uuid> {
        ui.add_space(20.0);

        if self.results.is_empty() && !self.is_searching.load(std::sync::atomic::Ordering::Relaxed)
        {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("No results").color(egui::Color32::GRAY));
            });
        } else {
            for sr in self.results.iter() {
                let sr_res = ui.vertical(|ui| {
                    match sr {
                        Error(err) => {
                            ui.horizontal(|ui| {
                                ui.add_space(self.x_margin);
                                ui.label(
                                    egui::RichText::new(err.msg.to_owned())
                                        .color(ui.visuals().extreme_bg_color),
                                );
                            });
                        }

                        FileNameMatch { id, path, matched_indices: _, score: _ } => {
                            let file = &core.get_file_by_id(*id).unwrap();
                            Self::show_file(ui, file, path, self.x_margin);
                        }

                        FileContentMatches { id, path, content_matches } => {
                            let file = &core.get_file_by_id(*id).unwrap();
                            Self::show_file(ui, file, path, self.x_margin);
                            ui.horizontal(|ui| {
                                ui.add_space(15.0);
                                ui.horizontal_wrapped(|ui| {
                                    let font_size = 15.0;
                                    self.show_content_match(ui, &content_matches[0], font_size);
                                });
                            });
                        }

                        _ => {}
                    };
                    ui.add_space(10.0);
                });
                ui.add(egui::Separator::default().shrink(ui.available_width() / 1.5));
                ui.add_space(10.0);

                let sr_res =
                    ui.interact(sr_res.response.rect, ui.next_auto_id(), egui::Sense::click());
                if sr_res.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand)
                }

                if sr_res.clicked() {
                    let id = match sr {
                        FileNameMatch { id, .. } => Some(id),
                        FileContentMatches { id, .. } => Some(id),
                        _ => None,
                    };
                    if id.is_some() {
                        return id;
                    }
                };
            }
        }

        None
    }

    fn show_file(ui: &mut egui::Ui, file: &lb::File, path: &str, x_margin: f32) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(x_margin);

            DocType::from_name(file.name.as_str()).to_icon().show(ui);

            ui.add_space(7.0);

            let mut job = egui::text::LayoutJob::single_section(
                file.name.clone(),
                egui::TextFormat::simple(
                    egui::FontId::proportional(18.0),
                    ui.visuals().text_color(),
                ),
            );
            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(x_margin);

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

    fn show_content_match(&self, ui: &mut egui::Ui, content_match: &ContentMatch, font_size: f32) {
        let matched_indices = &content_match.matched_indices;
        let str = content_match.paragraph.clone();
        let highlight_color = ui.visuals().widgets.active.bg_fill.gamma_multiply(0.5);

        let mut curr = 0;
        let mut next;

        let pre = str[0..matched_indices[0]].to_string();
        ui.label(egui::RichText::new(pre).size(font_size));

        while curr < matched_indices.len() {
            next = curr;

            while next < matched_indices.len() - 1
                && matched_indices[next] + 1 == matched_indices[next + 1]
            {
                next += 1;
            }

            if next == curr || curr == matched_indices.len() - 1 {
                let h_str = str
                    .chars()
                    .nth(matched_indices[curr])
                    .unwrap_or_default()
                    .to_string();
                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );

                curr += 1;
            } else {
                let h_str = str[matched_indices[curr]..matched_indices[next] + 1].to_string();

                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );
                curr = next + 1;
            }
            if curr < matched_indices.len() - 1 {
                ui.label(
                    egui::RichText::new(
                        str[matched_indices[next] + 1..matched_indices[curr]].to_string(),
                    )
                    .size(font_size),
                );
            }
        }

        let post = str[matched_indices[matched_indices.len() - 1] + 1..].to_string();
        ui.label(egui::RichText::new(post).size(font_size));
    }
}
