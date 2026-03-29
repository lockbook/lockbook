use std::{
    cmp::min,
    f32::INFINITY,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicU64},
    },
};

use egui::{
    Button, Color32, Context, CornerRadius, Frame, Key, Label, Layout, Margin, Modifiers, Pos2,
    RichText, Rounding, ScrollArea, Sense, Spacing, Stroke, TextEdit, Ui, UiBuilder, Vec2, Widget,
};
use lb_rs::Uuid;
use nucleo::{
    Nucleo,
    pattern::{CaseMatching, Normalization},
};
use time::Instant;

use crate::{
    show::InputStateExt,
    theme::{icons::Icon, palette_v2::ThemeExt},
    widgets::GlyphonTextEdit,
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

impl Search {
    pub fn new(ctx: &Context) -> Self {
        Self::default()
    }
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
                        //.stroke(Stroke::new(1., theme.neutral_bg()))
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::ZERO),
                )
                .title_bar(false)
                .collapsible(false)
                .show(&self.ctx.clone(), |ui| self.show_search(ui));
        }
    }

    pub fn show_search(&mut self, ui: &mut Ui) {
        ui.set_min_size(ui.available_size());
        let theme = self.ctx.get_lb_theme();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            // Tab strip
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

            // Search bar
            Frame::new()
                .fill(theme.neutral_bg())
                .outer_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
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

                        let size = ui.available_size();
                        ui.horizontal(|ui| {
                            ui.set_min_size(size);
                            ScrollArea::both()
                                .max_width(ui.available_width() / 2.)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        if let Some(state) = &self.search.background_state {
                                            let snapshot = state.engine.snapshot();
                                            for item in snapshot.matched_items(
                                                0..min(
                                                    snapshot.matched_item_count(),
                                                    snapshot.matched_item_count(),
                                                ),
                                            ) {
                                                ui.label(item.data);
                                            }
                                        }
                                    });
                                });

                            Frame::new()
                                .fill(theme.neutral_bg_secondary())
                                .outer_margin(Margin::symmetric(5, 5))
                                .show(ui, |ui| {
                                    ui.set_min_size(ui.available_size());
                                });
                        });
                    });
                })
        });
    }

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
