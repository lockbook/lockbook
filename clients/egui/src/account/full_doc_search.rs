use std::{
    sync::{
        mpsc::{self, TryRecvError},
        Arc, RwLock,
    },
    thread,
};

use eframe::egui;
use lb::service::search_service::SearchResult::*;
use lb::service::search_service::{SearchRequest, SearchResult};

use crate::theme::Icon;

pub struct FullDocSearch {
    requests: mpsc::Sender<String>,
    responses: mpsc::Receiver<Vec<SearchResult>>,
    pub query: String,
    results: Vec<SearchResult>,
    err_msg: String,
    pub is_searching: bool,
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
            is_searching: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, core: &lb::Core) {
        while let Ok(res) = self.responses.try_recv() {
            self.results = res;
        }

        ui.vertical_centered(|ui| {
            let v_margin = 15.0;

            let output = egui::TextEdit::singleline(&mut self.query)
                .desired_width(ui.available_size_before_wrap().x - 5.0)
                .hint_text("Search")
                .margin(egui::vec2(15.0, 9.0))
                .show(ui);

            if output.response.changed() {
                self.requests.send(self.query.clone()).unwrap();
            }

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
        });
    }
}
