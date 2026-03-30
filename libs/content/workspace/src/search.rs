use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64},
    },
    time::Instant,
};

use egui::{
    Button, Context, CornerRadius, Frame, Key, Margin, Modifiers, RichText, ScrollArea, TextEdit,
    Ui, Vec2, Widget,
};
use lb_rs::Uuid;
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};

use crate::{show::InputStateExt, theme::palette_v2::ThemeExt, workspace::Workspace};

#[derive(Default)]
pub struct Search {
    search_shown: bool,
    search_type: SearchType,
    query: String,
    background_state: Option<SearchSession>,
}

pub struct SearchSession {
    search_type: SearchType,
    engine: Nucleo<String>,
    submitted_query: String,
    ingest_state: IngestState,
}

#[derive(Default)]
pub struct IngestState {
    workers_spawned: Option<Instant>,
    files_ingested: AtomicU32,
    ingest_target: AtomicU32,
    ignored_files: AtomicU32,
    elapsed_ms: AtomicU64,
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
                {
                    let snapshot = state.engine.snapshot();
                    let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

                    for item in snapshot.matched_items(0..snapshot.matched_item_count()) {
                        let text = item.data.as_str();

                        let mut indices = Vec::new();

                        // Assumes your query is stored in column 0 and item.data is a String
                        state.engine.pattern.column_pattern(0).indices(
                            item.matcher_columns[0].slice(..),
                            &mut matcher,
                            &mut indices,
                        );

                        ui.horizontal_wrapped(|ui| {
                            let mut chars = text.chars().collect::<Vec<_>>();

                            for (char_idx, ch) in chars.drain(..).enumerate() {
                                let rich = if indices.contains(&(char_idx as u32)) {
                                    egui::RichText::new(ch.to_string()).strong().underline()
                                } else {
                                    egui::RichText::new(ch.to_string())
                                };

                                ui.label(rich);
                            }
                        });
                    }
                }
            }
        });
    }

    fn result_preview(&mut self, ui: &mut Ui) {}

    fn manage_nucleo(&mut self) {
        let ctx = self.ctx.clone();
        let notify = Arc::new(move || {
            ctx.request_repaint();
        });

        if self.search.background_state.is_none() {
            match self.search.search_type {
                SearchType::Path => {
                    let now = Instant::now();
                    let paths = self.core.list_paths(None).unwrap();
                    let s = SearchSession {
                        search_type: SearchType::Path,
                        engine: Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1),
                        ingest_state: IngestState::default(),
                        submitted_query: String::new(),
                    };
                    for path in paths {
                        s.engine.injector().push(path, |s, cols| {
                            cols[0] = s.as_str().into();
                        });
                    }
                    self.search.background_state = Some(s);
                }
                SearchType::Content => todo!(),
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

struct Entry {
    id: Uuid,
    name: String,
    path: String,
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
