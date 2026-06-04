pub mod content;
pub mod path;

/// The search experience, owned by its tab ([`crate::tab::TabContent::Search`])
/// so it renders through `Tab::show` like any other tab. Holds its own handles
/// to the core and file cache for query dispatch and the scope selector.
pub struct Search {
    pub search_type: SearchType,
    /// The directory the search is scoped to. Defaults to the account root.
    pub scope: lb_rs::Uuid,
    pub query: String,
    pub initialized: bool,
    pub executor: Arc<RwLock<Box<dyn SearchExecutor>>>,
    dispatched_query: String,

    core: Lb,
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

    pub fn name(&self) -> &'static str {
        match self {
            SearchType::Path => "Path",
            SearchType::Content => "Content",
        }
    }
}

pub trait SearchExecutor: Send + Sync {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    fn results(&self) -> &[SearchResult];
}

impl Search {
    pub fn new(lb: &Lb, ctx: &Context) -> Search {
        Search {
            search_type: SearchType::Path,
            scope: lb.get_root().map(|f| f.id).unwrap_or_default(),
            query: String::new(),
            initialized: false,
            executor: Arc::new(RwLock::new(SearchType::Path.create_executor(lb, ctx))),
            dispatched_query: String::new(),
            core: lb.clone(),
        }
    }

    /// Render the search tab. Returns the file to open when a result is
    /// activated (none yet — the results list is still being built).
    pub fn show(&mut self, ui: &mut Ui) -> Option<lb_rs::Uuid> {
        self.manage_executors(ui.ctx());
        ui.vertical(|ui| {
            ui.add_space(72.0);
            self.show_query_box(ui);
        });
        None
    }

    /// Swap the executor when the search type changes and dispatch the current
    /// query on a background thread. Safe to call every frame.
    fn manage_executors(&mut self, ctx: &Context) {
        let Ok(guard) = self.executor.try_read() else {
            return;
        };
        let stale_type = guard.search_type() != self.search_type;
        drop(guard);

        if stale_type {
            let executor = self.search_type.create_executor(&self.core, ctx);
            self.executor = Arc::new(RwLock::new(executor));
            self.dispatched_query.clear();
        }

        if self.query != self.dispatched_query {
            self.dispatched_query = self.query.clone();

            let executor = self.executor.clone();
            let ctx = ctx.clone();
            let query = self.query.clone();
            thread::spawn(move || {
                executor.write().unwrap().handle_query(&query);
                ctx.request_repaint();
            });
        }
    }

    /// The big "Open Quickly"-style query field: a centered, rounded, subtly
    /// filled box with a leading magnifying glass, large text, and an accent
    /// focus ring.
    fn show_query_box(&mut self, ui: &mut Ui) {
        let max_w = 720.0_f32.min(ui.available_width() - 48.0);
        let side = ((ui.available_width() - max_w) / 2.0).max(0.0);

        ui.horizontal(|ui| {
            ui.add_space(side);
            ui.allocate_ui_with_layout(
                Vec2::new(max_w, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.show_query_box_inner(ui),
            );
        });
    }

    fn show_query_box_inner(&mut self, ui: &mut Ui) {
        let theme = ui.ctx().get_lb_theme();
        let accent = theme.fg().get_color(theme.prefs().primary);

        let hint = match self.search_type {
            SearchType::Path => "Search Filenames",
            SearchType::Content => "Search Contents",
        };

        let frame = Frame::new()
            .fill(theme.neutral_bg_secondary())
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(20, 16));

        let out = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;

                // Leading icon doubles as the executor (search-type) dropdown.
                let icon_resp = Icon::SEARCH
                    .size(22.0)
                    .color(theme.neutral_fg_secondary())
                    .show(ui)
                    .on_hover_text("Choose what to search");
                egui::Popup::menu(&icon_resp).show(|ui| {
                    ui.set_min_width(160.0);
                    for ty in [SearchType::Path, SearchType::Content] {
                        if ui
                            .selectable_label(self.search_type == ty, ty.name())
                            .clicked()
                        {
                            self.search_type = ty;
                        }
                    }
                });

                let resp = TextEdit::singleline(&mut self.query)
                    .frame(false)
                    .hint_text(
                        RichText::new(hint)
                            .size(22.0)
                            .color(theme.neutral_fg_secondary()),
                    )
                    .text_color(theme.neutral_fg())
                    .font(egui::FontId::proportional(22.0))
                    .vertical_align(egui::Align::Center)
                    .desired_width(ui.available_width())
                    .margin(Margin::ZERO)
                    .show(ui)
                    .response;

                if !self.initialized || ui.ctx().memory(|m| m.focused().is_none()) {
                    self.initialized = true;
                    resp.request_focus();
                }

                resp.has_focus()
            })
            .inner
        });

        let focused = out.inner;
        let stroke = if focused {
            egui::Stroke::new(2.0, accent)
        } else {
            egui::Stroke::new(1.0, theme.neutral_fg_secondary().linear_multiply(0.25))
        };
        ui.painter().rect_stroke(
            out.response.rect,
            CornerRadius::same(12),
            stroke,
            egui::epaint::StrokeKind::Inside,
        );
    }
}

use std::sync::{Arc, RwLock};
use std::thread;

use egui::{Context, CornerRadius, Frame, Margin, RichText, TextEdit, Ui, Vec2};
use lb_rs::blocking::Lb;
use lb_rs::search::SearchResult;

use crate::{
    search::{content::ContentSearch, path::PathSearch},
    theme::{icons::Icon, palette_v2::ThemeExt},
};
