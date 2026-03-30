use std::{
    ops::Deref,
    sync::{Arc, RwLock},
    thread::{self, available_parallelism},
    time::Instant,
};

use egui::{
    Button, Context, CornerRadius, Frame, Key, Margin, Modifiers, RichText, ScrollArea, TextEdit,
    Ui, Vec2, Widget,
};
use lb_rs::model::file::File;
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};

use crate::{
    show::{DocType, InputStateExt},
    theme::palette_v2::ThemeExt,
    workspace::Workspace,
};

#[derive(Default)]
pub struct Search {
    search_shown: bool,
    search_type: SearchType,
    query: String,
    background_state: Option<SearchSession>,
}

pub struct SearchSession {
    search_type: SearchType,
    engine: Nucleo<Entry>,
    submitted_query: String,
    ingest_state: Arc<RwLock<IngestState>>,
}

#[derive(Default)]
pub struct IngestState {
    ingest_start: Option<Instant>,
    uningested_files: Vec<File>,
    files_ingested: u32,
    ingest_target: u32,
    ignored_files: u32,
    ingest_end: Option<Instant>,
}

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchType {
    #[default]
    Path,
    Content,
    // inspo:
    // All,
    // Tabs
    // Commands, this will need an evolution of the entry concept
    // maybe an enum, maybe a trait
    // Semantic
}

impl Workspace {
    pub fn show_search_modal(&mut self) {
        self.search.process_keys(&self.ctx);
        self.manage_nucleo();
        let size = self.ctx.screen_rect();
        let theme = self.ctx.get_lb_theme();

        if self.search.search_shown {
            egui::Window::new("")
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .min_width(size.width() * 0.8)
                .min_height(size.height() * 0.8)
                .resizable(false)
                .fade_in(true)
                .frame(
                    Frame::window(&self.ctx.style())
                        .fill(theme.neutral_bg_secondary())
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::ZERO),
                )
                .title_bar(false)
                .collapsible(false)
                .show(&self.ctx.clone(), |ui| {
                    ui.set_min_size(ui.available_size());
                    self.show_search(ui)
                });
        }
    }

    pub fn show_search(&mut self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            self.search_type_selector(ui);

            // Search bar
            Frame::new()
                .fill(theme.neutral_bg())
                .outer_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        self.search_bar(ui);
                        self.results_and_preview(ui);
                    });
                })
        });
    }

    fn search_type_selector(&mut self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            for button in [SearchType::Path, SearchType::Content] {
                let selected = self.search.search_type == button;

                let button_resp = Button::selectable(
                    selected,
                    RichText::new(button.name()).color(if selected {
                        theme.fg().get_color(theme.prefs().primary)
                    } else {
                        theme.neutral_fg()
                    }),
                )
                .corner_radius(CornerRadius::ZERO)
                .frame_when_inactive(true)
                .min_size(Vec2::new(85., 0.))
                .fill(if selected {
                    //theme.bg().get_color(theme.prefs().primary)
                    theme.neutral_bg()
                } else {
                    theme.neutral_bg_secondary()
                })
                .ui(ui);

                if button_resp.clicked() {
                    self.search.search_type = button;
                }
            }
            ui.allocate_space(Vec2::new(ui.available_width(), 0.));
        });
    }

    fn search_bar(&mut self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();
        ui.horizontal(|ui| {
            ui.visuals_mut().widgets.hovered.bg_stroke =
                egui::Stroke { width: 0.1, color: ui.visuals().weak_text_color() };
            ui.visuals_mut().selection.stroke =
                egui::Stroke { width: 0.3, color: ui.visuals().weak_text_color() };

            // todo stick search icon like we do in full doc search
            let resp = TextEdit::singleline(&mut self.search.query)
                .text_color(theme.neutral_fg())
                .frame(true)
                .background_color(theme.neutral_bg())
                .hint_text("Search")
                .desired_width(ui.available_size_before_wrap().x)
                .margin(Margin { left: 30, top: 5, bottom: 5, ..Margin::ZERO })
                .show(ui)
                .response;

            resp.request_focus();
        });
    }

    fn results_and_preview(&mut self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();
        let size = ui.available_size();
        ui.horizontal(|ui| {
            ui.set_min_size(size);
            ScrollArea::both()
                .max_width(ui.available_width() / 2.)
                .show(ui, |ui| {
                    self.result_picker(ui);
                });

            Frame::new()
                .fill(theme.neutral_bg_secondary())
                .outer_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.result_preview(ui);
                });
        });
    }

    fn result_picker(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(state) = &self.search.background_state {
                let snapshot = state.engine.snapshot();
                let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

                for item in snapshot.matched_items(0..snapshot.matched_item_count()) {
                    let mut entry = item.data.clone();

                    let mut indices = Vec::new();

                    state.engine.pattern.column_pattern(0).indices(
                        item.matcher_columns[0].slice(..),
                        &mut matcher,
                        &mut indices,
                    );

                    entry.matched_region = indices;

                    ui.spacing_mut().item_spacing.y = 5.0;
                    self.show_result_cell(ui, entry);
                }
            }
        });
    }

    fn show_result_cell(&self, ui: &mut Ui, entry: Entry) {
        ui.horizontal(|ui| {
            // todo: aesthetics, spacing, background color, vertical centering
            // todo: support folders, and generally a richer icon experience
            DocType::from_name(&entry.path).to_icon().show(ui);

            ui.vertical(|ui| {
                ui.label(&entry.file.name);
                ui.label(entry.parent_path());
            })
        });
    }

    fn result_preview(&mut self, ui: &mut Ui) {}

    fn manage_nucleo(&mut self) {
        let ctx = self.ctx.clone();
        let notify = Arc::new(move || {
            ctx.request_repaint();
        });

        let nucleo_search_type = self
            .search
            .background_state
            .as_ref()
            .map(|state| state.search_type);
        if nucleo_search_type != Some(self.search.search_type) {
            self.search.background_state = None;
        }

        if self.search.background_state.is_none() {
            let id_paths = Arc::new(self.core.list_paths_with_ids(None).unwrap());
            let metas = self.core.list_metadatas().unwrap();

            match self.search.search_type {
                SearchType::Path => {
                    let now = Instant::now();
                    let s = SearchSession {
                        search_type: SearchType::Path,
                        engine: Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1),
                        ingest_state: Arc::new(RwLock::new(IngestState::default())),
                        submitted_query: String::new(),
                    };
                    for (id, path) in id_paths.deref() {
                        if path == "/" {
                            continue;
                        };

                        s.engine.injector().push(
                            Entry {
                                file: metas.iter().find(|m| m.id == *id).unwrap().clone(),
                                path: path.clone(),
                                matched_region: vec![],
                            },
                            |e, cols| {
                                cols[0] = e.path.as_str().into();
                            },
                        );
                    }
                    self.search.background_state = Some(s);
                }
                SearchType::Content => {
                    let mut ingest_state = IngestState::default();
                    ingest_state.ingest_start = Some(Instant::now());

                    for meta in metas {
                        if !meta.is_document() {
                            continue;
                        }

                        if !meta.name.ends_with(".md") {
                            ingest_state.ignored_files += 1;
                            continue;
                        }

                        ingest_state.uningested_files.push(meta);
                    }

                    ingest_state.ingest_target = ingest_state.uningested_files.len() as u32;

                    let s = SearchSession {
                        search_type: SearchType::Content,
                        engine: Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1),
                        submitted_query: String::new(),
                        ingest_state: Arc::new(RwLock::new(ingest_state)),
                    };

                    let injector = s.engine.injector();

                    for _ in 0..available_parallelism().map(|p| p.get()).unwrap_or(4) {
                        let ingest = s.ingest_state.clone();
                        let lb = self.core.clone();
                        let injector = injector.clone();
                        let id_paths = id_paths.clone();
                        thread::spawn(move || {
                            let mut unlocked = ingest.write().unwrap();
                            let Some(meta) = unlocked.uningested_files.pop() else {
                                return;
                            };
                            drop(unlocked);

                            let id = meta.id;
                            let doc = lb
                                .read_document(id, false)
                                .ok()
                                .and_then(|bytes| String::from_utf8(bytes).ok());

                            let success = match doc {
                                Some(doc) => {
                                    injector.push(
                                        Entry {
                                            file: meta,
                                            path: id_paths
                                                .iter()
                                                .find(|(i, _)| *i == id)
                                                .map(|(_, path)| path)
                                                .unwrap()
                                                .clone(),
                                            matched_region: vec![],
                                        },
                                        |e, cols| {},
                                    );
                                    true
                                }
                                None => false,
                            };

                            let mut unlocked = ingest.write().unwrap();
                            if success {
                                unlocked.files_ingested += 1;
                            } else {
                                unlocked.ignored_files += 1;
                            }

                            if unlocked.uningested_files.is_empty() {
                                unlocked.ingest_end = Some(Instant::now());
                            }
                        });
                    }
                }
            }
        }

        if let Some(state) = &mut self.search.background_state {
            if state.submitted_query != self.search.query {
                state.engine.pattern.reparse(
                    0,
                    &self.search.query,
                    CaseMatching::Smart,
                    Normalization::Smart,
                    self.search.query.starts_with(&state.submitted_query),
                );
                state.submitted_query = self.search.query.clone();
            }

            // todo: understand this more deeply
            state.engine.tick(1);
        }
    }
}

#[derive(Clone)]
struct Entry {
    file: File,
    path: String,
    matched_region: Vec<u32>,
}

impl Entry {
    fn parent_path(&self) -> &str {
        if self.path.ends_with('/') {
            self.path
                .strip_suffix(&format!("{}/", self.file.name))
                .unwrap()
        } else {
            self.path.strip_suffix(&self.file.name).unwrap()
        }
    }
}

impl Search {
    fn process_keys(&mut self, ctx: &Context) {
        ctx.input_mut(|w| {
            if w.consume_key_exact(Modifiers::NONE, Key::Escape) {
                self.search_shown = false;
            }

            if w.consume_key_exact(Modifiers::COMMAND | Modifiers::SHIFT, Key::O) {
                // there's some more complexity we can add here
                self.search_shown = !self.search_shown;
                self.search_type = SearchType::Path;
            }

            if w.consume_key_exact(Modifiers::COMMAND | Modifiers::SHIFT, Key::F) {
                self.search_shown = !self.search_shown;
                self.search_type = SearchType::Content;
            }
        })
    }
}

impl SearchType {
    fn name(&self) -> &'static str {
        match &self {
            SearchType::Path => "Path",
            SearchType::Content => "Content",
        }
    }
}
