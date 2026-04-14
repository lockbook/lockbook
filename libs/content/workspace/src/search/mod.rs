pub mod content;
pub mod path;

pub struct Search {
    /// Whether the search modal is shown
    search_shown: bool,

    /// Which type of search are we targetting
    search_type: SearchType,

    /// What is the current string in the UI
    query: String,

    /// A search strategy that can execute the inputs above
    executor: Box<dyn SearchExecutor>,
}

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchType {
    #[default]
    Path,
    Content,
}

impl SearchType {
    fn create_executor(&self, lb: &Lb, ctx: &Context) -> Box<dyn SearchExecutor> {
        match self {
            SearchType::Path => Box::new(PathSearch::new(lb, ctx)),
            SearchType::Content => Box::new(ContentSearch::new(lb, ctx)),
        }
    }
}

pub trait SearchExecutor {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    /// Render the result list. Return `Some(id)` when the user activated a
    /// result (e.g., via a row shortcut) — the caller should dismiss the modal
    /// and open that file.
    fn show_result_picker(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid>;
    fn show_preview(&mut self, ui: &mut Ui);
}

impl Workspace {
    pub fn show_search_modal(&mut self) {
        self.search.process_keys(&self.ctx);
        self.manage_executors();
        let size = self.ctx.screen_rect();
        let theme = self.ctx.get_lb_theme();

        if self.search.search_shown {
            let activated = egui::Window::new("")
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
                })
                .and_then(|r| r.inner)
                .flatten();

            if let Some(id) = activated {
                self.search.search_shown = false;
                self.open_file(id, true, true);
            }
        }
    }

    pub fn show_search(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid> {
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
                        self.results_and_preview(ui)
                    })
                    .inner
                })
                .inner
        })
        .inner
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

    fn results_and_preview(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid> {
        let theme = self.ctx.get_lb_theme();
        let size = ui.available_size();
        ui.horizontal(|ui| {
            ui.set_min_size(size);
            let half = ui.available_width() / 2.;
            let activated = ui
                .allocate_ui_with_layout(
                    Vec2::new(half, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| self.search.executor.show_result_picker(ui),
                )
                .inner;

            Frame::new()
                .fill(theme.neutral_bg_secondary())
                .outer_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    self.search.executor.show_preview(ui);
                });

            activated
        })
        .inner
    }
    

    fn manage_executors(&mut self) {
        let executor_search_type = self.search.executor.search_type();
        if executor_search_type != self.search.search_type {
            self.search.executor = self.search.search_type.create_executor(&self.core, &self.ctx);
        }

        self.search.executor.handle_query(&self.search.query);
    }
}

impl Search {
    pub fn new(lb: &Lb, ctx: &Context) -> Search {
        Search {
            search_shown: false,
            search_type: SearchType::Path,
            query: String::new(),
            // this may make results go stale, perhaps the executor should be created upon showing
            // search, maybe not always, sometimes it could be good to keep content search results
            // and that whole state around. maybe empty query is a good signal whether or not the
            // state is valuable?
            executor: SearchType::Path.create_executor(lb, ctx), 
        }
    }

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

use std::{
    ops::Deref,
    sync::{Arc, RwLock},
    thread::{self, available_parallelism},
    time::Instant,
};

use egui::{
    Button, Context, CornerRadius, Frame, Key, Margin, Modifiers, RichText, TextEdit,
    Ui, Vec2, Widget,
};
use lb_rs::{blocking::Lb, model::file::File};
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};

use crate::{
    search::{content::ContentSearch, path::PathSearch},
    show::{DocType, InputStateExt},
    theme::palette_v2::ThemeExt,
    workspace::Workspace,
};
