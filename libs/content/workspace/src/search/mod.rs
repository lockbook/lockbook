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
        thread::spawn(move || load_folders(folders, core, ctx));
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
        thread::spawn(move || build_index(executor, building, core, ctx, search_type));
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
            thread::spawn(move || run_query(executor, ctx, query));
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
            thread::spawn(move || update_filter(executor, ctx, filter));
        }
    }

    /// Prompt row: field + picker switch + scope. Same chrome for every picker.
    fn show_prompt(&mut self, ui: &mut Ui, t: &Theme) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let pad = Space::Sm.pts();
        let host = egui::Id::new("search_query");
        let hint = match self.search_type {
            SearchType::Path => "Find files",
            SearchType::Content => "Live grep",
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.add_space(pad);
            let mut kind = match self.search_type {
                SearchType::Path => 0,
                SearchType::Content => 1,
            };
            let seg_w = segmented_width(ui, t, &["Filenames", "Contents"]);
            let field_w =
                (ui.available_width() - seg_w - control_height() - Space::Xs.pts() * 2.0 - pad)
                    .max(1.0);
            Field::new(t, &mut self.query)
                .id(host)
                .hint(hint)
                .leading(phosphor::SEARCH)
                .clearable(true)
                .width(field_w)
                .show(ui);
            ui.add(Spacer::new(Space::Xs).fill_cross(control_height()));
            if segmented(ui, t, &["Filenames", "Contents"], &mut kind).changed() {
                self.search_type = if kind == 0 { SearchType::Path } else { SearchType::Content };
            }
            ui.add(Spacer::new(Space::Xs).fill_cross(control_height()));
            if icon_button(ui, t, phosphor::FUNNEL, self.filters_open, t.neutral_bg()).clicked() {
                self.filters_open = !self.filters_open;
                if !self.filters_open {
                    self.scope_path.clear();
                }
            }
            ui.add_space(pad);
        });

        let edit_id = host.with("edit");
        self.query_focused = ui.memory(|m| m.has_focus(edit_id));
        if !self.initialized || ui.ctx().memory(|m| m.focused().is_none()) {
            self.initialized = true;
            ui.memory_mut(|m| m.request_focus(edit_id));
        }

        let filters_shown = !self.scope_path.is_empty() || self.filters_open;
        if (filters_shown || !self.query.is_empty())
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            if filters_shown {
                self.scope_path.clear();
                self.filters_open = false;
            } else {
                self.query.clear();
            }
        }
    }

    fn show_filter_bar(&mut self, ui: &mut Ui, t: &Theme) {
        let pad = Space::Sm.pts();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.add_space(pad);
            ui.label(
                TypeRole::Body
                    .rich("Inside")
                    .color(t.neutral_fg_secondary()),
            );
            ui.add(Spacer::new(Space::Xs).fill_cross(control_height()));
            if icon_button(ui, t, phosphor::FOLDERS, self.scope_path.is_empty(), t.neutral_bg())
                .clicked()
            {
                self.scope_path.clear();
            }
            ui.add(Spacer::new(Space::Xs).fill_cross(control_height()));
            let field_w = (ui.available_width() - pad).max(1.0);
            let resp = Field::new(t, &mut self.scope_path)
                .id("search_scope")
                .hint("Folder path")
                .leading(phosphor::FOLDER)
                .width(field_w)
                .show(ui);
            ui.add_space(pad);

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
        if anchor
            .ctx
            .input_mut(|i| i.consume_key_exact(egui::Modifiers::NONE, egui::Key::Enter))
        {
            chosen = matches.get(self.scope_selected).cloned();
        }

        egui::Popup::from_response(anchor)
            .open(true)
            .width(400.)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(400.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                egui::ScrollArea::vertical()
                    .max_height(600.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let t = ui.ctx().get_lb_theme();
                        for (idx, path) in matches.iter().enumerate() {
                            let selected = idx == self.scope_selected;
                            let (rect, row) = ui.allocate_at_least(
                                egui::vec2(ui.available_width(), control_height()),
                                egui::Sense::click(),
                            );
                            if selected || row.hovered() {
                                let amt = if selected {
                                    crate::style::FG_PRESS
                                } else {
                                    crate::style::FG_HOVER
                                };
                                ui.painter().rect_filled(
                                    rect.shrink(1.0),
                                    crate::style::Radius::Control.corner(),
                                    t.wash_toward_neutral_fg(t.neutral_bg(), amt),
                                );
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + Space::Sm.pts(), rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                path,
                                TypeRole::Body.font_id(),
                                t.neutral_fg(),
                            );
                            if row.clicked() {
                                chosen = Some(path.clone());
                            }
                            if selected {
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

/// Prompt stack at the bottom of the picker (hairline + pads + field).
fn prompt_band_h(filters_open: bool) -> f32 {
    let mut h = STROKE_HAIRLINE + Space::Sm.pts() + control_height() + Space::Sm.pts();
    if filters_open {
        h += Space::Xs.pts() + control_height();
    }
    h
}

/// Results above, prompt below. Adjacent, covering `max`.
fn picker_bands(max: egui::Rect, filters_open: bool) -> (egui::Rect, egui::Rect) {
    let ph = prompt_band_h(filters_open).min(max.height().max(0.0));
    let split_y = max.bottom() - ph;
    let results = egui::Rect::from_min_max(max.min, egui::pos2(max.right(), split_y));
    let prompt = egui::Rect::from_min_max(egui::pos2(max.left(), split_y), max.max);
    (results, prompt)
}

impl Workspace {
    /// Full-screen picker: results | preview, prompt at the bottom (Telescope).
    ///
    /// Driven from `show_current_tab_content` rather than `Tab::show` so the
    /// preview can use the workspace async file loader.
    pub(crate) fn show_search_tab(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let max = ui.max_rect();

        let extracted = {
            let Some(tab) = self.current_tab_mut() else {
                return;
            };
            let ContentState::Open(TabContent::Search(search)) = &mut tab.content else {
                return;
            };
            search.manage_executors(ui.ctx());
            (search.executor.clone(), search.search_type, search.query_focused, search.filters_open)
        };
        let (executor, search_type, query_focused, filters_open) = extracted;
        let (results_rect, prompt_rect) = picker_bands(max, filters_open);

        let t = ui.ctx().get_lb_theme();
        crate::style::place_at(ui, prompt_rect, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(prompt_rect.width());
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            Self::hairline(ui, true);
            ui.add(Spacer::new(Space::Sm));
            if let Some(tab) = self.current_tab_mut() {
                if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                    if filters_open {
                        search.show_filter_bar(ui, &t);
                        ui.add(Spacer::new(Space::Xs));
                    }
                    search.show_prompt(ui, &t);
                }
            }
            ui.add(Spacer::new(Space::Sm));
        });

        let (activated, _) = crate::style::place_at(
            ui,
            results_rect,
            egui::Layout::top_down(egui::Align::Min),
            |ui| self.results_and_preview(ui, &executor, search_type, query_focused),
        );
        crate::style::claim(ui, max);

        let Some((id, in_new_tab)) = activated else {
            return;
        };
        if self.is_folder(id) {
            let path = self.files.read().unwrap().path(id);
            if let Some(tab) = self.current_tab_mut() {
                if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                    search.scope_path = path;
                    search.query.clear();
                    search.filters_open = true;
                }
            }
            self.out.selected_file = Some(id);
        } else if in_new_tab {
            self.open_file(id, false, true);
        } else {
            self.open_file_replacing_search(id);
        }
    }

    fn results_and_preview(
        &mut self, ui: &mut Ui, executor: &Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
        search_type: SearchType, allow_kb_nav: bool,
    ) -> Option<(lb_rs::Uuid, bool)> {
        const MIN_PREVIEW_WIDTH: f32 = 560.0;
        let pad = LIST_PAD.pts();
        let max = ui.max_rect();
        let show_preview = max.width() >= MIN_PREVIEW_WIDTH;

        // Split full-bleed so the preview and divider meet the prompt hairline.
        // Recents / Shared wrap the list body in LIST_PAD (Sm, all sides); Files
        // tree only insets L/R (sticky headers stay edge-to-edge). This is a list.
        let (list_band, preview_rect) = if show_preview {
            let split = max.left() + max.width() * 0.4;
            (
                egui::Rect::from_min_max(max.min, egui::pos2(split, max.bottom())),
                Some(egui::Rect::from_min_max(
                    egui::pos2(split + STROKE_HAIRLINE, max.top()),
                    max.max,
                )),
            )
        } else {
            (max, None)
        };
        let list_rect = list_band.shrink2(egui::vec2(pad, pad));

        let ((picker, picked), _) =
            crate::style::place_at(ui, list_rect, egui::Layout::top_down(egui::Align::Min), |ui| {
                let picker = executor.try_write().ok().and_then(|mut guard| {
                    guard
                        .as_mut()
                        .map(|e| e.show_result_picker(ui, allow_kb_nav))
                });
                match picker {
                    Some(picker) => (picker, true),
                    None => {
                        ui.centered_and_justified(|ui| ui.spinner());
                        (PickerResponse::default(), false)
                    }
                }
            });
        crate::style::claim(ui, list_band);

        if picked {
            if show_preview {
                self.set_preview(picker.selected);
                if search_type == SearchType::Content {
                    if let Some(md) = self.preview.as_mut().and_then(|t| t.markdown_mut()) {
                        md.preview_navigate(picker.selected_range.clone());
                    }
                }
            } else {
                self.preview = None;
            }
        }

        if let Some(preview_rect) = preview_rect {
            ui.painter().vline(
                list_band.right(),
                list_band.y_range(),
                egui::Stroke { width: STROKE_HAIRLINE, color: ui.ctx().get_lb_theme().neutral() },
            );
            crate::style::place_at(
                ui,
                preview_rect,
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_clip_rect(ui.max_rect());
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
            crate::style::claim(ui, preview_rect);
        }

        picker.activated.map(|id| (id, picker.activated_in_new_tab))
    }

    /// Hairline divider using the shared stroke token.
    fn hairline(ui: &mut Ui, horizontal: bool) {
        let t = ui.ctx().get_lb_theme();
        let stroke = egui::Stroke { width: STROKE_HAIRLINE, color: t.neutral() };
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

use egui::{Context, Ui, Vec2};
use lb_rs::blocking::Lb;
use lb_rs::model::path_ops::Filter;
use lb_rs::search::SearchFilter;

use crate::{
    file_cache::FilesExt,
    search::{content::ContentSearch, path::PathSearch},
    show::InputStateExt,
    style::{
        Field, LIST_PAD, STROKE_HAIRLINE, Space, Spacer, Theme, ThemeExt, TypeRole, control_height,
        icon_button, phosphor, segmented, segmented_width,
    },
    tab::{ContentState, TabContent},
    workspace::Workspace,
};

#[tracing::instrument(level = "trace", skip_all)]
fn load_folders(folders: Arc<RwLock<Vec<(lb_rs::Uuid, String)>>>, core: Lb, ctx: Context) {
    let loaded = core
        .list_paths_with_ids(Some(Filter::FoldersOnly))
        .unwrap_or_default();
    *folders.write().unwrap() = loaded;
    ctx.request_repaint();
}

#[tracing::instrument(level = "trace", skip_all)]
fn build_index(
    executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>, building: Arc<AtomicBool>, core: Lb,
    ctx: Context, search_type: SearchType,
) {
    let mut guard = executor.write().unwrap();
    *guard = Some(search_type.create_executor(&core));
    drop(guard);
    building.store(false, Ordering::SeqCst);
    ctx.request_repaint();
}

#[tracing::instrument(level = "trace", skip_all)]
fn run_query(executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>, ctx: Context, query: String) {
    if let Some(executor) = executor.write().unwrap().as_mut() {
        executor.handle_query(&query);
    }
    ctx.request_repaint();
}

#[tracing::instrument(level = "trace", skip_all)]
fn update_filter(
    executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>, ctx: Context,
    filter: Option<SearchFilter>,
) {
    if let Some(executor) = executor.write().unwrap().as_mut() {
        executor.update_filter(filter);
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod layout_diag {
    use super::{picker_bands, prompt_band_h};
    use crate::style::{STROKE_HAIRLINE, Space, control_height};
    use egui::{Rect, pos2, vec2};

    fn fmt(r: Rect) -> String {
        format!(
            "x={:.1}..{:.1} y={:.1}..{:.1}  w={:.1} h={:.1}",
            r.left(),
            r.right(),
            r.top(),
            r.bottom(),
            r.width(),
            r.height()
        )
    }

    /// Headless dump of picker bands (titleband + editor rest).
    #[test]
    fn diagnose_picker_bands() {
        let max = Rect::from_min_size(pos2(0.0, 40.0), vec2(1200.0, 760.0));
        let ch = control_height();
        eprintln!("=== SEARCH PICKER BAND DIAG ===");
        eprintln!(
            "control_height={ch:.1} Sm={:.0} Xs={:.0} hairline={STROKE_HAIRLINE}",
            Space::Sm.pts(),
            Space::Xs.pts()
        );
        eprintln!("max {}", fmt(max));
        for filters in [false, true] {
            let (results, prompt) = picker_bands(max, filters);
            let expect = prompt_band_h(filters);
            eprintln!();
            eprintln!("-- filters_open={filters} --");
            eprintln!("results {}", fmt(results));
            eprintln!("prompt  {}", fmt(prompt));
            eprintln!("prompt_h {:.1}  expected {:.1}", prompt.height(), expect);
            eprintln!("gap results.bottom→prompt.top {:.1}", prompt.top() - results.bottom());
            assert!(
                (prompt.height() - expect).abs() < 0.01,
                "prompt height {} != {expect}",
                prompt.height()
            );
            assert!((results.bottom() - prompt.top()).abs() < 0.01, "bands must share the split");
            assert!((prompt.bottom() - max.bottom()).abs() < 0.01);
            assert!((results.top() - max.top()).abs() < 0.01);
            assert!(
                (results.height() + prompt.height() - max.height()).abs() < 0.01,
                "bands must cover max"
            );
            assert!(results.height() > 200.0, "results pane collapsed");
        }
    }
}
