pub mod content;
pub mod path;

pub struct Search {
    pub search_type: SearchType,
    pub query: String,
    pub initialized: bool,
    pub executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
    pub filters_open: bool,
    pub scope_path: String,
    folders: Arc<RwLock<Vec<(lb_rs::Uuid, String)>>>,
    scope_selected: usize,
    scope_was_focused: bool,
    query_focused: bool,
    dispatched_query: String,
    dispatched_filter: String,
    building: Arc<AtomicBool>,

    core: Lb,
}

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchType {
    #[default]
    Path,
    Content,
}

impl SearchType {
    fn create_executor(&self, lb: &Lb) -> Box<dyn SearchExecutor> {
        match self {
            SearchType::Path => Box::new(PathSearch::new(lb)),
            SearchType::Content => Box::new(ContentSearch::new(lb)),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SearchType::Path => "Path",
            SearchType::Content => "Content",
        }
    }
}

#[derive(Default)]
pub struct PickerResponse {
    pub activated: Option<lb_rs::Uuid>,
    /// When set alongside `activated`, the result should open in a new background
    /// tab (command/ctrl-click or the row's context menu) rather than replacing
    /// the search tab.
    pub activated_in_new_tab: bool,
    pub selected: Option<lb_rs::Uuid>,
    /// Byte range of the highlighted snippet within the selected file's
    /// content (content search only). Drives preview scroll/highlight.
    pub selected_range: Option<std::ops::Range<usize>>,
}

pub trait SearchExecutor: Send + Sync {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    fn update_filter(&mut self, filter: Option<SearchFilter>);
    fn set_kb_mode(&mut self, kb_mode: bool);
    /// Render the result list. `activated` is set when the user opens a result
    /// (e.g. Enter or row shortcut); `selected` tracks the highlighted row for
    /// the preview pane.
    fn show_result_picker(&mut self, ui: &mut Ui, allow_kb_nav: bool) -> PickerResponse;
}

impl Search {
    pub fn new(lb: &Lb, ctx: &Context) -> Search {
        let mut search = Search {
            search_type: SearchType::Path,
            query: String::new(),
            initialized: false,
            executor: Arc::new(RwLock::new(None)),
            filters_open: false,
            scope_path: String::new(),
            folders: Arc::new(RwLock::new(Vec::new())),
            scope_selected: 0,
            scope_was_focused: false,
            query_focused: false,
            dispatched_query: String::new(),
            dispatched_filter: String::new(),
            building: Arc::new(AtomicBool::new(false)),
            core: lb.clone(),
        };
        search.spawn_build(ctx);
        search.spawn_load_folders(ctx);
        search
    }

    fn spawn_load_folders(&self, ctx: &Context) {
        let folders = self.folders.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let loaded = core
                .list_paths_with_ids(Some(Filter::FoldersOnly))
                .unwrap_or_default();
            *folders.write().unwrap() = loaded;
            ctx.request_repaint();
        });
    }

    fn spawn_build(&mut self, ctx: &Context) {
        self.building.store(true, Ordering::SeqCst);
        self.dispatched_query.clear();
        self.dispatched_filter.clear();

        let executor = self.executor.clone();
        let building = self.building.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();
        let search_type = self.search_type;
        thread::spawn(move || {
            let mut guard = executor.write().unwrap();
            *guard = Some(search_type.create_executor(&core));
            drop(guard);
            building.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    /// Swap the executor when the search type changes and dispatch the current
    /// query on a background thread. Safe to call every frame.
    fn manage_executors(&mut self, ctx: &Context) {
        if self.building.load(Ordering::SeqCst) {
            return;
        }

        let Ok(guard) = self.executor.try_read() else {
            return;
        };
        let stale_type = match guard.as_ref() {
            Some(executor) => executor.search_type() != self.search_type,
            None => true,
        };
        drop(guard);

        if stale_type {
            self.spawn_build(ctx);
            return;
        }

        if self.query != self.dispatched_query {
            self.dispatched_query = self.query.clone();

            let executor = self.executor.clone();
            let ctx = ctx.clone();
            let query = self.query.clone();
            thread::spawn(move || {
                if let Some(executor) = executor.write().unwrap().as_mut() {
                    executor.handle_query(&query);
                }
                ctx.request_repaint();
            });
        }

        if self.scope_path != self.dispatched_filter {
            self.dispatched_filter = self.scope_path.clone();

            let filter = if self.scope_path.is_empty() {
                None
            } else {
                Some(SearchFilter::Path(self.scope_path.clone()))
            };

            let executor = self.executor.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                if let Some(executor) = executor.write().unwrap().as_mut() {
                    executor.update_filter(filter);
                }
                ctx.request_repaint();
            });
        }
    }

    /// The big "Open Quickly"-style query field: a centered, rounded, subtly
    /// filled box with a leading magnifying glass, large text, executor radio
    /// buttons, and an accent focus ring.
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

        let text_id = ui.id().with("search_query_input");
        let focused = ui.memory(|m| m.has_focus(text_id));
        self.query_focused = focused;

        let fill =
            if focused { ui.visuals().extreme_bg_color } else { theme.neutral_bg_secondary() };

        let frame = Frame::new()
            .fill(fill)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(20, 16));

        let out = frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    Icon::SEARCH
                        .size(22.0)
                        .color(theme.neutral_fg_secondary())
                        .show(ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let filter = IconButton::new(Icon::FILTER.size(18.0))
                            .tooltip("Filters")
                            .colored(self.filters_open)
                            .show(ui);
                        if filter.clicked() {
                            self.filters_open = !self.filters_open;
                            if !self.filters_open {
                                self.scope_path.clear();
                            }
                        }

                        let resp = TextEdit::singleline(&mut self.query)
                            .id(text_id)
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
                    });
                });

                // Executor selector along the bottom of the box.
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let label = |text: &str| {
                        RichText::new(text)
                            .size(14.0)
                            .color(theme.neutral_fg_secondary())
                    };
                    ui.radio_value(&mut self.search_type, SearchType::Path, label("Filenames"));
                    ui.radio_value(&mut self.search_type, SearchType::Content, label("Contents"));
                });

                if self.filters_open {
                    ui.add_space(8.0);
                    self.show_filter_bar(ui);
                }
            });
        });

        if (!self.scope_path.is_empty() || !self.query.is_empty())
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            if !self.scope_path.is_empty() {
                self.scope_path.clear();
                self.filters_open = false;
            } else {
                self.query.clear();
            }
        }

        if focused {
            ui.painter().rect_stroke(
                out.response.rect,
                CornerRadius::same(12),
                egui::Stroke::new(2.0, accent),
                egui::epaint::StrokeKind::Inside,
            );
        }
    }

    fn show_filter_bar(&mut self, ui: &mut Ui) {
        let theme = ui.ctx().get_lb_theme();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.set_min_height(22.0);
            ui.label(
                RichText::new("Searching inside")
                    .size(14.0)
                    .color(theme.neutral_fg_secondary()),
            );
            let home = IconButton::new(Icon::HOME.size(16.0))
                .tooltip("Home")
                .colored(self.scope_path.is_empty())
                .show(ui);
            if home.clicked() {
                self.scope_path.clear();
            }

            let resp = TextEdit::singleline(&mut self.scope_path)
                .frame(false)
                .text_color(theme.neutral_fg())
                .font(egui::FontId::proportional(14.0))
                .vertical_align(egui::Align::Center)
                .desired_width(ui.available_width())
                .margin(Margin::ZERO)
                .show(ui)
                .response;

            if resp.changed() {
                self.scope_selected = 0;
            }
            self.show_folder_dropdown(&resp);
        });
    }

    fn show_folder_dropdown(&mut self, anchor: &egui::Response) {
        let focused = anchor.has_focus();
        let open = focused || self.scope_was_focused;
        self.scope_was_focused = focused;
        if !open {
            return;
        }

        let needle = self.scope_path.to_lowercase();
        let matches: Vec<String> = {
            let folders = self.folders.read().unwrap();
            let mut matches: Vec<String> = folders
                .iter()
                .filter(|(_, path)| needle.is_empty() || path.to_lowercase().contains(&needle))
                .map(|(_, path)| path.clone())
                .collect();
            matches.sort_by(|a, b| {
                let depth = |p: &str| p.matches('/').count();
                depth(a)
                    .cmp(&depth(b))
                    .then_with(|| a.len().cmp(&b.len()))
                    .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
            });
            matches.truncate(50);
            matches
        };
        if matches.is_empty() {
            return;
        }

        self.scope_selected = self.scope_selected.min(matches.len() - 1);
        anchor.ctx.input_mut(|i| {
            if i.consume_key_exact(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                self.scope_selected = (self.scope_selected + 1).min(matches.len() - 1);
            }
            if i.consume_key_exact(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                self.scope_selected = self.scope_selected.saturating_sub(1);
            }
        });

        let mut chosen = None;
        if anchor.ctx.input_mut(|i| i.consume_key_exact(egui::Modifiers::NONE, egui::Key::Enter)) {
            chosen = matches.get(self.scope_selected).cloned();
        }

        egui::Popup::from_response(anchor)
            .open(true)
            .width(anchor.rect.width())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for (idx, path) in matches.iter().enumerate() {
                            let label = RichText::new(path).size(13.0);
                            let row = ui.selectable_label(idx == self.scope_selected, label);
                            if row.clicked() {
                                chosen = Some(path.clone());
                            }
                            if idx == self.scope_selected {
                                row.scroll_to_me(None);
                            }
                        }
                    });
            });

        if let Some(path) = chosen {
            self.scope_path = path;
            self.scope_selected = 0;
            self.scope_was_focused = false;
            anchor.surrender_focus();
        }
    }
}

impl Workspace {
    /// Render the whole search tab: the query box, then a results list (left)
    /// and read-only preview (right) filling the rest of the area.
    ///
    /// This is driven from `show_current_tab_content` rather than `Tab::show`
    /// because the preview pane reuses `self.preview` and the workspace's async
    /// file loader, which a `Tab` can't reach on its own.
    pub(crate) fn show_search_tab(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // Query box + executor management need the tab's `Search`; the
            // executor handle is cloned out so the results pass can borrow the
            // workspace (for the preview) without holding the tab borrow.
            let extracted = {
                let Some(tab) = self.tabs.get_mut(&Destination::Search) else {
                    return;
                };
                let ContentState::Open(TabContent::Search(search)) = &mut tab.content else {
                    return;
                };
                search.manage_executors(ui.ctx());
                ui.add_space(16.0);
                search.show_query_box(ui);
                (search.executor.clone(), search.search_type, search.query_focused)
            };
            let (executor, search_type, query_focused) = extracted;

            ui.add_space(10.0);
            Self::hairline(ui, true);
            ui.add_space(6.0);

            if let Some((id, in_new_tab)) =
                self.results_and_preview(ui, &executor, search_type, query_focused)
            {
                if self.is_folder(id) {
                    let path = self.files.read().unwrap().path(id);
                    if let Some(tab) = self.tabs.get_mut(&Destination::Search) {
                        if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                            search.scope_path = path;
                            search.query.clear();
                            search.filters_open = true;
                        }
                    }
                } else if in_new_tab {
                    self.open_file(id, false, true);
                } else {
                    self.open_file_replacing_search(id);
                }
            }
        });
    }

    fn results_and_preview(
        &mut self, ui: &mut Ui, executor: &Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
        search_type: SearchType, allow_kb_nav: bool,
    ) -> Option<(lb_rs::Uuid, bool)> {
        const OUTER_PAD: f32 = 24.0;
        const MIN_PREVIEW_WIDTH: f32 = 720.0;

        let size = ui.available_size();
        let show_preview = size.x >= MIN_PREVIEW_WIDTH;
        ui.horizontal(|ui| {
            ui.set_min_size(size);
            ui.add_space(OUTER_PAD);
            let picker_width = if show_preview {
                (ui.available_width() - (21.0 + OUTER_PAD)) / 2.
            } else {
                ui.available_width() - OUTER_PAD
            };
            let (picker, picked) = ui
                .allocate_ui_with_layout(
                    Vec2::new(picker_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let picker = executor.try_write().ok().and_then(|mut guard| {
                            guard.as_mut().map(|e| e.show_result_picker(ui, allow_kb_nav))
                        });
                        match picker {
                            Some(picker) => (picker, true),
                            None => {
                                ui.centered_and_justified(|ui| ui.spinner());
                                (PickerResponse::default(), false)
                            }
                        }
                    },
                )
                .inner;

            if picked {
                if show_preview {
                    self.set_preview(picker.selected);

                    // For content search, steer the read-only preview to the
                    // highlighted snippet.
                    if search_type == SearchType::Content {
                        if let Some(md) = self.preview.as_mut().and_then(|t| t.markdown_mut()) {
                            md.preview_navigate(picker.selected_range.clone());
                        }
                    }
                } else {
                    self.preview = None;
                }
            }

            if show_preview {
                Self::hairline(ui, false);

                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width() - OUTER_PAD, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        // without clip_rect, toolbar glyphs bleed outside the preview pane
                        ui.set_clip_rect(ui.max_rect());
                        // without push_id, interactive widgets (e.g. checkboxes) in the preview
                        // collide with identical widgets in a background tab (if same file)
                        ui.push_id("search_preview", |ui| match &mut self.preview {
                            Some(tab) => {
                                tab.show(ui);
                            }
                            None => {
                                ui.centered_and_justified(|ui| ui.spinner());
                            }
                        });
                    },
                );
            }

            picker.activated.map(|id| (id, picker.activated_in_new_tab))
        })
        .inner
    }

    /// A 1px separator that matches the search panel's subtle divider treatment.
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
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use egui::{Context, CornerRadius, Frame, Margin, RichText, TextEdit, Ui, Vec2};
use lb_rs::blocking::Lb;
use lb_rs::model::path_ops::Filter;
use lb_rs::search::SearchFilter;

use crate::{
    file_cache::FilesExt,
    search::{content::ContentSearch, path::PathSearch},
    show::InputStateExt,
    tab::{ContentState, Destination, TabContent},
    theme::{icons::Icon, palette_v2::ThemeExt},
    widgets::IconButton,
    workspace::Workspace,
};
