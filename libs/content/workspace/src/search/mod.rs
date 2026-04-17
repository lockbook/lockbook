pub mod content;
pub mod path;

pub struct Search {
    pub search_shown: bool,
    search_type: SearchType,
    query: String,
    pub initialized: bool,
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

#[derive(Default)]
pub struct PickerResponse {
    pub activated: Option<lb_rs::Uuid>,
    pub selected: Option<lb_rs::Uuid>,
}

pub trait SearchExecutor {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    /// Render the result list. `activated` is set when the user opens a result
    /// (e.g. Enter or row shortcut); `selected` tracks the highlighted row for
    /// the preview pane.
    fn show_result_picker(&mut self, ui: &mut Ui) -> PickerResponse;
    fn show_preview(&mut self, ui: &mut Ui, tab: Option<&mut Tab>);
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
                self.open_file(id, true, true);
                self.search.search_shown = false;
            }
        }
    }

    pub fn show_search(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid> {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(6.0);
            self.search_type_selector(ui);
            ui.add_space(6.0);
            Self::hairline(ui, true);

            ui.add_space(6.0);
            self.search_bar(ui);
            ui.add_space(6.0);
            Self::hairline(ui, true);

            ui.add_space(6.0);
            self.results_and_preview(ui)
        })
        .inner
    }

    /// A 1px separator that matches the modal's subtle divider treatment.
    fn hairline(ui: &mut Ui, horizontal: bool) {
        let color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let stroke = egui::Stroke { width: 1.0, color };
        if horizontal {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().hline(rect.x_range(), rect.center().y, stroke);
        } else {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(1.0, ui.available_height()), egui::Sense::hover());
            ui.painter().vline(rect.center().x, rect.y_range(), stroke);
        }
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
                .fill(if selected { theme.neutral_bg() } else { theme.neutral_bg_secondary() })
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

            ui.add_space(14.0);

            // Magnifying glass leading the text edit, vertically centered with the input.
            ui.allocate_ui_with_layout(
                egui::vec2(18.0, 28.0),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    Icon::SEARCH
                        .size(16.0)
                        .color(theme.neutral_fg_secondary())
                        .show(ui);
                },
            );

            // No background color — the text edit sits directly on the modal
            // surface, so tabs/search/results read as one continuous area.
            let resp = TextEdit::singleline(&mut self.search.query)
                .text_color(theme.neutral_fg())
                .frame(false)
                .hint_text("Search")
                .desired_width(ui.available_size_before_wrap().x - 10.0)
                .margin(Margin { left: 2, top: 5, bottom: 5, ..Margin::ZERO })
                .show(ui)
                .response;

            if !self.search.initialized || ui.ctx().memory(|m| m.focused().is_none()) {
                self.search.initialized = true;
                resp.request_focus();
            }
        });
    }

    fn results_and_preview(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid> {
        let size = ui.available_size();
        ui.horizontal(|ui| {
            ui.set_min_size(size);
            ui.add_space(10.0);
            let half = (ui.available_width() - 31.0) / 2.;
            let picker = ui
                .allocate_ui_with_layout(
                    Vec2::new(half, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| self.search.executor.show_result_picker(ui),
                )
                .inner;

            Self::hairline(ui, false);

            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width() - 10.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    // without clip_rect, toolbar glyphs bleed outside the preview pane
                    ui.set_clip_rect(ui.max_rect());
                    // without push_id, interactive widgets (e.g. checkboxes) in the preview
                    // collide with identical widgets in a background tab (if same file)
                    ui.push_id("search_preview", |ui| {
                        self.search.executor.show_preview(ui, None);
                    });
                },
            );

            picker.activated
        })
        .inner
    }

    fn manage_executors(&mut self) {
        let executor_search_type = self.search.executor.search_type();
        if executor_search_type != self.search.search_type {
            self.search.executor = self
                .search
                .search_type
                .create_executor(&self.core, &self.ctx);
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
            initialized: false,
            executor: SearchType::Path.create_executor(lb, ctx),
        }
    }

    fn process_keys(&mut self, ctx: &Context) {
        ctx.input_mut(|w| {
            if w.consume_key_exact(Modifiers::NONE, Key::Escape) {
                self.search_shown = false;
            }

            // Cmd+O: open (or toggle) the path search.
            if w.consume_key_exact(Modifiers::COMMAND, Key::O) {
                if self.search_shown && self.search_type == SearchType::Path {
                    self.search_shown = false;
                } else {
                    self.search_shown = true;
                    self.search_type = SearchType::Path;
                    self.initialized = false;
                }
            }

            // Cmd+Shift+F: open the content search, or if already open, switch between
            // Path and Content rather than dismissing.
            if w.consume_key_exact(Modifiers::COMMAND | Modifiers::SHIFT, Key::F) {
                if !self.search_shown {
                    self.search_shown = true;
                    self.search_type = SearchType::Content;
                    self.initialized = false;
                } else {
                    self.search_type = match self.search_type {
                        SearchType::Path => SearchType::Content,
                        SearchType::Content => SearchType::Path,
                    };
                    self.initialized = false;
                }
            }

            // Tab / Shift+Tab: cycle through search types while the modal is open.
            if self.search_shown {
                let next = w.consume_key_exact(Modifiers::NONE, Key::Tab);
                let prev = w.consume_key_exact(Modifiers::SHIFT, Key::Tab);
                if next || prev {
                    self.search_type = match (self.search_type, prev) {
                        (SearchType::Path, false) => SearchType::Content,
                        (SearchType::Content, false) => SearchType::Path,
                        (SearchType::Path, true) => SearchType::Content,
                        (SearchType::Content, true) => SearchType::Path,
                    };
                    self.initialized = false;
                }
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

use egui::{
    Button, Context, CornerRadius, Frame, Key, Margin, Modifiers, RichText, TextEdit, Ui, Vec2,
    Widget,
};
use lb_rs::blocking::Lb;

use crate::{
    file_cache::FilesExt as _,
    search::{content::ContentSearch, path::PathSearch},
    show::InputStateExt,
    tab::{ContentState, Tab},
    theme::{icons::Icon, palette_v2::ThemeExt},
    workspace::Workspace,
};
