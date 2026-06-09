pub mod content;
pub mod path;

pub struct Search {
    pub search_type: SearchType,
    pub query: String,
    pub initialized: bool,
    pub executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
    dispatched_query: String,
    building: Arc<AtomicBool>,

    core: Lb,
    files: Arc<RwLock<FileCache>>,
}

#[derive(Default, Eq, PartialEq, Clone, Copy)]
pub enum SearchType {
    #[default]
    Path,
    Content,
}

impl SearchType {
    fn create_executor(&self, lb: &Lb, files: &Arc<RwLock<FileCache>>) -> Box<dyn SearchExecutor> {
        std::thread::sleep(Duration::from_millis(1000));
        match self {
            SearchType::Path => Box::new(PathSearch::new(lb, files.clone())),
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
    pub selected: Option<lb_rs::Uuid>,
    /// Byte range of the highlighted snippet within the selected file's
    /// content (content search only). Drives preview scroll/highlight.
    pub selected_range: Option<std::ops::Range<usize>>,
}

pub trait SearchExecutor: Send + Sync {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    fn set_kb_mode(&mut self, kb_mode: bool);
    /// Render the result list. `activated` is set when the user opens a result
    /// (e.g. Enter or row shortcut); `selected` tracks the highlighted row for
    /// the preview pane.
    fn show_result_picker(&mut self, ui: &mut Ui) -> PickerResponse;
}

impl Search {
    pub fn new(lb: &Lb, ctx: &Context, files: Arc<RwLock<FileCache>>) -> Search {
        let mut search = Search {
            search_type: SearchType::Path,
            query: String::new(),
            initialized: false,
            executor: Arc::new(RwLock::new(None)),
            dispatched_query: String::new(),
            building: Arc::new(AtomicBool::new(false)),
            core: lb.clone(),
            files,
        };
        search.spawn_build(ctx);
        search
    }

    fn spawn_build(&mut self, ctx: &Context) {
        self.building.store(true, Ordering::SeqCst);
        self.dispatched_query.clear();

        let executor = self.executor.clone();
        let building = self.building.clone();
        let core = self.core.clone();
        let files = self.files.clone();
        let ctx = ctx.clone();
        let search_type = self.search_type;
        thread::spawn(move || {
            let mut guard = executor.write().unwrap();
            *guard = Some(search_type.create_executor(&core, &files));
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
            });
        });

        if focused {
            ui.painter().rect_stroke(
                out.response.rect,
                CornerRadius::same(12),
                egui::Stroke::new(2.0, accent),
                egui::epaint::StrokeKind::Inside,
            );
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
                let building = search.building.load(Ordering::SeqCst);
                (search.executor.clone(), search.search_type, building)
            };
            let (executor, search_type, creating) = extracted;

            ui.add_space(10.0);
            Self::hairline(ui, true);
            ui.add_space(6.0);

            if let Some(id) = self.results_and_preview(ui, &executor, search_type, creating) {
                self.open_file_replacing_search(id);
            }
        });
    }

    fn results_and_preview(
        &mut self, ui: &mut Ui, executor: &Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
        search_type: SearchType, creating: bool,
    ) -> Option<lb_rs::Uuid> {
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
                        let picker = if creating {
                            None
                        } else {
                            executor.try_write().ok().and_then(|mut guard| {
                                guard.as_mut().map(|e| e.show_result_picker(ui))
                            })
                        };
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

            picker.activated
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
use std::time::Duration;

use egui::{Context, CornerRadius, Frame, Margin, RichText, TextEdit, Ui, Vec2};
use lb_rs::blocking::Lb;

use crate::{
    file_cache::FileCache,
    search::{content::ContentSearch, path::PathSearch},
    tab::{ContentState, Destination, TabContent},
    theme::{icons::Icon, palette_v2::ThemeExt},
    workspace::Workspace,
};
