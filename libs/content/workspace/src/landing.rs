use egui::{
    Align, Button, Color32, Direction, FontId, Frame, Key, Layout, Modifiers, Rect, RichText,
    Sense, Stroke, UiBuilder, Vec2,
};
use lb_rs::Uuid;
use lb_rs::model::account::Account;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::usage::bytes_to_human;
use serde::{Deserialize, Serialize};
use std::f32;
use std::ops::BitOrAssign;
use std::sync::Arc;

use crate::file_cache::{FileCache, FilesExt};
use crate::show::{DocType, ElapsedHumanString as _, InputStateExt};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::{GlyphonLabel, GlyphonTextEdit, IconButton};
use crate::workspace::Workspace;

/// One explorer tab: a sticky working directory plus the filter/sort UI state
/// and transient rename state. The surface this renders serves as landing page,
/// new-tab page, folder view, file explorer, and (later) search — distinguished
/// only by `scope` and `page.search_term`.
pub struct Explorer {
    /// Per-tab instance id, mirrored in `Destination::Explorer`.
    pub instance: Uuid,
    /// The working directory this tab browses. Seeded from `focused_parent`
    /// when the tab opens, then sticky/independent thereafter.
    pub scope: Uuid,
    /// Filter/sort settings. Seeded from (and persisted back to) the shared
    /// landing-page defaults so preferences carry across tabs and sessions.
    pub page: LandingPage,

    pub rename_target: Option<Uuid>,
    pub rename_buffer: String,
    /// Focus the search field on the tab's first rendered frame.
    pub first_frame: bool,

    /// Scope navigation history (folder ids), independent of the tab strip's
    /// file back/forward stacks.
    pub back: Vec<Uuid>,
    pub forward: Vec<Uuid>,
}

impl Explorer {
    pub fn new(instance: Uuid, scope: Uuid, page: LandingPage) -> Self {
        Self {
            instance,
            scope,
            page,
            rename_target: None,
            rename_buffer: String::new(),
            first_frame: true,
            back: Vec::new(),
            forward: Vec::new(),
        }
    }

    /// A cheap stand-in used while the real `Explorer` is swapped out of its
    /// tab for rendering (which needs `&mut Workspace`).
    pub fn placeholder() -> Self {
        Self::new(Uuid::nil(), Uuid::nil(), LandingPage::default())
    }

    /// Navigate into `folder`, recording the current scope for `back`.
    pub fn navigate(&mut self, folder: Uuid) {
        if folder == self.scope {
            return;
        }
        self.back.push(self.scope);
        self.forward.clear();
        self.scope = folder;
    }

    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.back.pop() {
            self.forward.push(self.scope);
            self.scope = prev;
            true
        } else {
            false
        }
    }

    pub fn go_forward(&mut self) -> bool {
        if let Some(next) = self.forward.pop() {
            self.back.push(self.scope);
            self.scope = next;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LandingPage {
    search_term: String,
    doc_types: Vec<DocType>,
    collaborators: Vec<String>,
    only_me: bool,
    sort: Sort,
    sort_asc: bool,
    flatten_tree: bool,

    /// save landing page?
    #[serde(skip)]
    dirty: bool,

    #[serde(skip)]
    cached_files: Vec<File>,

    #[serde(skip)]
    cache_generation: u64,

    #[serde(skip)]
    cache_snapshot: Option<Box<LandingPage>>,
    /// The folder the cached list was built for. Lives outside `PartialEq`
    /// because it's a `Workspace` concern (not a persisted setting), but
    /// it still has to invalidate the cache when the user navigates.
    #[serde(skip)]
    cached_focused_parent: Option<Uuid>,
}

impl PartialEq for LandingPage {
    fn eq(&self, other: &Self) -> bool {
        self.search_term == other.search_term
            && self.doc_types == other.doc_types
            && self.collaborators == other.collaborators
            && self.only_me == other.only_me
            && self.sort == other.sort
            && self.sort_asc == other.sort_asc
            && self.flatten_tree == other.flatten_tree
    }
}

impl Default for LandingPage {
    fn default() -> Self {
        Self {
            search_term: Default::default(),
            doc_types: Default::default(),
            collaborators: Default::default(),
            only_me: Default::default(),
            sort: Default::default(),
            sort_asc: true,
            flatten_tree: true,
            cached_files: Vec::new(),
            cache_generation: 0,
            cache_snapshot: None,
            cached_focused_parent: None,
            dirty: false,
        }
    }
}

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
enum Sort {
    Name,
    Type,
    #[default]
    Modified,
    Collaborators,
    Size,
}

#[derive(Default)]
pub struct Response {
    pub open_file: Option<Uuid>,
    pub create_note: bool,
    pub create_drawing: bool,
    pub create_folder: bool,
    pub rename_request: Option<(Uuid, String)>,
    pub delete_request: Option<Uuid>,
}

impl BitOrAssign for Response {
    fn bitor_assign(&mut self, rhs: Self) {
        self.open_file = self.open_file.or(rhs.open_file);
        self.create_note |= rhs.create_note;
        self.create_drawing |= rhs.create_drawing;
        self.create_folder |= rhs.create_folder;
        self.rename_request = self.rename_request.take().or(rhs.rename_request);
        self.delete_request = self.delete_request.or(rhs.delete_request);
    }
}

// ─── Landing-page row layout ─────────────────────────────────────────────
//
// Heights are constant per row variant so total content height is known
// before any row renders. `show_files` precomputes a `Vec<RowLayout>`,
// hands `total_height` to the scroll area, and renders only rows whose
// rects intersect the visible viewport.

const HEADER_HEIGHT: f32 = 32.0;
const SEPARATOR_HEIGHT: f32 = 36.0;
const FILE_ROW_HEIGHT: f32 = 30.0;
const ROW_CORNER_RADIUS: f32 = 4.0;
const ROW_PAD_X: f32 = 12.0;
/// Reading-width cap for centered content (heading, filters, file rows).
/// Scroll area spans the canvas; content sits in a centered column of
/// at most this width so the scrollbar can live at the canvas right edge.
const MAX_CONTENT_W: f32 = 1000.0;
/// Minimum gap between centered content and the canvas edges. Keeps
/// file rows from running flush against the window when the window is
/// narrower than `MAX_CONTENT_W`.
const CANVAS_GUTTER_X: f32 = 45.0;
/// Vertical band the cell content sits in — sized to fit the file-type
/// icon (mono 19) with a touch of breathing room. `col_rects` centers
/// this band inside `FILE_ROW_HEIGHT` so cells are vertically centered.
const CELL_CONTENT_H: f32 = 22.0;

const COL_NAME_MIN_W: f32 = 240.0;
const COL_MODIFIED_W: f32 = 180.0;
const COL_COLLAB_W: f32 = 100.0;
const COL_SIZE_W: f32 = 80.0;
const COL_USAGE_BAR_W: f32 = 120.0;
const COL_GAP: f32 = 24.0;

#[derive(Clone, Copy)]
struct LayoutCols {
    show_modified: bool,
    show_collab: bool,
    show_size: bool,
    show_usage_bar: bool,
}

/// Drop columns from least-essential first (usage bar → collab → size →
/// modified) until everything fits alongside `COL_NAME_MIN_W` of name
/// space. Name is always shown.
fn layout_cols(width: f32) -> LayoutCols {
    let usable = width - 2.0 * ROW_PAD_X;
    let mut cols = LayoutCols {
        show_modified: true,
        show_collab: true,
        show_size: true,
        show_usage_bar: true,
    };
    let needed = |c: LayoutCols| -> f32 {
        COL_NAME_MIN_W
            + if c.show_modified { COL_GAP + COL_MODIFIED_W } else { 0.0 }
            + if c.show_collab { COL_GAP + COL_COLLAB_W } else { 0.0 }
            + if c.show_size { COL_GAP + COL_SIZE_W } else { 0.0 }
            + if c.show_usage_bar { COL_GAP + COL_USAGE_BAR_W } else { 0.0 }
    };
    if usable < needed(cols) {
        cols.show_usage_bar = false;
    }
    if usable < needed(cols) {
        cols.show_collab = false;
    }
    if usable < needed(cols) {
        cols.show_size = false;
    }
    if usable < needed(cols) {
        cols.show_modified = false;
    }
    cols
}

#[derive(Clone, Copy)]
struct ColRects {
    name: Rect,
    modified: Option<Rect>,
    collab: Option<Rect>,
    size: Option<Rect>,
    usage_bar: Option<Rect>,
}

fn col_rects(row_rect: Rect, cols: LayoutCols) -> ColRects {
    let row_rect = row_rect.shrink2(Vec2::new(ROW_PAD_X, 0.0));
    let center_y = row_rect.center().y;
    let top = center_y - CELL_CONTENT_H / 2.0;
    let bottom = center_y + CELL_CONTENT_H / 2.0;
    let cell = |left: f32, width: f32| {
        Rect::from_min_max(egui::Pos2::new(left, top), egui::Pos2::new(left + width, bottom))
    };
    let mut right = row_rect.right();
    let alloc = |right: &mut f32, width: f32| -> Rect {
        let r = cell(*right - width, width);
        *right -= width + COL_GAP;
        r
    };

    let usage_bar = cols
        .show_usage_bar
        .then(|| alloc(&mut right, COL_USAGE_BAR_W));
    let size = cols.show_size.then(|| alloc(&mut right, COL_SIZE_W));
    let collab = cols.show_collab.then(|| alloc(&mut right, COL_COLLAB_W));
    let modified = cols
        .show_modified
        .then(|| alloc(&mut right, COL_MODIFIED_W));
    let name = cell(row_rect.left(), (right - row_rect.left()).max(0.0));

    ColRects { name, modified, collab, size, usage_bar }
}

#[derive(Clone, Copy)]
enum RowKind {
    TimeSeparator(&'static str),
    File(usize),
}

struct RowLayout {
    y_top: f32,
    height: f32,
    kind: RowKind,
}

fn time_category(diff_millis: i64) -> &'static str {
    let day = 24 * 60 * 60 * 1000;
    if diff_millis <= day {
        "Today"
    } else if diff_millis <= 2 * day {
        "Yesterday"
    } else if diff_millis <= 7 * day {
        "This Week"
    } else if diff_millis <= 30 * day {
        "This Month"
    } else if diff_millis <= 365 * day {
        "This Year"
    } else {
        "All Time"
    }
}

fn build_row_layout(descendents: &[File], sort: &Sort, files: &FileCache) -> (Vec<RowLayout>, f32) {
    let mut rows = Vec::with_capacity(descendents.len() + 8);
    let mut y = 0.0;

    let mut current_category: &str = "";
    for (idx, child) in descendents.iter().enumerate() {
        if *sort == Sort::Modified {
            let now = lb_rs::model::clock::get_time().0;
            let diff = now - files.last_modified_recursive(child.id) as i64;
            let category = time_category(diff);
            if category != current_category {
                rows.push(RowLayout {
                    y_top: y,
                    height: SEPARATOR_HEIGHT,
                    kind: RowKind::TimeSeparator(category),
                });
                y += SEPARATOR_HEIGHT;
                current_category = category;
            }
        }
        rows.push(RowLayout { y_top: y, height: FILE_ROW_HEIGHT, kind: RowKind::File(idx) });
        y += FILE_ROW_HEIGHT;
    }

    (rows, y)
}

impl Workspace {
    /// Render an explorer tab against its own `Explorer` state. Returns the
    /// actions the caller should apply (open/create/rename/delete); folder
    /// navigation is the caller's job too so it can record scope history.
    pub fn render_explorer(&mut self, ui: &mut egui::Ui, ex: &mut Explorer) -> Response {
        const MARGIN: i8 = 45;

        // `show_tabs` zeroes `item_spacing` for the tab strip; the explorer is a
        // full page and expects the app's default spacing.
        ui.spacing_mut().item_spacing = ui.ctx().style().spacing.item_spacing;

        // Self-heal a stale scope (deleted folder) back to root.
        {
            let files = self.files.read().unwrap();
            if files.get_by_id(ex.scope).map(|f| f.is_folder()) != Some(true) {
                ex.scope = files.root().id;
            }
        }
        let scope = ex.scope;

        let mut response = Response::default();

        ui.vertical_centered_justified(|ui| {
            // Heading + filters sit in a `MARGIN`-padded frame; the
            // bottom edge is flush so the gap below is just `add_space`.
            Frame::canvas(ui.style())
                .inner_margin(egui::Margin { left: MARGIN, right: MARGIN, top: MARGIN, bottom: 0 })
                .stroke(Stroke::NONE)
                .fill(Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

                    let canvas_w = ui.available_width();
                    let content_w = canvas_w.min(MAX_CONTENT_W);
                    let pad = ((canvas_w - content_w) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.allocate_ui_with_layout(
                            Vec2::new(content_w, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                response |= self.show_heading(ui, scope);
                                ui.add_space(40.0);
                                response |= self.show_filters(ui, ex, scope);
                            },
                        );
                    });
                });

            ui.add_space(40.0);

            // Files: rendered outside the frame so the scroll area spans
            // the full canvas — the scrollbar lands flush against the
            // canvas right edge. Row content centers internally.
            response |= self.show_files(ui, ex, scope);
        });

        // Persist filter/sort settings if they changed.
        if ex.page.dirty {
            ex.page.dirty = false;
            self.cfg.set_landing_page(ex.page.clone());
        }
        ex.first_frame = false;

        response
    }

    /// "Welcome, <Username>" or selected folder with breadcrumb for parent
    fn show_heading(&mut self, ui: &mut egui::Ui, scope: Uuid) -> Response {
        let mut response = Response::default();

        let files_arc = Arc::clone(&self.files);
        let files_guard = files_arc.read().unwrap();
        let files = &*files_guard;
        let folder = files.get_by_id(scope).unwrap();

        ui.style_mut().visuals.hyperlink_color = ui.visuals().text_color();
        ui.vertical(|ui| {
            if folder.id == files.root().id {
                ui.label(
                    RichText::new("Welcome,")
                        .font(FontId::proportional(24.0))
                        .weak(),
                );
            } else {
                ui.label(
                    RichText::new("Welcome,")
                        .font(FontId::proportional(24.0))
                        .weak()
                        .color(Color32::TRANSPARENT),
                );
            }

            // Breadcrumb / Folder name
            ui.horizontal(|ui| {
                const HEADING_FONT_SIZE: f32 = 40.;
                const HEADING_LINE_HEIGHT: f32 = 56.;

                // Show breadcrumb to parent folder. Skipped at root and at share
                // boundaries where the parent lives in the sharer's tree.
                if let Some(parent) = files.get_by_id(folder.parent).filter(|_| !folder.is_root()) {
                    let resp = ui.add(
                        GlyphonLabel::new(&parent.name, ui.visuals().text_color())
                            .font_size(HEADING_FONT_SIZE)
                            .line_height(HEADING_LINE_HEIGHT)
                            .sense(Sense::click()),
                    );
                    if resp.clicked() {
                        response.open_file = Some(parent.id);
                    }
                    resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                    ui.label(
                        RichText::new(Icon::CHEVRON_RIGHT.icon)
                            .font(FontId::monospace(19.0))
                            .weak(),
                    );
                }

                ui.add(
                    GlyphonLabel::new(&folder.name, ui.visuals().text_color())
                        .font_size(HEADING_FONT_SIZE)
                        .line_height(HEADING_LINE_HEIGHT),
                );
            });
        });

        response
    }

    /// Create button, filter text box, show folders toggle, file types selector, collaborators selector
    fn show_filters(&mut self, ui: &mut egui::Ui, ex: &mut Explorer, scope: Uuid) -> Response {
        let mut response = Response::default();

        let files_arc = Arc::clone(&self.files);
        let files_guard = files_arc.read().unwrap();
        let files = &*files_guard;
        let account = &self.account;
        let folder = files.get_by_id(scope).unwrap();

        ui.horizontal_top(|ui| {
            // experimentally matches combo box height which I cannot figure out how to determine or control
            let filters_height = 35.0;

            // Create button - dropdown for new items
            ui.scope(|ui| {
                let fill = ui
                    .style()
                    .visuals
                    .widgets
                    .active
                    .bg_fill
                    .gamma_multiply(0.8);
                ui.visuals_mut().widgets.noninteractive.weak_bg_fill = fill;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = fill;
                ui.visuals_mut().widgets.hovered.weak_bg_fill = fill;
                ui.visuals_mut().widgets.active.weak_bg_fill = fill;
                ui.visuals_mut().widgets.open.weak_bg_fill = fill;

                egui::ComboBox::from_id_salt("create_combo")
                    .selected_text("Create")
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;

                        ui.with_layout(
                            Layout {
                                main_dir: Direction::LeftToRight,
                                main_wrap: false,
                                main_align: Align::Min,
                                main_justify: true,
                                cross_align: Align::Min,
                                cross_justify: false,
                            },
                            |ui| {
                                if ui.button("Note").clicked() {
                                    response.create_note = true;
                                    ui.close();
                                }
                            },
                        );
                        ui.with_layout(
                            Layout {
                                main_dir: Direction::LeftToRight,
                                main_wrap: false,
                                main_align: Align::Min,
                                main_justify: true,
                                cross_align: Align::Min,
                                cross_justify: false,
                            },
                            |ui| {
                                if ui.button("Drawing").clicked() {
                                    response.create_drawing = true;
                                    ui.close();
                                }
                            },
                        );
                        ui.with_layout(
                            Layout {
                                main_dir: Direction::LeftToRight,
                                main_wrap: false,
                                main_align: Align::Min,
                                main_justify: true,
                                cross_align: Align::Min,
                                cross_justify: false,
                            },
                            |ui| {
                                if ui.button("Folder").clicked() {
                                    response.create_folder = true;
                                    ui.close();
                                }
                            },
                        );
                    });
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                // Collaborators filter
                egui::ComboBox::from_id_salt(ui.next_auto_id())
                    .selected_text(if ex.page.only_me {
                        "Collaborators: Only Me".to_string()
                    } else if ex.page.collaborators.is_empty() {
                        "Collaborators: Any".to_string()
                    } else {
                        format!("Collaborators: {}", ex.page.collaborators.len())
                    })
                    .width(180.)
                    .height(f32::INFINITY)
                    .show_ui(ui, |ui| {
                        // Collect all unique collaborators from files in scope
                        let files_in_scope = if ex.page.flatten_tree {
                            files.descendents(folder.id)
                        } else {
                            files.children(folder.id)
                        };

                        let mut all_collaborators = std::collections::HashSet::new();
                        for file in files_in_scope {
                            for share in &file.shares {
                                if share.shared_with != "<unknown>"
                                    && share.shared_with != account.username
                                {
                                    all_collaborators.insert(share.shared_with.clone());
                                }
                                if share.shared_by != "<unknown>"
                                    && share.shared_by != account.username
                                {
                                    all_collaborators.insert(share.shared_by.clone());
                                }
                            }
                        }

                        let mut collaborators_list: Vec<String> =
                            all_collaborators.into_iter().collect();
                        collaborators_list.sort();

                        ui.style_mut().spacing.button_padding.y = 5.0;
                        ui.add_space(5.);
                        if ui.button("Any").clicked() {
                            ex.page.only_me = false;
                            ex.page.collaborators.clear();
                            ex.page.dirty = true;
                        }
                        ui.add_space(5.);
                        if ui.button("Only Me").clicked() {
                            ex.page.only_me = true;
                            ex.page.collaborators.clear();
                            ex.page.dirty = true;
                        }

                        for collaborator in &collaborators_list {
                            let mut is_selected =
                                ex.page.collaborators.iter().any(|c| c == collaborator);

                            if ui.checkbox(&mut is_selected, collaborator).changed() {
                                if is_selected {
                                    // Add the collaborator if not present
                                    if !ex.page.collaborators.iter().any(|c| c == collaborator) {
                                        ex.page.collaborators.push(collaborator.clone());
                                    }
                                } else {
                                    // Remove the collaborator
                                    ex.page.collaborators.retain(|c| c != collaborator);
                                }

                                ex.page.only_me = false;
                                ex.page.dirty = true;
                            }
                        }
                    });

                // File type filter
                egui::ComboBox::from_id_salt(ui.next_auto_id())
                    .selected_text(if ex.page.doc_types.is_empty() {
                        "Types: Any".to_string()
                    } else {
                        format!("Types: {}", ex.page.doc_types.len())
                    })
                    .width(100.)
                    .height(f32::INFINITY)
                    .show_ui(ui, |ui| {
                        ui.style_mut().spacing.button_padding.y = 5.0;
                        if ui.button("Any").clicked() {
                            ex.page.doc_types.clear();
                            ex.page.dirty = true;
                        }
                        ui.add_space(5.);

                        let doc_types = [
                            DocType::Markdown,
                            DocType::SVG,
                            DocType::Image,
                            DocType::PDF,
                            DocType::PlainText,
                            DocType::Code,
                            DocType::Unknown,
                        ];
                        for doc_type in &doc_types {
                            let mut is_selected = ex.page.doc_types.iter().any(|dt| dt == doc_type);

                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut is_selected, "").changed() {
                                    if is_selected {
                                        // Add the doc type if not present
                                        if !ex.page.doc_types.iter().any(|dt| dt == doc_type) {
                                            ex.page.doc_types.push(*doc_type);
                                            ex.page.dirty = true;
                                        }
                                    } else {
                                        // Remove the doc type
                                        ex.page.doc_types.retain(|&t| t != *doc_type);
                                        ex.page.dirty = true;
                                    }
                                }

                                ui.label(
                                    RichText::new(doc_type.to_icon().icon)
                                        .font(FontId::monospace(16.0))
                                        .color(ui.visuals().weak_text_color()),
                                );
                                ui.label(doc_type.to_string());
                            });

                            ui.add_space(5.);
                        }
                    });

                // Flatten tree toggle
                if IconButton::new(
                    if ex.page.flatten_tree { Icon::FOLDER_OPEN } else { Icon::FOLDER }.size(22.),
                )
                .show(ui)
                .clicked()
                {
                    ex.page.flatten_tree = !ex.page.flatten_tree;
                    ex.page.dirty = true;
                }

                // Search box - takes remaining space
                let search_box_size = Vec2::new(ui.available_width(), filters_height);
                ui.allocate_ui_with_layout(
                    search_box_size,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.painter().rect(
                            ui.max_rect(),
                            filters_height / 2.,
                            ui.visuals().extreme_bg_color,
                            ui.visuals().widgets.noninteractive.bg_stroke,
                            egui::epaint::StrokeKind::Inside,
                        );

                        ui.add_space(15.0); // margin

                        ui.label(
                            RichText::new(Icon::FILTER.icon)
                                .font(FontId::monospace(19.0))
                                .color(ui.visuals().weak_text_color()),
                        );

                        // Check for Cmd+F (or Ctrl+F on non-Mac)
                        let cmd_f =
                            ui.input_mut(|i| i.consume_key_exact(Modifiers::COMMAND, Key::F));

                        let search_id = egui::Id::new("landing_search");

                        // Focus when Cmd+F is pressed or on first frame
                        if cmd_f || ex.first_frame {
                            ui.memory_mut(|m| m.request_focus(search_id));
                        }

                        let hint = if folder.is_root() {
                            "Filter".to_string()
                        } else {
                            format!("Filter in {}", &folder.name)
                        };

                        let has_text = !ex.page.search_term.is_empty();
                        let clear_space = if has_text { 25.0 } else { 0.0 };
                        let edit_width = ui.available_width() - clear_space - 15.0;

                        ui.allocate_ui_with_layout(
                            Vec2::new(edit_width, filters_height),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                if GlyphonTextEdit::new(&mut ex.page.search_term)
                                    .id(search_id)
                                    .hint_text(hint)
                                    .show(ui)
                                    .changed()
                                {
                                    ex.page.dirty = true;
                                }
                            },
                        );

                        // Clear button (X icon) when there's text
                        #[allow(clippy::collapsible_if)]
                        if has_text {
                            if IconButton::new(Icon::CLOSE.size(16.))
                                .hover_bg(false)
                                .show(ui)
                                .clicked()
                            {
                                ex.page.search_term.clear();
                                ex.page.dirty = true;
                            }
                        }
                    },
                );
            });
        });

        response
    }

    fn filtered_sorted_files(ex: &mut Explorer, files: &FileCache, account: &Account, scope: Uuid) {
        let focused = scope;
        if ex.page.cache_generation == files.last_modified
            && ex.page.cache_snapshot.as_deref() == Some(&ex.page)
            && ex.page.cached_focused_parent == Some(focused)
        {
            return;
        }

        let folder = files.get_by_id(focused).unwrap();

        // Filter
        let mut descendents = if ex.page.flatten_tree {
            files.descendents(folder.id)
        } else {
            files.children(folder.id)
        };

        // At root, include pending shares
        if folder.is_root() {
            if ex.page.flatten_tree {
                // When flattening, include all shared files (documents will
                // survive the flatten filter below, just like own-tree files)
                for f in files.shared.values() {
                    descendents.push(f);
                }
            } else {
                // When not flattening, include only share roots
                for f in files.shared.values() {
                    if !files.shared.contains_key(&f.parent) {
                        descendents.push(f);
                    }
                }
            }
        }
        let mut descendents: Vec<_> = descendents
            .into_iter()
            .filter(|child| {
                // Search term filter
                let search_match = if ex.page.search_term.is_empty() {
                    true
                } else {
                    child
                        .name
                        .to_lowercase()
                        .contains(&ex.page.search_term.to_lowercase())
                };

                // Flatten tree filter - hide folders when flattening
                let flatten_match = !ex.page.flatten_tree || child.is_document();

                // Doc type filter
                let doc_type_match = if ex.page.doc_types.is_empty() {
                    true
                } else if child.is_folder() {
                    // hide folders
                    false
                } else {
                    let child_doc_type = DocType::from_name(&child.name);
                    ex.page
                        .doc_types
                        .iter()
                        .any(|filter_type| filter_type == &child_doc_type)
                };

                // Collaborator filter
                let collaborator_match = if ex.page.only_me {
                    // Only show files where the current user is the only collaborator
                    child.shares.is_empty()
                } else if ex.page.collaborators.is_empty() {
                    // No collaborator filter
                    true
                } else {
                    // Get all collaborators for this file
                    let mut file_collaborators = std::collections::HashSet::new();
                    for share in &child.shares {
                        if share.shared_with != "<unknown>" && share.shared_with != account.username
                        {
                            file_collaborators.insert(&share.shared_with);
                        }
                        if share.shared_by != "<unknown>" && share.shared_by != account.username {
                            file_collaborators.insert(&share.shared_by);
                        }
                    }

                    // Check if filter collaborators are a subset of file collaborators
                    ex.page
                        .collaborators
                        .iter()
                        .all(|filter_collab| file_collaborators.contains(&filter_collab))
                };

                search_match && doc_type_match && flatten_match && collaborator_match
            })
            .collect();

        // Sort
        match ex.page.sort {
            Sort::Name => {
                descendents.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
            Sort::Type => {
                descendents.sort_by(|a, b| {
                    // Folders first, then sort by document type
                    match (a.is_folder(), b.is_folder()) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        (true, true) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        (false, false) => {
                            let a_type = DocType::from_name(&a.name);
                            let b_type = DocType::from_name(&b.name);
                            a_type
                                .to_string()
                                .cmp(&b_type.to_string())
                                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                        }
                    }
                })
            }
            Sort::Modified => {
                descendents.sort_by_key(|f| u64::MAX - files.last_modified_recursive(f.id))
            }
            Sort::Collaborators => descendents.sort_by_key(|f| usize::MAX - f.shares.len()),
            Sort::Size => descendents.sort_by_key(|f| u64::MAX - files.size_bytes_recursive[&f.id]),
        }
        if !ex.page.sort_asc {
            descendents.reverse()
        }

        ex.page.cached_files = descendents.into_iter().cloned().collect();
        ex.page.cache_generation = files.last_modified;
        ex.page.cached_focused_parent = Some(focused);
        ex.page.cache_snapshot = Some(Box::new(ex.page.clone()));
    }

    /// Files table with columns that you click to select a sort key
    fn show_files(&mut self, ui: &mut egui::Ui, ex: &mut Explorer, scope: Uuid) -> Response {
        let mut response = Response::default();

        let files_arc = Arc::clone(&self.files);
        let files_guard = files_arc.read().unwrap();
        let files = &*files_guard;
        let account = self.account.clone();
        Self::filtered_sorted_files(ex, files, &account, scope);

        if ex.page.cached_files.is_empty() {
            if !ex.page.search_term.is_empty() {
                ui.label(
                    RichText::new("No files found matching your search.")
                        .color(ui.visuals().weak_text_color()),
                );
            }
            return response;
        }

        // Scoped to this ui (the file scroll area inherits it) — mutating the
        // ctx style here would leak the solid scrollbar into the tab strip and
        // the host's scroll areas.
        ui.style_mut().spacing.scroll = {
            let mut s = egui::style::ScrollStyle::solid();
            s.bar_width = 10.0;
            s
        };

        let max_usage = ex
            .page
            .cached_files
            .iter()
            .filter_map(|f| files.size_bytes_recursive.get(&f.id).copied())
            .max()
            .unwrap_or(1) as f32;

        let (rows, total_height) = build_row_layout(&ex.page.cached_files, &ex.page.sort, files);

        // Header lives outside the scroll area so it doesn't scroll away.
        // Uses the same centering math as the rows below.
        let avail = ui.available_width();
        let content_w = (avail - 2.0 * CANVAS_GUTTER_X).clamp(0.0, MAX_CONTENT_W);
        let content_x_offset = ((avail - content_w) / 2.0).max(0.0);
        let cols = layout_cols(content_w);
        ui.horizontal(|ui| {
            ui.add_space(content_x_offset);
            ui.allocate_ui_with_layout(
                Vec2::new(content_w, HEADER_HEIGHT),
                Layout::top_down(Align::Min),
                |ui| Self::show_header_row(ex, ui, cols),
            );
        });
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt(("explorer_files", ex.instance))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_viewport(ui, |ui, viewport| {
                // Pin the scroll body to canvas width with a zero-height
                // full-width allocation.
                ui.allocate_space(Vec2::new(avail, 0.0));

                // rows are sorted by y_top — partition_point is O(log N)
                let first = rows.partition_point(|r| r.y_top + r.height < viewport.min.y);
                let last = rows.partition_point(|r| r.y_top < viewport.max.y);

                // Leading skip so the cursor lands at the first visible row.
                let leading = rows.get(first).map_or(total_height, |r| r.y_top);
                if leading > 0.0 {
                    ui.allocate_space(Vec2::new(0.0, leading));
                }

                for row in &rows[first..last] {
                    let cursor = ui.cursor();
                    let row_rect = Rect::from_min_size(
                        egui::Pos2::new(cursor.left() + content_x_offset, cursor.top()),
                        Vec2::new(content_w, row.height),
                    );
                    ui.scope_builder(UiBuilder::new().max_rect(row_rect), |ui| match row.kind {
                        RowKind::TimeSeparator(label) => Self::show_separator_row(ui, label),
                        RowKind::File(idx) => Self::show_file_row(
                            ex,
                            ui,
                            idx,
                            files,
                            &account,
                            max_usage,
                            cols,
                            &mut response,
                        ),
                    });
                }

                // Trailing skip so the scroll area knows the full content height.
                let consumed = rows
                    .get(last.saturating_sub(1))
                    .map_or(leading, |r| r.y_top + r.height);
                let trailing = (total_height - consumed).max(0.0);
                if trailing > 0.0 {
                    ui.allocate_space(Vec2::new(0.0, trailing));
                }
            });

        response
    }

    fn show_header_row(ex: &mut Explorer, ui: &mut egui::Ui, cols: LayoutCols) {
        let header_font = FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold")));
        let rects = col_rects(ui.max_rect(), cols);

        // Helper: render one sort-button header in `rect`. Closure decides
        // what the click means (cycle name<->type, or toggle asc/desc).
        let render_header =
            |page: &mut LandingPage,
             ui: &mut egui::Ui,
             rect: Rect,
             text: &str,
             active: bool,
             on_click: &mut dyn FnMut(&mut LandingPage)| {
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            Button::new(RichText::new(text).font(header_font.clone())).frame(false),
                        );
                        if resp.clicked() {
                            on_click(page);
                        }
                        if active {
                            let chevron =
                                if page.sort_asc { Icon::CHEVRON_DOWN } else { Icon::CHEVRON_UP };
                            ui.label(RichText::new(chevron.icon).font(FontId::monospace(12.0)));
                        }
                    });
                });
            };

        // Name / Type — clicking cycles name asc → desc → type asc → desc → name.
        let name_text = if ex.page.sort == Sort::Type { "Type" } else { "Name" };
        let name_active = matches!(ex.page.sort, Sort::Name | Sort::Type);
        render_header(&mut ex.page, ui, rects.name, name_text, name_active, &mut |page| {
            match (&page.sort, page.sort_asc) {
                (Sort::Name, true) => page.sort_asc = false,
                (Sort::Name, false) => {
                    page.sort = Sort::Type;
                    page.sort_asc = true;
                }
                (Sort::Type, true) => page.sort_asc = false,
                _ => {
                    page.sort = Sort::Name;
                    page.sort_asc = true;
                }
            };
            page.dirty = true;
        });

        let toggle_sort = |page: &mut LandingPage, target: Sort| {
            if page.sort == target {
                page.sort_asc = !page.sort_asc;
            } else {
                page.sort = target;
                page.sort_asc = true;
            }
            page.dirty = true;
        };

        if let Some(rect) = rects.modified {
            let active = ex.page.sort == Sort::Modified;
            render_header(&mut ex.page, ui, rect, "Modified", active, &mut |page| {
                toggle_sort(page, Sort::Modified)
            });
        }

        if let Some(rect) = rects.collab {
            let active = ex.page.sort == Sort::Collaborators;
            render_header(&mut ex.page, ui, rect, "Collaborators", active, &mut |page| {
                toggle_sort(page, Sort::Collaborators)
            });
        }

        if let Some(rect) = rects.size {
            let active = ex.page.sort == Sort::Size;
            render_header(&mut ex.page, ui, rect, "Size", active, &mut |page| {
                toggle_sort(page, Sort::Size)
            });
        }
    }

    fn show_separator_row(ui: &mut egui::Ui, label: &'static str) {
        let row_rect = ui.max_rect().shrink2(Vec2::new(ROW_PAD_X, 0.0));
        // Bottom-align the bold label so the breathing room from the
        // original design sits above the text, not below.
        let label_rect = Rect::from_min_max(
            egui::Pos2::new(row_rect.left(), row_rect.bottom() - 22.0),
            row_rect.max,
        );
        ui.scope_builder(UiBuilder::new().max_rect(label_rect), |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.label(
                    RichText::new(label)
                        .font(FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold"))))
                        .weak(),
                );
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn show_file_row(
        ex: &mut Explorer, ui: &mut egui::Ui, idx: usize, files: &FileCache, account: &Account,
        max_usage: f32, cols: LayoutCols, response: &mut Response,
    ) {
        let row_rect = ui.max_rect();
        let rects = col_rects(row_rect, cols);
        let child = ex.page.cached_files[idx].clone();
        let child = &child;
        let is_renaming = ex.rename_target == Some(child.id);

        // Positional hover check — child widgets would steal `Response::hovered`.
        if ui.rect_contains_pointer(row_rect) {
            let bg = ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.4);
            ui.painter().rect_filled(row_rect, ROW_CORNER_RADIUS, bg);
            if !is_renaming {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
        }

        let line_height = ui.text_style_height(&egui::TextStyle::Body);
        let font_size = ui
            .ctx()
            .style()
            .text_styles
            .get(&egui::TextStyle::Body)
            .map(|f| f.size)
            .unwrap_or(14.0);
        let secondary_font_size = (font_size - 1.0).max(10.0);

        // ── Name cell (icon + label / textedit) ────────────────────────────
        let rename_id = egui::Id::new("landing_rename");
        let rename_submitted =
            is_renaming && GlyphonTextEdit::process_events(ui, rename_id, &mut ex.rename_buffer);

        let inner_resp = ui
            .scope_builder(UiBuilder::new().max_rect(rects.name), |ui| {
                ui.horizontal(|ui| -> egui::Response {
                    let doc_type = DocType::from_name(if is_renaming {
                        &ex.rename_buffer
                    } else {
                        &child.name
                    });

                    let is_pending = files.shared.contains_key(&child.id);
                    let is_shared = is_pending || !child.shares.is_empty();
                    let theme = ui.ctx().get_lb_theme();
                    if child.is_folder() {
                        let folder_icon =
                            if is_shared { Icon::SHARED_FOLDER } else { Icon::FOLDER };
                        let color = if is_shared {
                            theme.fg().get_color(theme.prefs().secondary)
                        } else {
                            theme.fg().get_color(theme.prefs().primary)
                        };
                        ui.label(
                            RichText::new(folder_icon.icon)
                                .font(FontId::monospace(19.0))
                                .color(color),
                        )
                        .on_hover_ui(|ui| {
                            ui.label("Folder");
                        });
                    } else {
                        ui.label(
                            RichText::new(doc_type.to_icon().icon)
                                .font(FontId::monospace(19.0))
                                .color(ui.visuals().weak_text_color()),
                        )
                        .on_hover_ui(|ui| {
                            ui.label(format!("{doc_type}"));
                        });
                    }

                    if is_renaming {
                        let stem_end = ex
                            .rename_buffer
                            .rfind('.')
                            .unwrap_or(ex.rename_buffer.len());
                        let rename_w =
                            GlyphonLabel::new(&ex.rename_buffer, egui::Color32::default())
                                .font_size(font_size)
                                .line_height(line_height)
                                .measure(ui)
                                .x;
                        let text_width = rename_w.max(ui.available_width());
                        let (text_rect, _) = ui.allocate_exact_size(
                            egui::vec2(text_width, line_height),
                            egui::Sense::hover(),
                        );
                        ui.place(
                            text_rect,
                            GlyphonTextEdit::new(&mut ex.rename_buffer)
                                .id(rename_id)
                                .font_size(font_size)
                                .line_height(line_height)
                                .select_on_focus(0, stem_end),
                        )
                    } else {
                        let display_name = doc_type.display_name(&child.name);
                        ui.add(
                            GlyphonLabel::new(display_name, ui.visuals().text_color())
                                .font_size(font_size)
                                .line_height(line_height)
                                .max_width(ui.available_width())
                                .sense(Sense::click()),
                        )
                    }
                })
                .inner
            })
            .inner;

        // Rename completion. Open / hover tooltip / context menu all
        // live on the row response below.
        if is_renaming {
            if !inner_resp.has_focus() && !inner_resp.lost_focus() {
                ui.memory_mut(|m| m.request_focus(rename_id));
            }
            if rename_submitted {
                response.rename_request = Some((child.id, ex.rename_buffer.clone()));
                ex.rename_target = None;
            } else if inner_resp.lost_focus() {
                ex.rename_target = None;
            }
        }

        // ── Modified cell ──────────────────────────────────────────────────
        if let Some(modified_rect) = rects.modified {
            ui.scope_builder(UiBuilder::new().max_rect(modified_rect), |ui| {
                let last_modified_timestamp = files.last_modified_recursive(child.id);
                let formatted_date = {
                    let system_time = std::time::UNIX_EPOCH
                        + std::time::Duration::from_millis(last_modified_timestamp);
                    let datetime: chrono::DateTime<chrono::Local> = system_time.into();
                    datetime.format("%B %d, %Y at %I:%M %p").to_string()
                };
                let weak = ui.visuals().weak_text_color();
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(last_modified_timestamp.elapsed_human_string())
                                .size(secondary_font_size)
                                .color(weak),
                        )
                        .selectable(false),
                    )
                    .on_hover_text(&formatted_date);

                    let mut last_modified_by = files.last_modified_by_recursive(child.id);
                    if last_modified_by == account.username {
                        last_modified_by = "you";
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("by {}", last_modified_by))
                                .size(secondary_font_size)
                                .color(weak),
                        )
                        .selectable(false),
                    );
                });
            });
        }

        // ── Collaborators cell ─────────────────────────────────────────────
        if let Some(collab_rect) = rects.collab {
            ui.scope_builder(UiBuilder::new().max_rect(collab_rect), |ui| {
                let share_count = child.shares.len();
                let header_font = FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold")));
                let weak = ui.visuals().weak_text_color();
                let label_response = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if share_count > 0 {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(share_count.to_string())
                                        .size(secondary_font_size)
                                        .color(weak),
                                )
                                .selectable(false),
                            )
                            .union(
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(Icon::ACCOUNT.icon)
                                            .font(FontId::monospace(16.0))
                                            .color(weak),
                                    )
                                    .selectable(false),
                                ),
                            )
                        } else {
                            ui.add(
                                egui::Label::new(
                                    RichText::new("-").size(secondary_font_size).color(weak),
                                )
                                .selectable(false),
                            )
                            .union(
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(Icon::ACCOUNT.icon)
                                            .font(FontId::monospace(16.0))
                                            .color(Color32::TRANSPARENT),
                                    )
                                    .selectable(false),
                                ),
                            )
                        }
                    })
                    .inner;

                label_response.on_hover_ui(|ui| {
                    ui.allocate_space(Vec2::X * 100.);
                    ui.vertical(|ui| {
                        let mut write_shares = Vec::new();
                        let mut read_shares = Vec::new();
                        for share in &child.shares {
                            match share.mode {
                                ShareMode::Write => write_shares.push(share),
                                ShareMode::Read => read_shares.push(share),
                            }
                        }
                        write_shares.sort_by_key(|s| &s.shared_with);
                        read_shares.sort_by_key(|s| &s.shared_with);

                        ui.style_mut().visuals.indent_has_left_vline = false;

                        ui.label(RichText::new("Owner").font(header_font.clone()).weak());
                        ui.indent("owner", |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&child.owner));
                                if child.owner == account.username {
                                    ui.label(RichText::new("(you)").weak());
                                }
                            });
                        });

                        if !write_shares.is_empty() {
                            ui.add_space(10.);
                            ui.label(RichText::new("Write").font(header_font.clone()).weak());
                            ui.indent("write_shares", |ui| {
                                for share in write_shares {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&share.shared_with));
                                        if share.shared_with == account.username {
                                            ui.label(RichText::new("(you)").weak());
                                        }
                                    });
                                }
                            });
                        }

                        if !read_shares.is_empty() {
                            ui.add_space(10.);
                            ui.label(RichText::new("Read").font(header_font.clone()).weak());
                            ui.indent("read_shares", |ui| {
                                for share in read_shares {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&share.shared_with));
                                        if share.shared_with == account.username {
                                            ui.label(RichText::new("(you)").weak());
                                        }
                                    });
                                }
                            });
                        }
                    });
                });
            });
        }

        // ── Size cell ──────────────────────────────────────────────────────
        if let Some(size_rect) = rects.size {
            ui.scope_builder(UiBuilder::new().max_rect(size_rect), |ui| {
                let weak = ui.visuals().weak_text_color();
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(bytes_to_human(
                                files
                                    .size_bytes_recursive
                                    .get(&child.id)
                                    .copied()
                                    .unwrap_or_default(),
                            ))
                            .size(secondary_font_size)
                            .color(weak),
                        )
                        .selectable(false),
                    );
                });
            });
        }

        // ── Usage bar ──────────────────────────────────────────────────────
        if let Some(bar_rect) = rects.usage_bar {
            // Inset vertically so the bar is a slim accent inside the row,
            // not a full-height block.
            let bar_h = (bar_rect.height() * 0.4).min(8.0);
            let inner =
                Rect::from_center_size(bar_rect.center(), Vec2::new(bar_rect.width(), bar_h));
            let usage = files
                .size_bytes_recursive
                .get(&child.id)
                .copied()
                .unwrap_or_default() as f32;
            let target_w = inner.width() * (usage / max_usage);
            let filled = Rect::from_min_size(inner.min, Vec2::new(target_w, inner.height()));
            let theme = ui.ctx().get_lb_theme();
            let track = ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.4);
            ui.painter().rect_filled(inner, 2.0, track);
            ui.painter()
                .rect_filled(filled, 2.0, theme.fg().blue.gamma_multiply(0.7));
        }

        // Row interact registered last → top of z-order, so it captures
        // clicks even on cells whose hover-only labels would otherwise
        // absorb the click. Skipped during rename so the text edit
        // continues to receive input.
        if !is_renaming {
            let id = egui::Id::new("landing_row").with(child.id);
            let row_resp = ui.interact(row_rect, id, Sense::click());
            if row_resp.clicked() && response.open_file.is_none() {
                response.open_file = Some(child.id);
            }
            row_resp
                .on_hover_ui({
                    let theme = ui.ctx().get_lb_theme();
                    let segments = files.path_segments(child.id);
                    let share_color = theme.fg().get_color(theme.prefs().secondary);
                    let normal_color = ui.visuals().text_color();
                    move |ui: &mut egui::Ui| {
                        let colored_spans: Vec<(&str, Option<egui::Color32>)> = segments
                            .iter()
                            .map(|(text, shared)| {
                                let color = if *shared { Some(share_color) } else { None };
                                (text.as_str(), color)
                            })
                            .collect();
                        ui.add(GlyphonLabel::new_colored(colored_spans, normal_color));
                    }
                })
                .context_menu(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);
                    if !child.is_folder() && ui.button("Open").clicked() {
                        response.open_file = Some(child.id);
                        ui.close();
                    }
                    if !child.is_folder() {
                        ui.separator();
                    }
                    if ui.button("Rename").clicked() {
                        ex.rename_target = Some(child.id);
                        ex.rename_buffer = child.name.clone();
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        response.delete_request = Some(child.id);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("New Document").clicked() {
                        response.create_note = true;
                        ui.close();
                    }
                    if ui.button("New Drawing").clicked() {
                        response.create_drawing = true;
                        ui.close();
                    }
                    if ui.button("New Folder").clicked() {
                        response.create_folder = true;
                        ui.close();
                    }
                });
        }
    }
}
