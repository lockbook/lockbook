use egui::{
    Align, Button, Color32, Direction, FontId, Frame, Key, KeyboardShortcut, Layout, Modifiers,
    Rect, RichText, Stroke, Ui, Vec2,
};
use lb_rs::Uuid;
use lb_rs::model::account::Account;
use lb_rs::model::file::{File, ShareMode};
use lb_rs::model::usage::bytes_to_human;
use serde::{Deserialize, Serialize};
use std::f32;
use std::ops::BitOrAssign;
use std::sync::Arc;
use std::time::Duration;

use crate::file_cache::{FileCache, FilesExt};
use crate::show::{DocType, ElapsedHumanString as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;
use crate::workspace::Workspace;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPage {
    search_term: String,
    doc_types: Vec<DocType>,
    collaborators: Vec<String>,
    only_me: bool,
    sort: Sort,
    sort_asc: bool,
    flatten_tree: bool,
}

impl Default for LandingPage {
    fn default() -> Self {
        Self {
            search_term: Default::default(),
            doc_types: Default::default(),
            collaborators: Default::default(),
            only_me: Default::default(),
            sort: Default::default(),
            sort_asc: Default::default(),
            flatten_tree: true,
        }
    }
}

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
enum Sort {
    Name,
    Type,
    #[default]
    Modified,
    Size,
}

#[derive(Default)]
pub struct Response {
    pub open_file: Option<Uuid>,
    pub create_note: bool,
    pub create_drawing: bool,
    pub create_folder: bool,
}

impl BitOrAssign for Response {
    fn bitor_assign(&mut self, rhs: Self) {
        self.open_file = self.open_file.or(rhs.open_file);
        self.create_note |= rhs.create_note;
        self.create_drawing |= rhs.create_drawing;
        self.create_folder |= rhs.create_folder;
    }
}

impl Workspace {
    pub fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let initial_landing_page = self.landing_page.clone();

        const MARGIN: f32 = 45.0;
        const MAX_WIDTH: f32 = 1000.0;

        let width = ui.max_rect().width().min(MAX_WIDTH) - 2. * MARGIN;
        let height = ui.available_size().y - 2. * MARGIN;

        let mut response = Response::default();

        ui.vertical_centered_justified(|ui| {
            Frame::canvas(ui.style())
                .inner_margin(MARGIN)
                .stroke(Stroke::NONE)
                .fill(Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });

                    let padding = (ui.available_width() - width) / 2.;
                    let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        response |= self.show_heading(ui);
                        ui.add_space(40.0);
                        response |= self.show_filters(ui);
                        ui.add_space(40.0);
                        response |= self.show_files(ui);
                    });
                });
        });

        let Some(files) = &self.files else {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
            return;
        };

        if let Some(id) = response.open_file {
            if files.files.get_by_id(id).unwrap().is_document() {
                self.open_file(id, true, false);
            } else {
                self.focused_parent = Some(id);
                self.out.selected_file = Some(id)
            }
        }
        if response.create_note {
            self.create_doc(false);
        }
        if response.create_drawing {
            self.create_doc(true);
        }
        if response.create_folder {
            self.create_folder();
        }

        // Persist landing page if it changed
        if self.landing_page != initial_landing_page {
            self.cfg.set_landing_page(self.landing_page.clone());
        }
    }

    /// "Welcome, <Username>" or selected folder with breadcrumb for parent
    fn show_heading(&mut self, ui: &mut egui::Ui) -> Response {
        let mut response = Response::default();

        let Some(files) = &self.files else {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
            return response;
        };
        let folder = files
            .files
            .get_by_id(self.effective_focused_parent())
            .unwrap();

        ui.style_mut().visuals.hyperlink_color = ui.visuals().text_color();
        ui.vertical(|ui| {
            if folder.id == files.files.root().id {
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
                if !folder.is_root() {
                    let parent = files.files.get_by_id(folder.parent).unwrap();
                    if ui
                        .link(RichText::new(&parent.name).font(FontId::proportional(40.0)))
                        .clicked()
                    {
                        response.open_file = Some(parent.id);
                    }
                    ui.label(
                        RichText::new(Icon::CHEVRON_RIGHT.icon)
                            .font(FontId::monospace(19.0))
                            .weak(),
                    );
                }
                ui.label(RichText::new(&folder.name).font(FontId::proportional(40.0)));
            });
        });

        response
    }

    /// Create button, filter text box, show folders toggle, file types selector, collaborators selector
    fn show_filters(&mut self, ui: &mut egui::Ui) -> Response {
        let mut response = Response::default();

        let (Some(files), Some(account)) = (&self.files, &self.account) else {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
            return response;
        };
        let folder = files
            .files
            .get_by_id(self.effective_focused_parent())
            .unwrap();

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

                egui::ComboBox::from_id_source("create_combo")
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
                                    ui.close_menu();
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
                                    ui.close_menu();
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
                                    ui.close_menu();
                                }
                            },
                        );
                    });
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                // Collaborators filter
                egui::ComboBox::from_id_source(ui.next_auto_id())
                    .selected_text(if self.landing_page.only_me {
                        "Collaborators: Only Me".to_string()
                    } else if self.landing_page.collaborators.is_empty() {
                        "Collaborators: Any".to_string()
                    } else {
                        format!("Collaborators: {}", self.landing_page.collaborators.len())
                    })
                    .width(180.)
                    .height(f32::INFINITY)
                    .show_ui(ui, |ui| {
                        // Collect all unique collaborators from files in scope
                        let files_in_scope = if self.landing_page.flatten_tree {
                            files.files.descendents(folder.id)
                        } else {
                            files.files.children(folder.id)
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
                            self.landing_page.only_me = false;
                            self.landing_page.collaborators.clear();
                        }
                        ui.add_space(5.);
                        if ui.button("Only Me").clicked() {
                            self.landing_page.only_me = true;
                            self.landing_page.collaborators.clear();
                        }

                        for collaborator in &collaborators_list {
                            let mut is_selected = self
                                .landing_page
                                .collaborators
                                .iter()
                                .any(|c| c == collaborator);

                            if ui.checkbox(&mut is_selected, collaborator).changed() {
                                if is_selected {
                                    // Add the collaborator if not present
                                    if !self
                                        .landing_page
                                        .collaborators
                                        .iter()
                                        .any(|c| c == collaborator)
                                    {
                                        self.landing_page.collaborators.push(collaborator.clone());
                                    }
                                } else {
                                    // Remove the collaborator
                                    self.landing_page
                                        .collaborators
                                        .retain(|c| c != collaborator);
                                }

                                self.landing_page.only_me = false;
                            }
                        }
                    });

                // File type filter
                egui::ComboBox::from_id_source(ui.next_auto_id())
                    .selected_text(if self.landing_page.doc_types.is_empty() {
                        "Types: Any".to_string()
                    } else {
                        format!("Types: {}", self.landing_page.doc_types.len())
                    })
                    .width(100.)
                    .height(f32::INFINITY)
                    .show_ui(ui, |ui| {
                        ui.style_mut().spacing.button_padding.y = 5.0;
                        if ui.button("Any").clicked() {
                            self.landing_page.doc_types.clear();
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
                            let mut is_selected =
                                self.landing_page.doc_types.iter().any(|dt| dt == doc_type);

                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut is_selected, "").changed() {
                                    if is_selected {
                                        // Add the doc type if not present
                                        if !self
                                            .landing_page
                                            .doc_types
                                            .iter()
                                            .any(|dt| dt == doc_type)
                                        {
                                            self.landing_page.doc_types.push(*doc_type);
                                        }
                                    } else {
                                        // Remove the doc type
                                        self.landing_page.doc_types.retain(|&t| t != *doc_type);
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
                    if self.landing_page.flatten_tree { Icon::FOLDER_OPEN } else { Icon::FOLDER }
                        .size(22.),
                )
                .show(ui)
                .clicked()
                {
                    self.landing_page.flatten_tree = !self.landing_page.flatten_tree;
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
                        );

                        ui.add_space(15.0); // margin

                        ui.label(
                            RichText::new(Icon::FILTER.icon)
                                .font(FontId::monospace(19.0))
                                .color(ui.visuals().weak_text_color()),
                        );

                        // Check for Cmd+F (or Ctrl+F on non-Mac)
                        let cmd_f = ui.input_mut(|i| {
                            i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F))
                        });

                        let response =
                            egui::TextEdit::singleline(&mut self.landing_page.search_term)
                                .hint_text(if folder.is_root() {
                                    "Filter".to_string()
                                } else {
                                    format!("Filter in {}", &folder.name)
                                })
                                .frame(false)
                                .margin(Vec2::ZERO)
                                .desired_width(
                                    ui.available_width()
                                    - 25.0 // space for 'x'
                                    - 15.,
                                ) // margin
                                .show(ui)
                                .response;

                        // Focus when Cmd+F is pressed
                        if cmd_f {
                            response.request_focus();
                        }

                        // Clear button (X icon) when there's text
                        #[allow(clippy::collapsible_if)]
                        if !self.landing_page.search_term.is_empty() {
                            if IconButton::new(Icon::CLOSE.size(16.)).show(ui).clicked() {
                                self.landing_page.search_term.clear();
                            }
                        }
                    },
                );
            });
        });

        response
    }

    fn filtered_sorted_files<'a>(&self, files: &'a FileCache, account: &Account) -> Vec<&'a File> {
        let folder = files
            .files
            .get_by_id(self.effective_focused_parent())
            .unwrap();

        // Filter
        let descendents = if self.landing_page.flatten_tree {
            files.files.descendents(folder.id)
        } else {
            files.files.children(folder.id)
        };
        let mut descendents: Vec<_> = descendents
            .into_iter()
            .filter(|child| {
                // Search term filter
                let search_match = if self.landing_page.search_term.is_empty() {
                    true
                } else {
                    child
                        .name
                        .to_lowercase()
                        .contains(&self.landing_page.search_term.to_lowercase())
                };

                // Flatten tree filter - hide folders when flattening
                let flatten_match = !self.landing_page.flatten_tree || child.is_document();

                // Doc type filter
                let doc_type_match = if self.landing_page.doc_types.is_empty() {
                    true
                } else if child.is_folder() {
                    // hide folders
                    false
                } else {
                    let child_doc_type = DocType::from_name(&child.name);
                    self.landing_page
                        .doc_types
                        .iter()
                        .any(|filter_type| filter_type == &child_doc_type)
                };

                // Collaborator filter
                let collaborator_match = if self.landing_page.only_me {
                    // Only show files where the current user is the only collaborator
                    child.shares.is_empty()
                } else if self.landing_page.collaborators.is_empty() {
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
                    self.landing_page
                        .collaborators
                        .iter()
                        .all(|filter_collab| file_collaborators.contains(&filter_collab))
                };

                search_match && doc_type_match && flatten_match && collaborator_match
            })
            .collect();

        // Sort
        match self.landing_page.sort {
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
            Sort::Size => descendents.sort_by_key(|f| u64::MAX - files.size_bytes_recursive[&f.id]),
        }
        if !self.landing_page.sort_asc {
            descendents.reverse()
        }

        descendents
    }

    /// Files table with columns that you click to select a sort key
    fn show_files(&mut self, ui: &mut egui::Ui) -> Response {
        let mut response = Response::default();

        let (Some(files), Some(account)) = (&self.files, &self.account) else {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
            return response;
        };
        let descendents = self.filtered_sorted_files(files, account);

        // Show
        if !descendents.is_empty() {
            ui.ctx().style_mut(|style| {
                style.spacing.scroll = egui::style::ScrollStyle::thin();
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.allocate_space(ui.available_width() * Vec2::X);
                egui::Grid::new("files_grid")
                    .num_columns(6)
                    .spacing([40.0, 10.0])
                    .show(ui, |ui| {
                        let header_font =
                            FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold")));

                        // Header: Name / Type
                        ui.horizontal(|ui| {
                            let text =
                                if self.landing_page.sort == Sort::Type { "Type" } else { "Name" };
                            if ui
                                .add(
                                    Button::new(RichText::new(text).font(header_font.clone()))
                                        .frame(false),
                                )
                                .clicked()
                            {
                                // Click to cycle through sorting by name or type (inspired by Spotify 'Title' / 'Artist' sort)
                                match (&self.landing_page.sort, self.landing_page.sort_asc) {
                                    (Sort::Name, true) => {
                                        // name asc -> name desc
                                        self.landing_page.sort_asc = false;
                                    }
                                    (Sort::Name, false) => {
                                        // name desc -> type asc
                                        self.landing_page.sort = Sort::Type;
                                        self.landing_page.sort_asc = true;
                                    }
                                    (Sort::Type, true) => {
                                        // type asc -> type desc
                                        self.landing_page.sort_asc = false;
                                    }
                                    _ => {
                                        // type desc (or anything else) -> name asc
                                        self.landing_page.sort = Sort::Name;
                                        self.landing_page.sort_asc = true;
                                    }
                                }
                            }
                            if matches!(self.landing_page.sort, Sort::Name | Sort::Type) {
                                let chevron = if self.landing_page.sort_asc {
                                    Icon::CHEVRON_UP
                                } else {
                                    Icon::CHEVRON_DOWN
                                };
                                ui.label(RichText::new(chevron.icon).font(FontId::monospace(12.0)));
                            }
                        });

                        // Header: Modified
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new("Modified").font(header_font.clone()),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                if self.landing_page.sort == Sort::Modified {
                                    self.landing_page.sort_asc = !self.landing_page.sort_asc;
                                } else {
                                    self.landing_page.sort = Sort::Modified;
                                    self.landing_page.sort_asc = true;
                                }
                            }
                            if self.landing_page.sort == Sort::Modified {
                                let chevron = if self.landing_page.sort_asc {
                                    Icon::CHEVRON_UP
                                } else {
                                    Icon::CHEVRON_DOWN
                                };
                                ui.label(RichText::new(chevron.icon).font(FontId::monospace(12.0)));
                            }
                        });

                        // Header: Collaborators
                        ui.label(RichText::new("Collaborators").font(header_font.clone()));

                        // Header: Usage
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    Button::new(RichText::new("Size").font(header_font.clone()))
                                        .frame(false),
                                )
                                .clicked()
                            {
                                if self.landing_page.sort == Sort::Size {
                                    self.landing_page.sort_asc = !self.landing_page.sort_asc;
                                } else {
                                    self.landing_page.sort = Sort::Size;
                                    self.landing_page.sort_asc = true;
                                }
                            }
                            if self.landing_page.sort == Sort::Size {
                                let chevron = if self.landing_page.sort_asc {
                                    Icon::CHEVRON_UP
                                } else {
                                    Icon::CHEVRON_DOWN
                                };
                                ui.label(RichText::new(chevron.icon).font(FontId::monospace(12.0)));
                            }
                        });

                        // Header: Usage (Bar Chart)
                        ui.label("");

                        ui.end_row();

                        let mut current_time_category = "";
                        let mut child_idx = 0;
                        while child_idx < descendents.len() {
                            let child = descendents[child_idx];

                            // Check if we need to insert a time separator (only when sorting by modified)
                            if self.landing_page.sort == Sort::Modified {
                                let current_modified = files.last_modified_recursive(child.id);
                                let now = lb_rs::model::clock::get_time().0;
                                let current_time_diff = now - current_modified as i64;

                                let get_time_category = |millis: i64| -> &str {
                                    let day = 24 * 60 * 60 * 1000;
                                    if millis <= day {
                                        "Today"
                                    } else if millis <= 2 * day {
                                        "Yesterday"
                                    } else if millis <= 7 * day {
                                        "This Week"
                                    } else if millis <= 30 * day {
                                        "This Month"
                                    } else if millis <= 365 * day {
                                        "This Year"
                                    } else {
                                        "All Time"
                                    }
                                };

                                let new_category = get_time_category(current_time_diff);

                                if new_category != current_time_category {
                                    current_time_category = new_category;

                                    ui.vertical(|ui| {
                                        if !current_time_category.is_empty() {
                                            ui.add_space(10.);
                                        }
                                        ui.horizontal(|ui| {
                                            ui.add_space(-20.);
                                            ui.label(
                                                RichText::new(current_time_category)
                                                    .font(FontId::new(
                                                        16.0,
                                                        egui::FontFamily::Name(Arc::from("Bold")),
                                                    ))
                                                    .weak(),
                                            );
                                        });
                                    });
                                    ui.label("");
                                    ui.end_row();
                                }
                            }

                            child_idx += 1;

                            // File name
                            ui.horizontal(|ui| {
                                // Icon
                                if child.is_folder() {
                                    let folder_icon = if !child.shares.is_empty() {
                                        Icon::SHARED_FOLDER
                                    } else {
                                        Icon::FOLDER
                                    };
                                    ui.label(
                                        RichText::new(folder_icon.icon)
                                            .font(FontId::monospace(19.0))
                                            .color(ui.style().visuals.widgets.active.bg_fill),
                                    )
                                    .on_hover_ui(|ui| {
                                        ui.label("Folder");
                                    });
                                } else {
                                    let doc_type = DocType::from_name(&child.name);
                                    ui.label(
                                        RichText::new(doc_type.to_icon().icon)
                                            .font(FontId::monospace(19.0))
                                            .color(ui.visuals().weak_text_color()),
                                    )
                                    .on_hover_ui(|ui| {
                                        ui.label(format!("{doc_type}"));
                                    });
                                }

                                let doc_type = DocType::from_name(&child.name);
                                let text = if doc_type.hide_ext() {
                                    let wo = std::path::Path::new(&child.name)
                                        .file_stem()
                                        .map(|stem| stem.to_str().unwrap())
                                        .unwrap_or(&child.name);
                                    egui::WidgetText::from(wo)
                                } else {
                                    egui::WidgetText::from(&child.name)
                                };
                                let link_response = ui.link(text);
                                if link_response.clicked() {
                                    response.open_file = Some(child.id);
                                }

                                // Show full path on hover
                                link_response.on_hover_ui(|ui| {
                                    ui.label(self.core.get_path_by_id(child.id).unwrap());
                                });
                            });

                            // Last modified
                            {
                                let last_modified_timestamp =
                                    files.last_modified_recursive(child.id);
                                let formatted_date = {
                                    let system_time = std::time::UNIX_EPOCH
                                        + std::time::Duration::from_millis(last_modified_timestamp);
                                    let datetime: chrono::DateTime<chrono::Local> =
                                        system_time.into();
                                    datetime.format("%B %d, %Y at %I:%M %p").to_string()
                                };

                                ui.horizontal(|ui: &mut Ui| {
                                    ui.label(RichText::new(
                                        last_modified_timestamp.elapsed_human_string(),
                                    ))
                                    .on_hover_text(&formatted_date);

                                    let mut last_modified_by =
                                        files.last_modified_by_recursive(child.id);
                                    if last_modified_by == account.username {
                                        last_modified_by = "you";
                                    }
                                    ui.label(
                                        RichText::new(format!("by {}", last_modified_by)).weak(),
                                    );
                                });
                            }

                            // Collaborators
                            ui.horizontal(|ui| {
                                let share_count = child.shares.len();
                                let label_response = if share_count > 0 {
                                    ui.allocate_ui_with_layout(
                                        ui.available_size_before_wrap(),
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui.label(RichText::new(share_count.to_string())).union(
                                                ui.label(
                                                    RichText::new(Icon::ACCOUNT.icon)
                                                        .font(FontId::monospace(16.0))
                                                        .color(ui.visuals().weak_text_color()),
                                                ),
                                            )
                                        },
                                    )
                                    .inner
                                } else {
                                    ui.allocate_ui_with_layout(
                                        ui.available_size_before_wrap(),
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui.label(RichText::new("-").weak()).union(
                                                ui.label(
                                                    RichText::new(Icon::ACCOUNT.icon)
                                                        .font(FontId::monospace(16.0))
                                                        .color(Color32::TRANSPARENT),
                                                ),
                                            )
                                        },
                                    )
                                    .inner
                                };

                                // Show collaborators on hover
                                label_response.on_hover_ui(|ui| {
                                    ui.allocate_space(Vec2::X * 100.);
                                    ui.vertical(|ui| {
                                        // Separate shares by access level
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

                                        // Show owner access
                                        ui.label(
                                            RichText::new("Owner").font(header_font.clone()).weak(),
                                        );
                                        ui.indent("owner", |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&child.owner));
                                                if child.owner == account.username {
                                                    ui.label(RichText::new("(you)").weak());
                                                }
                                            });
                                        });

                                        // Show write access
                                        if !write_shares.is_empty() {
                                            ui.add_space(10.);
                                            ui.label(
                                                RichText::new("Write")
                                                    .font(header_font.clone())
                                                    .weak(),
                                            );
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

                                        // Show read access
                                        if !read_shares.is_empty() {
                                            ui.add_space(10.);
                                            ui.label(
                                                RichText::new("Read")
                                                    .font(header_font.clone())
                                                    .weak(),
                                            );
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

                            // Usage
                            ui.label(RichText::new({
                                bytes_to_human(files.size_bytes_recursive[&child.id] as _)
                            }));

                            // Usage bar chart
                            let scroll_area_right = ui.max_rect().max.x;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;

                                let (_, rect) = ui.allocate_space(ui.available_height() * Vec2::Y);
                                let cell_left = rect.min.x;

                                let available_width = scroll_area_right - cell_left;
                                let mut rect = Rect::from_min_size(
                                    rect.min,
                                    Vec2::new(available_width, ui.available_height()),
                                );

                                let target_width = rect.width()
                                    * files.usage_portion_scaled(child.id, &descendents);
                                let excess_width = rect.width() - target_width;
                                rect.max.x -= excess_width;

                                ui.painter().rect_filled(
                                    rect,
                                    2.0,
                                    ui.visuals().widgets.active.bg_fill.gamma_multiply(0.8),
                                );
                            });

                            ui.end_row();
                        }
                    });
            });
        } else if !self.landing_page.search_term.is_empty() {
            ui.label(
                RichText::new("No files found matching your search.")
                    .color(ui.visuals().weak_text_color()),
            );
        }

        response
    }
}
