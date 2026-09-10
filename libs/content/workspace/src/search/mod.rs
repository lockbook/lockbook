pub mod content;
pub mod path;

pub struct Search {
    pub search_type: SearchType,
    pub query: String,
    pub initialized: bool,
    pub executor: Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
    pub scope_path: String,
    scope_open: bool,
    /// Draft dest while the folder sheet is open (commit on Search).
    scope_dest: Option<lb_rs::Uuid>,
    /// Expanded folders in the scope tree picker.
    scope_expanded: std::collections::HashSet<lb_rs::Uuid>,
    query_focused: bool,
    /// Enter in the query field: activate the highlighted result this frame.
    enter_activate: bool,
    /// ⌘Enter: same as Enter, but open in a new tab.
    enter_new_tab: bool,
    /// Esc after chip/query: leave search (back, or close a disposable tab).
    dismiss: bool,
    dispatched_query: String,
    dispatched_filter: String,
    building: Arc<AtomicBool>,
    building_started: web_time::Instant,

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
    /// Empty-state control: clear the folder chip and search everywhere.
    pub clear_scope: bool,
}

pub trait SearchExecutor: Send + Sync {
    fn search_type(&self) -> SearchType;
    fn handle_query(&mut self, query: &str);
    fn update_filter(&mut self, filter: Option<SearchFilter>);
    fn set_kb_mode(&mut self, kb_mode: bool);
    /// Activate the highlighted result (query-field Enter).
    fn request_activate(&mut self);
    /// Activate the highlighted result in a new tab (⌘Enter).
    fn request_activate_in_new_tab(&mut self);
    fn has_rows(&self) -> bool;
    /// Render the result list. `activated` is set when the user opens a result
    /// (e.g. Enter or row shortcut); `selected` tracks the highlighted row for
    /// the preview pane. `scope_name` is the chip folder (glyphon, may contain emoji).
    fn show_result_picker(
        &mut self, ui: &mut Ui, allow_kb_nav: bool, scope_name: &str, empty_centered: bool,
    ) -> PickerResponse;
}

impl Search {
    pub fn new(lb: &Lb, ctx: &Context) -> Search {
        let mut search = Search {
            search_type: SearchType::Path,
            query: String::new(),
            initialized: false,
            executor: Arc::new(RwLock::new(None)),
            scope_path: String::new(),
            scope_open: false,
            scope_dest: None,
            scope_expanded: std::collections::HashSet::new(),
            query_focused: false,
            enter_activate: false,
            enter_new_tab: false,
            dismiss: false,
            dispatched_query: String::new(),
            dispatched_filter: String::new(),
            building: Arc::new(AtomicBool::new(false)),
            building_started: web_time::Instant::now(),
            core: lb.clone(),
        };
        search.spawn_build(ctx);
        search
    }

    /// Home / whole tree. Used when opening search via ⌘O / ⌘⇧F.
    pub fn clear_scope(&mut self) {
        self.scope_path.clear();
        self.scope_open = false;
        self.scope_dest = None;
    }

    fn spawn_build(&mut self, ctx: &Context) {
        self.building.store(true, Ordering::SeqCst);
        self.building_started = web_time::Instant::now();
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

        // Query/filter are in-memory; run them here so the UI isn't locked
        // out (a background write made try_write fail and flashed a spinner
        // between keystrokes). Index construction stays on a thread.
        if self.query != self.dispatched_query {
            if let Ok(mut guard) = self.executor.try_write() {
                if let Some(e) = guard.as_mut() {
                    e.handle_query(&self.query);
                    self.dispatched_query = self.query.clone();
                }
            }
        }

        if self.scope_path != self.dispatched_filter {
            if let Ok(mut guard) = self.executor.try_write() {
                if let Some(e) = guard.as_mut() {
                    let filter = if self.scope_path.is_empty() {
                        None
                    } else {
                        Some(SearchFilter::Path(self.scope_path.clone()))
                    };
                    e.update_filter(filter);
                    self.dispatched_filter = self.scope_path.clone();
                }
            }
        }
    }

    /// Prompt row: field + folder-name chip + picker switch.
    ///
    /// Measure trailing chrome first (chip, then segmented), then shrink the
    /// field into the remainder. Chip is accent folder + name; a trailing X
    /// appears only when scoped away from home. Click opens a light folder sheet.
    fn show_prompt(
        &mut self, ui: &mut Ui, t: &Theme,
        files: &std::sync::Arc<std::sync::RwLock<crate::file_cache::FileCache>>,
    ) {
        let h = control_height();
        let pad = Space::Sm.pts();
        let gap = Space::Xs.pts();
        let host = egui::Id::new("search_query");
        let edit_id = host.with("edit");
        // Sheet / this prompt own Enter/Esc before the query Field (it swallows
        // both while focused). Sticky restore would also steal focus back after
        // the chip click that opened the sheet.
        let mut sheet_enter = false;
        if self.scope_open {
            ui.memory_mut(|m| m.surrender_focus(edit_id));
            sheet_enter = ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
            });
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            self.enter_activate = true;
            self.enter_new_tab = true;
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            // Query field would swallow Enter; activate the highlighted result instead.
            self.enter_activate = true;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.scope_open {
                self.close_scope_sheet();
            } else if !self.scope_path.is_empty() {
                self.scope_path.clear();
            } else if !self.query.is_empty() {
                self.query.clear();
            } else {
                self.dismiss = true;
            }
        }
        let hint = match self.search_type {
            SearchType::Path => "Find files",
            SearchType::Content => "Search in files",
        };
        let opts = ["Filenames", "Contents"];
        let mut kind = match self.search_type {
            SearchType::Path => 0,
            SearchType::Content => 1,
        };

        let folder_name = {
            let files = files.read().unwrap();
            scope_folder_name(&files, &self.scope_path)
        };

        let at_home = self.scope_path.is_empty();
        let full_w = crate::style::ui_width(ui);
        let inner_w = (full_w - pad * 2.0).max(1.0);
        let seg_w = segmented_width(ui, t, &opts);
        let chip_w = scope_chip_width(ui, &folder_name, !at_home);
        let field_w = (inner_w - chip_w - gap - seg_w - gap).max(1.0);

        let origin = crate::style::origin(ui);
        let row = Rect::from_min_size(pos2(origin.x + pad, origin.y), vec2(inner_w, h));
        let mut x = row.left();

        crate::style::place_at(
            ui,
            Rect::from_min_size(pos2(x, row.top()), vec2(field_w, h)),
            Layout::left_to_right(Align::Center),
            |ui| {
                Field::new(t, &mut self.query)
                    .id(host)
                    .hint(hint)
                    .leading(phosphor::SEARCH)
                    .clearable(true)
                    .sticky(!self.scope_open)
                    .width(field_w)
                    .show(ui);
            },
        );
        x += field_w + gap;

        let chip_r = Rect::from_min_size(pos2(x, row.top()), vec2(chip_w, h));
        let (chip, cleared) =
            paint_scope_chip(ui, t, chip_r, &folder_name, self.scope_open, !at_home);
        crate::style::tip_text(
            ui.ctx(),
            &chip,
            "Search is scoped to this folder. Click to choose another.",
        );
        x += chip_w + gap;
        if cleared {
            self.scope_path.clear();
            self.close_scope_sheet();
        }

        crate::style::place_at(
            ui,
            Rect::from_min_size(pos2(x, row.top()), vec2(seg_w, h)),
            Layout::left_to_right(Align::Center),
            |ui| {
                if segmented(ui, t, &opts, &mut kind).changed() {
                    self.search_type =
                        if kind == 0 { SearchType::Path } else { SearchType::Content };
                    self.query.clear();
                }
            },
        );

        crate::style::claim(ui, Rect::from_min_size(origin, vec2(full_w, h)));

        let mut opened = false;
        if !cleared && chip.clicked() {
            if self.scope_open {
                self.close_scope_sheet();
            } else {
                self.open_scope_sheet(ui.ctx(), files);
                opened = true;
            }
        }
        self.show_scope_sheet(ui.ctx(), t, files, opened, sheet_enter);

        self.query_focused = !self.scope_open && ui.memory(|m| m.has_focus(edit_id));
        if !self.scope_open && (!self.initialized || ui.ctx().memory(|m| m.focused().is_none())) {
            self.initialized = true;
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
    }

    fn open_scope_sheet(
        &mut self, ctx: &egui::Context,
        files: &std::sync::Arc<std::sync::RwLock<crate::file_cache::FileCache>>,
    ) {
        let files = files.read().unwrap();
        let root = files.root().id;
        let id = files
            .by_path(&self.scope_path)
            .map(|f| f.id)
            .unwrap_or(root);
        crate::style::expand_ancestors_of(&*files, id, &mut self.scope_expanded);
        ctx.data_mut(|d| {
            d.remove::<bool>(crate::style::folder_tree_scroll_key("search_scope"));
        });
        self.scope_dest = Some(id);
        self.scope_open = true;
        ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("search_query").with("edit")));
    }

    fn close_scope_sheet(&mut self) {
        self.scope_open = false;
        self.scope_dest = None;
    }

    fn show_scope_sheet(
        &mut self, ctx: &egui::Context, t: &Theme,
        files: &std::sync::Arc<std::sync::RwLock<crate::file_cache::FileCache>>,
        opened_this_frame: bool, enter_commit: bool,
    ) {
        if !self.scope_open {
            return;
        }

        let dest = {
            let files = files.read().unwrap();
            let root = files.root().id;
            self.scope_dest
                .or_else(|| files.by_path(&self.scope_path).map(|f| f.id))
                .or(Some(root))
        };

        let files_g = files.read().unwrap();
        let mut out = crate::style::show_folder_sheet(
            ctx,
            t,
            &*files_g,
            &mut self.scope_expanded,
            dest,
            &[],
            "search_scope",
            "Folder",
            "Choose a folder to search in.",
            "Done",
            crate::style::SheetFooterOpts::default()
                .divider(false)
                .quiet_primary(true)
                .primary_shortcut(crate::style::shortcut_enter()),
            |_| {},
        );
        drop(files_g);

        if enter_commit && dest.is_some() {
            out.confirm = true;
        }

        if let Some(id) = out.picked {
            self.scope_dest = Some(id);
        }
        if out.dismiss && !opened_this_frame {
            self.close_scope_sheet();
        }
        if out.confirm {
            if let Some(id) = self.scope_dest.or(dest) {
                let files = files.read().unwrap();
                if id == files.root().id {
                    self.scope_path.clear();
                } else {
                    self.scope_path = files.path(id);
                }
            }
            self.close_scope_sheet();
        }
    }
}

const SCOPE_CHIP_MAX_W: f32 = 180.0;

fn scope_clear_sz() -> f32 {
    crate::style::control_icon_hit()
}

fn scope_chip_width(ui: &Ui, name: &str, show_clear: bool) -> f32 {
    let h = control_height();
    let pad = crate::style::space::control::PAD_X.pts() * 2.0;
    let icon = crate::style::tree_metrics::ICON_SLOT;
    let clear =
        if show_clear { crate::style::space::control::PAD_X.pts() + scope_clear_sz() } else { 0.0 };
    let inner_max = (SCOPE_CHIP_MAX_W - pad - icon - clear).max(1.0);
    let inner = crate::style::file_name::measure_sized(
        ui,
        name,
        crate::style::file_name::body_font_size(),
        crate::style::file_name::body_line_height(),
        inner_max,
    );
    (pad + icon + inner + clear).clamp(h, SCOPE_CHIP_MAX_W)
}

fn paint_scope_chip(
    ui: &mut Ui, t: &Theme, rect: Rect, name: &str, open: bool, show_clear: bool,
) -> (egui::Response, bool) {
    let pad = crate::style::space::control::PAD_X.pts();
    let clear_sz = scope_clear_sz();
    let clear_reserve = if show_clear { pad + clear_sz } else { 0.0 };
    let name_r = Rect::from_min_max(
        pos2(rect.left(), rect.top()),
        pos2((rect.right() - clear_reserve).max(rect.left() + pad), rect.bottom()),
    );
    let resp = ui.interact(name_r, ui.id().with("search_scope_chip"), crate::style::sense_click());
    let fills = crate::style::quiet_canvas_fills(t);
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), name_r) || open;
    let fill = crate::style::interact_fill(
        ui.ctx(),
        resp.id,
        over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    ui.painter()
        .rect_filled(rect, crate::style::Radius::Control.corner(), fill);

    let icon_ink = t.accent();
    let ig = ui.painter().layout_no_wrap(
        phosphor::FOLDER.into(),
        crate::style::phosphor_ui_font_id(),
        icon_ink,
    );
    ui.painter()
        .galley(pos2(rect.left() + pad, rect.center().y - ig.size().y / 2.0), ig, icon_ink);
    let name_slot = Rect::from_min_max(
        pos2(rect.left() + pad + crate::style::tree_metrics::ICON_SLOT, rect.top()),
        pos2((rect.right() - pad - clear_reserve).max(rect.left() + pad), rect.bottom()),
    );
    crate::style::paint_file_name(ui, name, t.neutral_fg(), name_slot);

    let mut cleared = false;
    if show_clear {
        let xr = Rect::from_center_size(
            pos2(rect.right() - pad - clear_sz / 2.0, rect.center().y),
            vec2(clear_sz, clear_sz),
        );
        let x_resp = crate::style::place_at(ui, xr, Layout::left_to_right(Align::Center), |ui| {
            crate::style::icon_button_hit(ui, t, phosphor::X, false, fill, clear_sz)
        })
        .0;
        cleared = x_resp.clicked();
    }
    (resp, cleared)
}

/// Prompt stack at the top of the picker (pads + field).
fn prompt_band_h() -> f32 {
    Space::Sm.pts() + control_height() + Space::Sm.pts()
}

/// Prompt above, results below. Adjacent, covering `max`.
fn picker_bands(max: egui::Rect) -> (egui::Rect, egui::Rect) {
    let ph = prompt_band_h().min(max.height().max(0.0));
    let split_y = max.top() + ph;
    let prompt = egui::Rect::from_min_max(max.min, egui::pos2(max.right(), split_y));
    let results = egui::Rect::from_min_max(egui::pos2(max.left(), split_y), max.max);
    (results, prompt)
}

impl Workspace {
    /// Full-screen picker: prompt on top, results | preview below.
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
            let index_age = search
                .building
                .load(Ordering::SeqCst)
                .then(|| search.building_started.elapsed().as_secs_f32());
            (
                search.executor.clone(),
                search.search_type,
                search.query_focused && !search.scope_open,
                index_age,
                search.scope_path.clone(),
            )
        };
        let (executor, search_type, query_focused, index_age, scope_path) = extracted;
        let folder_name = {
            let files = self.files.read().unwrap();
            scope_folder_name(&files, &scope_path)
        };
        let (results_rect, prompt_rect) = picker_bands(max);

        let t = ui.ctx().get_lb_theme();
        let files = self.files.clone();
        crate::style::place_at(ui, prompt_rect, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(prompt_rect.width());
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.add(Spacer::new(Space::Sm));
            if let Some(tab) = self.current_tab_mut() {
                if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                    search.show_prompt(ui, &t, &files);
                }
            }
            ui.add(Spacer::new(Space::Sm));
        });

        let (enter_activate, enter_new_tab, dismiss) = {
            let Some(tab) = self.current_tab_mut() else {
                return;
            };
            let ContentState::Open(TabContent::Search(search)) = &mut tab.content else {
                return;
            };
            (
                std::mem::take(&mut search.enter_activate),
                std::mem::take(&mut search.enter_new_tab),
                std::mem::take(&mut search.dismiss),
            )
        };
        if dismiss {
            self.dismiss_search();
            return;
        }

        let ((activated, clear_scope), _) = crate::style::place_at(
            ui,
            results_rect,
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                self.results_and_preview(
                    ui,
                    &executor,
                    search_type,
                    query_focused,
                    enter_activate,
                    enter_new_tab,
                    index_age,
                    &folder_name,
                )
            },
        );
        crate::style::claim(ui, max);

        if clear_scope {
            if let Some(tab) = self.current_tab_mut() {
                if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                    search.scope_path.clear();
                    search.dispatched_filter.clear();
                }
            }
            if let Ok(mut guard) = executor.try_write() {
                if let Some(e) = guard.as_mut() {
                    e.update_filter(None);
                }
            }
        }

        let Some((id, new_tab)) = activated else {
            return;
        };
        if self.is_folder(id) {
            let files = self.files.read().unwrap();
            let root = files.root().id;
            let path = if id == root { String::new() } else { files.path(id) };
            drop(files);
            if let Some(tab) = self.current_tab_mut() {
                if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                    search.scope_path = path.clone();
                    search.query.clear();
                    search.scope_open = false;
                    search.dispatched_query.clear();
                    search.dispatched_filter = path.clone();
                }
            }
            if let Ok(mut guard) = executor.try_write() {
                if let Some(e) = guard.as_mut() {
                    e.handle_query("");
                    let filter =
                        if path.is_empty() { None } else { Some(SearchFilter::Path(path)) };
                    e.update_filter(filter);
                }
            }
            self.out.selected_file = Some(id);
            self.out.selected_folder_changed = true;
            self.focused_parent = Some(id);
        } else {
            // Search stays in the strip. Activate an existing session; otherwise
            // create. ⌘-click leaves Search current.
            self.focus_or_create_file(id, !new_tab);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn results_and_preview(
        &mut self, ui: &mut Ui, executor: &Arc<RwLock<Option<Box<dyn SearchExecutor>>>>,
        search_type: SearchType, allow_kb_nav: bool, enter_activate: bool, enter_new_tab: bool,
        index_age: Option<f32>, scope_name: &str,
    ) -> (Option<(lb_rs::Uuid, bool)>, bool) {
        const MIN_PREVIEW_WIDTH: f32 = 560.0;
        let pad = LIST_PAD.pts();
        let max = ui.max_rect();
        let has_rows = executor
            .try_read()
            .ok()
            .and_then(|g| g.as_ref().map(|e| e.has_rows()))
            .unwrap_or(false);
        let preview_idle = self.preview.is_none() && self.preview_pending.is_none();
        // Nothing in either pane: one empty state, full width (no split / placeholder).
        let unify_empty = !has_rows && preview_idle;
        let show_preview = max.width() >= MIN_PREVIEW_WIDTH && !unify_empty;

        // Split full-bleed. L/R LIST_PAD only — first row shares a top with
        // the preview (Files tree), not Recents/Shared’s all-sides wrap.
        let (list_band, preview_rect) = if show_preview {
            let split = max.left() + max.width() * 0.33;
            (
                egui::Rect::from_min_max(max.min, egui::pos2(split, max.bottom())),
                Some(egui::Rect::from_min_max(egui::pos2(split, max.top()), max.max)),
            )
        } else {
            (max, None)
        };
        let list_rect = list_band.shrink2(egui::vec2(pad, 0.0));

        let ((picker, picked), _) =
            crate::style::place_at(ui, list_rect, egui::Layout::top_down(egui::Align::Min), |ui| {
                match executor.try_write() {
                    Ok(mut guard) => match guard.as_mut() {
                        Some(e) => {
                            if enter_activate {
                                if enter_new_tab {
                                    e.request_activate_in_new_tab();
                                } else {
                                    e.request_activate();
                                }
                            }
                            (e.show_result_picker(ui, allow_kb_nav, scope_name, unify_empty), true)
                        }
                        None => (index_not_ready(ui, search_type, index_age, unify_empty), false),
                    },
                    // Query/build thread holds the lock — keep the pane, don't spinner.
                    Err(_) => (PickerResponse::default(), false),
                }
            });
        crate::style::claim(ui, list_band);

        if picked {
            if show_preview {
                self.set_preview(picker.selected);
                if let Some(pending) = &self.preview_pending {
                    pending.warm_preview();
                }
                self.promote_preview();
                if search_type == SearchType::Content {
                    let id = picker.selected;
                    let range = picker.selected_range.clone();
                    if let Some(md) = self
                        .preview
                        .as_mut()
                        .filter(|t| t.id() == id)
                        .and_then(|t| t.markdown_mut())
                    {
                        md.preview_navigate(range);
                    } else if let Some(md) = self
                        .preview_pending
                        .as_mut()
                        .filter(|t| t.id() == id)
                        .and_then(|t| t.markdown_mut())
                    {
                        md.preview_navigate(range);
                    }
                }
            } else {
                self.preview = None;
                self.preview_pending = None;
            }
        }

        if let Some(preview_rect) = preview_rect {
            crate::style::place_at(
                ui,
                preview_rect,
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_clip_rect(ui.max_rect());
                    ui.push_id("search_preview", |ui| {
                        const LOAD_DELAY_SECS: f32 = 0.20;
                        if let Some(pending) = &self.preview_pending {
                            pending.warm_preview();
                        }
                        let pending_loading = self
                            .preview_pending
                            .as_ref()
                            .is_some_and(|tab| !tab.preview_ready());
                        let elapsed = self
                            .preview_pending
                            .as_ref()
                            .map(|tab| tab.last_changed.elapsed().as_secs_f32())
                            .unwrap_or(0.0);
                        if pending_loading && elapsed >= LOAD_DELAY_SECS {
                            crate::style::loading_indicator(ui);
                        } else if let Some(tab) = self.preview.as_mut() {
                            if let Some(pdf) = tab.pdf_mut() {
                                pdf.compact = !self.desktop_tab_policy;
                            }
                            tab.show(ui);
                            if pending_loading {
                                ui.ctx()
                                    .request_repaint_after(std::time::Duration::from_secs_f32(
                                        (LOAD_DELAY_SECS - elapsed).max(1.0 / 60.0),
                                    ));
                            }
                        } else if pending_loading {
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_secs_f32(
                                    (LOAD_DELAY_SECS - elapsed).max(1.0 / 60.0),
                                ));
                            preview_placeholder(ui);
                        } else {
                            preview_placeholder(ui);
                        }
                    });
                },
            );
            crate::style::claim(ui, preview_rect);
        }

        (picker.activated.map(|id| (id, picker.activated_in_new_tab)), picker.clear_scope)
    }
}

fn paint_search_empty(
    ui: &mut Ui, title: &str, subtitle: &str, offer_clear: bool, in_folder: Option<&str>,
    center: bool,
) -> bool {
    let t = ui.ctx().get_lb_theme();
    let muted = t.neutral_fg_secondary();
    let rect = ui.available_rect_before_wrap();
    let icon_g = ui.painter().layout_no_wrap(
        phosphor::SEARCH.into(),
        crate::style::phosphor_ui_font_id(),
        t.accent(),
    );
    let mut content_h = icon_g.size().y + Space::Sm.pts() + TypeRole::Heading.line_height();
    if !subtitle.is_empty() {
        content_h += Space::Xxs.pts() + TypeRole::Body.line_height();
    }
    if offer_clear {
        content_h += Space::Sm.pts() + control_height();
    }
    let y = if center {
        (rect.center().y - content_h / 2.0).max(rect.top())
    } else {
        rect.top() + Space::Lg.pts()
    };
    let block = Rect::from_min_size(
        pos2(rect.left(), y),
        vec2(rect.width(), content_h.min(rect.height()).max(1.0)),
    );
    let (clear, _) = crate::style::place_at(ui, block, Layout::top_down(Align::Center), |ui| {
        let (icon_rect, _) = ui.allocate_exact_size(icon_g.size(), egui::Sense::hover());
        ui.painter().galley(icon_rect.min, icon_g, t.accent());
        ui.add(Spacer::new(Space::Sm));
        paint_empty_title(ui, &t, title, in_folder);
        if !subtitle.is_empty() {
            ui.add(Spacer::new(Space::Xxs));
            ui.label(TypeRole::Body.rich(subtitle).color(muted));
        }
        let mut clear = false;
        if offer_clear {
            ui.add(Spacer::new(Space::Sm));
            if crate::style::Button::secondary(&t, "Search everywhere")
                .max_width(220.0)
                .show(ui)
                .clicked()
            {
                clear = true;
            }
        }
        clear
    });
    crate::style::claim(ui, rect);
    clear
}

/// Chip / empty-state folder: glyphon display name (emoji-safe).
fn scope_folder_name(files: &crate::file_cache::FileCache, scope_path: &str) -> String {
    let f = files.by_path(scope_path).unwrap_or_else(|| files.root());
    crate::style::display_file_name(&f.name).to_owned()
}

/// Heading; when scoped, “{title} in **folder**” via glyphon so emoji names shape.
fn paint_empty_title(ui: &mut Ui, t: &Theme, title: &str, in_folder: Option<&str>) {
    let max_w = (ui.max_rect().width() - Space::Lg.pts() * 2.0).max(40.0);
    let fs = TypeRole::Heading.size();
    let lh = TypeRole::Heading.line_height();
    let ink = t.neutral_fg();
    if let Some(folder) = in_folder {
        let prefix = format!("{title} in ");
        ui.add(
            crate::widgets::GlyphonLabel::new_rich(vec![(&prefix, false), (folder, true)], ink)
                .font_size(fs)
                .line_height(lh)
                .max_width(max_w)
                .text_overflow(crate::widgets::TextOverflow::EndEllipsis),
        );
    } else {
        ui.add(
            crate::widgets::GlyphonLabel::new(title, ink)
                .font_size(fs)
                .line_height(lh)
                .max_width(max_w)
                .text_overflow(crate::widgets::TextOverflow::EndEllipsis),
        );
    }
}

fn index_not_ready(
    ui: &mut Ui, search_type: SearchType, index_age: Option<f32>, empty_centered: bool,
) -> PickerResponse {
    const DELAY: f32 = 0.20;
    if index_age.is_none_or(|age| age >= DELAY) {
        crate::style::loading_indicator(ui);
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f32(
                (DELAY - index_age.unwrap_or(0.0)).max(1.0 / 60.0),
            ));
        if search_type == SearchType::Content {
            paint_search_empty(
                ui,
                "Search in files",
                "Type to search file contents",
                false,
                None,
                empty_centered,
            );
        }
    }
    PickerResponse::default()
}

fn preview_placeholder(ui: &mut Ui) {
    let t = ui.ctx().get_lb_theme();
    ui.centered_and_justified(|ui| {
        ui.label(
            TypeRole::Body
                .rich("Select a result to preview")
                .color(t.neutral_fg_secondary()),
        );
    });
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use egui::{Align, Context, Layout, Rect, Ui, Vec2, pos2, vec2};
use lb_rs::blocking::Lb;
use lb_rs::search::SearchFilter;

use crate::{
    file_cache::FilesExt,
    search::{content::ContentSearch, path::PathSearch},
    style::{
        Field, LIST_PAD, Space, Spacer, Theme, ThemeExt, TypeRole, control_height, phosphor,
        segmented, segmented_width,
    },
    tab::{ContentState, TabContent},
    workspace::Workspace,
};

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
        let (results, prompt) = picker_bands(max);
        let expect = prompt_band_h();
        eprintln!("results {}", fmt(results));
        eprintln!("prompt  {}", fmt(prompt));
        eprintln!("prompt_h {:.1}  expected {:.1}", prompt.height(), expect);
        eprintln!("gap prompt.bottom→results.top {:.1}", results.top() - prompt.bottom());
        assert!(
            (prompt.height() - expect).abs() < 0.01,
            "prompt height {} != {expect}",
            prompt.height()
        );
        assert!((prompt.bottom() - results.top()).abs() < 0.01, "bands must share the split");
        assert!((prompt.top() - max.top()).abs() < 0.01);
        assert!((results.bottom() - max.bottom()).abs() < 0.01);
        assert!(
            (results.height() + prompt.height() - max.height()).abs() < 0.01,
            "bands must cover max"
        );
        assert!(results.height() > 200.0, "results pane collapsed");
    }
}
