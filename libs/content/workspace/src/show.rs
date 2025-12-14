use basic_human_duration::ChronoHumanDuration;
use egui::os::OperatingSystem;
use egui::{
    Align, Align2, Button, Color32, Direction, DragAndDrop, EventFilter, FontId, Frame, Galley, Id,
    Image, Key, KeyboardShortcut, LayerId, Layout, Modifiers, Order, Rangef, Rect, RichText, Sense,
    Stroke, TextStyle, TextWrapMode, Ui, Vec2, ViewportCommand, include_image, vec2,
};
use lb_rs::model::usage::bytes_to_human;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{f32, mem};
use tracing::instrument;

use crate::file_cache::FilesExt;
use crate::output::Response;
use crate::tab::{ContentState, TabContent, TabStatus, core_get_by_relative_path, image_viewer};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;
use crate::workspace::Workspace;

#[derive(Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingPage {
    search_term: String,
    doc_types: Vec<DocType>,
    last_modified: Option<LastModifiedFilter>,
    sort: Sort,
    sort_asc: bool,
    flatten_tree: bool,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum LastModifiedFilter {
    Today,
    ThisWeek,
    ThisMonth,
    ThisYear,
}

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
enum Sort {
    Name,
    Type,
    #[default]
    Modified,
    Size,
}

impl Display for LastModifiedFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LastModifiedFilter::Today => write!(f, "Today"),
            LastModifiedFilter::ThisWeek => write!(f, "This Week"),
            LastModifiedFilter::ThisMonth => write!(f, "This Month"),
            LastModifiedFilter::ThisYear => write!(f, "This Year"),
        }
    }
}

impl Workspace {
    #[instrument(level="trace", skip_all, fields(frame = self.ctx.frame_nr()))]
    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        if self.ctx.input(|inp| !inp.raw.events.is_empty()) {
            self.user_last_seen = Instant::now();
        }

        self.set_tooltip_visibility(ui);

        self.process_lb_updates();
        self.process_task_updates();
        self.process_keys();
        self.status.message = self.status_message();

        if self.is_empty() {
            if self.show_tabs {
                self.show_landing_page(ui);
            } else {
                self.show_mobile_landing_page(ui);
            }
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(ui));
        }
        if self.out.tabs_changed || self.current_tab_changed {
            self.cfg.set_tabs(&self.tabs, self.current_tab);
        }

        mem::take(&mut self.out)
    }

    fn set_tooltip_visibility(&mut self, ui: &mut egui::Ui) {
        let has_touch = ui.input(|r| {
            r.events.iter().any(|e| {
                matches!(e, egui::Event::Touch { device_id: _, id: _, phase: _, pos: _, force: _ })
            })
        });
        if has_touch && self.last_touch_event.is_none() {
            self.last_touch_event = Some(Instant::now());
        }

        if let Some(last_touch_event) = self.last_touch_event {
            if Instant::now() - last_touch_event > Duration::from_secs(5) {
                self.ctx
                    .style_mut(|style| style.interaction.tooltip_delay = 0.0);
                self.last_touch_event = None;
            } else {
                self.ctx
                    .style_mut(|style| style.interaction.tooltip_delay = f32::MAX);
            }
        }
    }

    fn show_mobile_landing_page(&mut self, ui: &mut egui::Ui) {
        let punchout = if ui.visuals().dark_mode {
            include_image!("../punchout-dark.png")
        } else {
            include_image!("../punchout-light.png")
        };

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                let image_size = egui::vec2(200.0, 200.0);
                ui.add(Image::new(punchout).fit_to_exact_size(image_size));
                ui.add_space(120.0);

                ui.label(
                    RichText::new("TOOLS")
                        .small()
                        .weak()
                        .text_style(egui::TextStyle::Button),
                );
                ui.add_space(24.0);

                let is_beta = self
                    .core
                    .get_account()
                    .map(|a| a.is_beta())
                    .unwrap_or_default();
                if is_beta
                    && ui
                        .add_sized(
                            [200.0, 44.0],
                            egui::Button::new(RichText::new("Mind Map").size(18.0)),
                        )
                        .clicked()
                {
                    self.upsert_mind_map(self.core.clone());
                }
                ui.add_space(12.0);

                if ui
                    .add_sized(
                        [200.0, 44.0],
                        egui::Button::new(RichText::new("Space Inspector").size(18.0)),
                    )
                    .clicked()
                {
                    self.start_space_inspector(self.core.clone(), None);
                }
            });
        });
    }

    fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let initial_landing_page = self.landing_page.clone();

        let (Some(files), Some(account)) = (&self.files, &self.account) else {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
            return;
        };

        let folder = files
            .files
            .get_by_id(self.effective_focused_parent())
            .unwrap();

        const MARGIN: f32 = 45.0;
        const MAX_WIDTH: f32 = 1000.0;

        let width = ui.max_rect().width().min(MAX_WIDTH) - 2. * MARGIN;
        let height = ui.available_size().y - 2. * MARGIN;

        let mut open_file = None;
        let mut create_note = false;
        let mut create_drawing = false;
        let mut create_folder = false;

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
                                        .link(
                                            RichText::new(&parent.name)
                                                .font(FontId::proportional(40.0)),
                                        )
                                        .clicked()
                                    {
                                        open_file = Some(parent.id);
                                    }
                                    ui.label(
                                        RichText::new(Icon::CHEVRON_RIGHT.icon)
                                            .font(FontId::monospace(19.0))
                                            .weak(),
                                    );
                                }
                                ui.label(
                                    RichText::new(&folder.name).font(FontId::proportional(40.0)),
                                );
                            });

                            ui.add_space(40.0);

                            // Search box and filters
                            ui.horizontal_top(|ui| {
                                // Create button - dropdown for new items
                                let filters_height = 40.0;

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
                                            ui.visuals_mut().widgets.inactive.weak_bg_fill =
                                                Color32::TRANSPARENT;

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
                                                        create_note = true;
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
                                                        create_drawing = true;
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
                                                        create_folder = true;
                                                        ui.close_menu();
                                                    }
                                                },
                                            );
                                        });
                                });

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        // Last modified filter
                                        egui::ComboBox::from_id_source(ui.next_auto_id())
                                            .selected_text(
                                                if let Some(filter) =
                                                    &self.landing_page.last_modified
                                                {
                                                    format!("Modified: {}", filter)
                                                } else {
                                                    "Modified: Anytime".to_string()
                                                },
                                            )
                                            .width(180.)
                                            .height(f32::INFINITY)
                                            .show_ui(ui, |ui| {
                                                let last_modified_filters = [
                                                    LastModifiedFilter::Today,
                                                    LastModifiedFilter::ThisWeek,
                                                    LastModifiedFilter::ThisMonth,
                                                    LastModifiedFilter::ThisYear,
                                                ];

                                                for filter in &last_modified_filters {
                                                    let is_selected = self
                                                        .landing_page
                                                        .last_modified
                                                        .as_ref()
                                                        .map(|f| f == filter)
                                                        .unwrap_or(false);

                                                    if ui
                                                        .selectable_label(
                                                            is_selected,
                                                            filter.to_string(),
                                                        )
                                                        .clicked()
                                                    {
                                                        if is_selected {
                                                            self.landing_page.last_modified = None;
                                                        } else {
                                                            self.landing_page.last_modified =
                                                                Some(*filter);
                                                        }
                                                    }
                                                }

                                                if ui.button("Clear").clicked() {
                                                    self.landing_page.last_modified = None;
                                                }
                                            });

                                        // Document filter
                                        egui::ComboBox::from_id_source(ui.next_auto_id())
                                            .selected_text(
                                                if self.landing_page.doc_types.is_empty() {
                                                    "Types: All".to_string()
                                                } else {
                                                    format!(
                                                        "Types: {}",
                                                        self.landing_page.doc_types.len()
                                                    )
                                                },
                                            )
                                            .width(100.)
                                            .height(f32::INFINITY)
                                            .show_ui(ui, |ui| {
                                                let doc_types = [
                                                    DocType::Markdown,
                                                    DocType::SVG,
                                                    DocType::PlainText,
                                                    DocType::Code,
                                                    DocType::Image,
                                                    DocType::PDF,
                                                    DocType::Unknown,
                                                ];

                                                for doc_type in &doc_types {
                                                    let mut is_selected = self
                                                        .landing_page
                                                        .doc_types
                                                        .iter()
                                                        .any(|dt| dt == doc_type);

                                                    ui.horizontal(|ui| {
                                                        if ui
                                                            .checkbox(&mut is_selected, "")
                                                            .changed()
                                                        {
                                                            if is_selected {
                                                                // Add the doc type if not present
                                                                if !self
                                                                    .landing_page
                                                                    .doc_types
                                                                    .iter()
                                                                    .any(|dt| dt == doc_type)
                                                                {
                                                                    self.landing_page
                                                                        .doc_types
                                                                        .push(*doc_type);
                                                                }
                                                            } else {
                                                                // Remove the doc type
                                                                self.landing_page
                                                                    .doc_types
                                                                    .retain(|&t| t != *doc_type);
                                                            }
                                                        }

                                                        ui.label(
                                                            RichText::new(doc_type.to_icon().icon)
                                                                .font(FontId::monospace(16.0))
                                                                .color(
                                                                    ui.visuals().weak_text_color(),
                                                                ),
                                                        );
                                                        ui.label(doc_type.to_string());
                                                    });

                                                    ui.add_space(5.);
                                                }

                                                if ui.button("Clear").clicked() {
                                                    self.landing_page.doc_types.clear();
                                                }
                                            });

                                        // Flatten tree toggle
                                        if IconButton::new(
                                            if self.landing_page.flatten_tree {
                                                Icon::FOLDER_OPEN
                                            } else {
                                                Icon::FOLDER
                                            }
                                            .size(22.),
                                        )
                                        .show(ui)
                                        .clicked()
                                        {
                                            self.landing_page.flatten_tree =
                                                !self.landing_page.flatten_tree;
                                        }

                                        // Search box - takes remaining space
                                        Frame::none()
                                            .fill(ui.visuals().extreme_bg_color)
                                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                                            .rounding(filters_height / 2.0) // Make it capsule-shaped
                                            .inner_margin(egui::Margin::symmetric(15.0, 5.0))
                                            .show(ui, |ui| {
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(
                                                        ui.available_width(),
                                                        filters_height - 20.0,
                                                    ),
                                                    egui::Layout::left_to_right(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(Icon::FILTER.icon)
                                                                .font(FontId::monospace(19.0))
                                                                .color(
                                                                    ui.visuals().weak_text_color(),
                                                                ),
                                                        );

                                                        // Check for Cmd+F (or Ctrl+F on non-Mac)
                                                        let cmd_f = ui.input_mut(|i| {
                                                            i.consume_shortcut(
                                                                &KeyboardShortcut::new(
                                                                    Modifiers::COMMAND,
                                                                    Key::F,
                                                                ),
                                                            )
                                                        });

                                                        let search_term_is_empty = self
                                                            .landing_page
                                                            .search_term
                                                            .is_empty();
                                                        let search_edit =
                                                            egui::TextEdit::singleline(
                                                                &mut self.landing_page.search_term,
                                                            )
                                                            .hint_text(if folder.is_root() {
                                                                "Filter".to_string()
                                                            } else {
                                                                format!(
                                                                    "Filter in {}",
                                                                    &folder.name
                                                                )
                                                            })
                                                            .frame(false)
                                                            .margin(Vec2::ZERO);

                                                        let response = ui.add_sized(
                                                            [
                                                                ui.available_width()
                                                                    - if !search_term_is_empty {
                                                                        25.0
                                                                    } else {
                                                                        0.0
                                                                    },
                                                                ui.available_height(),
                                                            ],
                                                            search_edit,
                                                        );

                                                        // Focus when Cmd+F is pressed
                                                        if cmd_f {
                                                            response.request_focus();
                                                        }

                                                        // Clear button (X icon) when there's text
                                                        #[allow(clippy::collapsible_if)]
                                                        if !self.landing_page.search_term.is_empty()
                                                        {
                                                            if IconButton::new(
                                                                Icon::CLOSE.size(16.),
                                                            )
                                                            .show(ui)
                                                            .clicked()
                                                            {
                                                                self.landing_page
                                                                    .search_term
                                                                    .clear();
                                                            }
                                                        }
                                                    },
                                                );
                                            });
                                    },
                                );
                            });

                            ui.add_space(40.0);

                            // sort & filter
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

                                    // Last modified filter
                                    let last_modified_match = if let Some(filter) =
                                        &self.landing_page.last_modified
                                    {
                                        let now = lb_rs::model::clock::get_time().0;
                                        let file_modified = files.last_modified_recursive(child.id);
                                        let time_diff = now - file_modified as i64;

                                        match filter {
                                            LastModifiedFilter::Today => {
                                                time_diff <= 24 * 60 * 60 * 1000
                                            }
                                            LastModifiedFilter::ThisWeek => {
                                                time_diff <= 7 * 24 * 60 * 60 * 1000
                                            }
                                            LastModifiedFilter::ThisMonth => {
                                                time_diff <= 30 * 24 * 60 * 60 * 1000
                                            }
                                            LastModifiedFilter::ThisYear => {
                                                time_diff <= 365 * 24 * 60 * 60 * 1000
                                            }
                                        }
                                    } else {
                                        true
                                    };

                                    // Flatten tree filter - hide folders when flattening
                                    let flatten_match =
                                        !self.landing_page.flatten_tree || child.is_document();

                                    search_match
                                        && doc_type_match
                                        && last_modified_match
                                        && flatten_match
                                })
                                .collect();

                            // Sort
                            if self.landing_page.flatten_tree {
                                // When flattened, always sort by modified
                                descendents.sort_by_key(|f| {
                                    u64::MAX - files.last_modified_recursive(f.id)
                                });
                            } else {
                                match self.landing_page.sort {
                                    Sort::Name => descendents.sort_by(|a, b| {
                                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                    }),
                                    Sort::Type => {
                                        descendents.sort_by(|a, b| {
                                            // Folders first, then sort by document type
                                            match (a.is_folder(), b.is_folder()) {
                                                (true, false) => std::cmp::Ordering::Less,
                                                (false, true) => std::cmp::Ordering::Greater,
                                                (true, true) => a
                                                    .name
                                                    .to_lowercase()
                                                    .cmp(&b.name.to_lowercase()),
                                                (false, false) => {
                                                    let a_type = DocType::from_name(&a.name);
                                                    let b_type = DocType::from_name(&b.name);
                                                    a_type
                                                        .to_string()
                                                        .cmp(&b_type.to_string())
                                                        .then_with(|| {
                                                            a.name
                                                                .to_lowercase()
                                                                .cmp(&b.name.to_lowercase())
                                                        })
                                                }
                                            }
                                        })
                                    }
                                    Sort::Modified => descendents.sort_by_key(|f| {
                                        u64::MAX - files.last_modified_recursive(f.id)
                                    }),
                                    Sort::Size => descendents.sort_by_key(|f| {
                                        u64::MAX - files.size_bytes_recursive(f.id)
                                    }),
                                }
                            }
                            if !self.landing_page.sort_asc {
                                descendents.reverse()
                            }

                            // Files table
                            if !descendents.is_empty() {
                                ui.ctx().style_mut(|style| {
                                    style.spacing.scroll = egui::style::ScrollStyle::thin();
                                });
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    egui::Grid::new("files_grid")
                                        .num_columns(if self.landing_page.flatten_tree {
                                            2
                                        } else {
                                            5
                                        })
                                        .spacing([40.0, 10.0])
                                        .show(ui, |ui| {
                                            // Header
                                            let header_font = FontId::new(
                                                16.0,
                                                egui::FontFamily::Name(Arc::from("Bold")),
                                            );
                                            if !self.landing_page.flatten_tree {
                                                // Header: Name
                                                ui.horizontal(|ui| {
                                                    ui.add_space(20.0);
                                                    ui.horizontal(|ui| {
                                                        if ui
                                                            .add(
                                                                Button::new(
                                                                    RichText::new("Name")
                                                                        .font(header_font.clone())
                                                                        .weak(),
                                                                )
                                                                .frame(false),
                                                            )
                                                            .clicked()
                                                        {
                                                            if self.landing_page.sort == Sort::Name
                                                            {
                                                                self.landing_page.sort_asc =
                                                                    !self.landing_page.sort_asc;
                                                            } else {
                                                                self.landing_page.sort = Sort::Name;
                                                                self.landing_page.sort_asc = true;
                                                            }
                                                        }
                                                        if self.landing_page.sort == Sort::Name {
                                                            let chevron =
                                                                if self.landing_page.sort_asc {
                                                                    Icon::CHEVRON_UP
                                                                } else {
                                                                    Icon::CHEVRON_DOWN
                                                                };
                                                            ui.label(
                                                                RichText::new(chevron.icon)
                                                                    .font(FontId::monospace(12.0))
                                                                    .weak(),
                                                            );
                                                        }
                                                    });
                                                });

                                                // Header: Type
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .add(
                                                            Button::new(
                                                                RichText::new("Type")
                                                                    .font(header_font.clone())
                                                                    .weak(),
                                                            )
                                                            .frame(false),
                                                        )
                                                        .clicked()
                                                    {
                                                        if self.landing_page.sort == Sort::Type {
                                                            self.landing_page.sort_asc =
                                                                !self.landing_page.sort_asc;
                                                        } else {
                                                            self.landing_page.sort = Sort::Type;
                                                            self.landing_page.sort_asc = true;
                                                        }
                                                    }
                                                    if self.landing_page.sort == Sort::Type {
                                                        let chevron = if self.landing_page.sort_asc
                                                        {
                                                            Icon::CHEVRON_UP
                                                        } else {
                                                            Icon::CHEVRON_DOWN
                                                        };
                                                        ui.label(
                                                            RichText::new(chevron.icon)
                                                                .font(FontId::monospace(12.0))
                                                                .weak(),
                                                        );
                                                    }
                                                });

                                                // Header: Modified
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .add(
                                                            Button::new(
                                                                RichText::new("Modified")
                                                                    .font(header_font.clone())
                                                                    .weak(),
                                                            )
                                                            .frame(false),
                                                        )
                                                        .clicked()
                                                    {
                                                        if self.landing_page.sort == Sort::Modified
                                                        {
                                                            self.landing_page.sort_asc =
                                                                !self.landing_page.sort_asc;
                                                        } else {
                                                            self.landing_page.sort = Sort::Modified;
                                                            self.landing_page.sort_asc = true;
                                                        }
                                                    }
                                                    if self.landing_page.sort == Sort::Modified {
                                                        let chevron = if self.landing_page.sort_asc
                                                        {
                                                            Icon::CHEVRON_UP
                                                        } else {
                                                            Icon::CHEVRON_DOWN
                                                        };
                                                        ui.label(
                                                            RichText::new(chevron.icon)
                                                                .font(FontId::monospace(12.0))
                                                                .weak(),
                                                        );
                                                    }
                                                });

                                                // Header: Usage
                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .add(
                                                            Button::new(
                                                                RichText::new("Usage")
                                                                    .font(header_font)
                                                                    .weak(),
                                                            )
                                                            .frame(false),
                                                        )
                                                        .clicked()
                                                    {
                                                        if self.landing_page.sort == Sort::Size {
                                                            self.landing_page.sort_asc =
                                                                !self.landing_page.sort_asc;
                                                        } else {
                                                            self.landing_page.sort = Sort::Size;
                                                            self.landing_page.sort_asc = true;
                                                        }
                                                    }
                                                    if self.landing_page.sort == Sort::Size {
                                                        let chevron = if self.landing_page.sort_asc
                                                        {
                                                            Icon::CHEVRON_UP
                                                        } else {
                                                            Icon::CHEVRON_DOWN
                                                        };
                                                        ui.label(
                                                            RichText::new(chevron.icon)
                                                                .font(FontId::monospace(12.0))
                                                                .weak(),
                                                        );
                                                    }
                                                });
                                                ui.label("");
                                                ui.end_row();
                                            }

                                            let mut current_time_category = -1;
                                            let mut child_idx = 0;
                                            while child_idx < descendents.len() {
                                                let child = descendents[child_idx];

                                                // Check if we need to insert a time separator (only when flattening)
                                                if self.landing_page.flatten_tree {
                                                    let current_modified =
                                                        files.last_modified_recursive(child.id);
                                                    let now = lb_rs::model::clock::get_time().0;
                                                    let current_time_diff =
                                                        now - current_modified as i64;

                                                    let get_time_category =
                                                        |time_diff: i64| -> i32 {
                                                            if time_diff <= 24 * 60 * 60 * 1000 {
                                                                0
                                                            }
                                                            // Today
                                                            else if time_diff
                                                                <= 2 * 24 * 60 * 60 * 1000
                                                            {
                                                                1
                                                            }
                                                            // Yesterday
                                                            else if time_diff
                                                                <= 7 * 24 * 60 * 60 * 1000
                                                            {
                                                                2
                                                            }
                                                            // This week
                                                            else if time_diff
                                                                <= 30 * 24 * 60 * 60 * 1000
                                                            {
                                                                3
                                                            }
                                                            // This month
                                                            else if time_diff
                                                                <= 365 * 24 * 60 * 60 * 1000
                                                            {
                                                                4
                                                            }
                                                            // This year
                                                            else {
                                                                5
                                                            } // All time
                                                        };

                                                    let new_category =
                                                        get_time_category(current_time_diff);

                                                    if new_category > current_time_category {
                                                        current_time_category = new_category;
                                                        let category_name =
                                                            match current_time_category {
                                                                0 => "Today",
                                                                1 => "Yesterday",
                                                                2 => "This Week",
                                                                3 => "This Month",
                                                                4 => "This Year",
                                                                _ => "All Time",
                                                            };

                                                        ui.vertical(|ui| {
                                                            ui.add_space(20.);
                                                            ui.label(
                                                                RichText::new(category_name)
                                                                    .font(FontId::new(
                                                                        16.0,
                                                                        egui::FontFamily::Name(
                                                                            Arc::from("Bold"),
                                                                        ),
                                                                    ))
                                                                    .weak(),
                                                            );
                                                        });
                                                        ui.label("");
                                                        ui.end_row();
                                                    }
                                                }

                                                child_idx += 1;

                                                // File name
                                                ui.horizontal(|ui| {
                                                    ui.add_space(20.0);

                                                    // File icon
                                                    if child.is_folder() {
                                                        let folder_icon =
                                                            if !child.shares.is_empty() {
                                                                Icon::SHARED_FOLDER
                                                            } else {
                                                                Icon::FOLDER
                                                            };
                                                        ui.label(
                                                            RichText::new(folder_icon.icon)
                                                                .font(FontId::monospace(19.0))
                                                                .color(
                                                                    ui.style()
                                                                        .visuals
                                                                        .widgets
                                                                        .active
                                                                        .bg_fill,
                                                                ),
                                                        );
                                                    } else {
                                                        ui.label(
                                                            RichText::new(
                                                                DocType::from_name(&child.name)
                                                                    .to_icon()
                                                                    .icon,
                                                            )
                                                            .font(FontId::monospace(19.0))
                                                            .color(ui.visuals().weak_text_color()),
                                                        );
                                                    }

                                                    // Show parent path if tree is flattened
                                                    if self.landing_page.flatten_tree
                                                        && !child.is_root()
                                                        && child.parent != folder.id
                                                    {
                                                        let parent = files
                                                            .files
                                                            .get_by_id(child.parent)
                                                            .unwrap();
                                                        if ui
                                                            .link(
                                                                RichText::new(&parent.name).weak(),
                                                            )
                                                            .clicked()
                                                        {
                                                            open_file = Some(parent.id);
                                                        }
                                                        ui.label(RichText::new(" / ").weak());
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
                                                    if ui.link(text).clicked() {
                                                        open_file = Some(child.id);
                                                    }
                                                });

                                                // // Type column
                                                if !self.landing_page.flatten_tree {
                                                    ui.label(RichText::new(if child.is_folder() {
                                                        "Folder".to_string()
                                                    } else {
                                                        DocType::from_name(&child.name).to_string()
                                                    }));
                                                }

                                                // Last modified
                                                {
                                                    let last_modified_timestamp =
                                                        files.last_modified_recursive(child.id);
                                                    let formatted_date = {
                                                        let system_time = std::time::UNIX_EPOCH
                                                            + std::time::Duration::from_millis(
                                                                last_modified_timestamp,
                                                            );
                                                        let datetime: chrono::DateTime<
                                                            chrono::Local,
                                                        > = system_time.into();
                                                        datetime
                                                            .format("%B %d, %Y at %I:%M %p")
                                                            .to_string()
                                                    };

                                                    if self.landing_page.flatten_tree {
                                                        ui.with_layout(
                                                            Layout {
                                                                main_dir: Direction::RightToLeft,
                                                                main_wrap: false,
                                                                main_align: Align::Max,
                                                                main_justify: false,
                                                                cross_align: Align::Min,
                                                                cross_justify: false,
                                                            },
                                                            |ui| {
                                                                let mut last_modified_by = files
                                                                    .last_modified_by_recursive(
                                                                        child.id,
                                                                    );
                                                                if last_modified_by
                                                                    == account.username
                                                                {
                                                                    last_modified_by = "you";
                                                                }
                                                                ui.label(
                                                                    RichText::new(format!(
                                                                        "by {}",
                                                                        last_modified_by
                                                                    ))
                                                                    .weak(),
                                                                );

                                                                ui.label(RichText::new(
                                                                    last_modified_timestamp
                                                                        .elapsed_human_string(),
                                                                ))
                                                                .on_hover_text(&formatted_date);
                                                            },
                                                        );
                                                    } else {
                                                        ui.horizontal(|ui: &mut Ui| {
                                                            ui.label(RichText::new(
                                                                last_modified_timestamp
                                                                    .elapsed_human_string(),
                                                            ))
                                                            .on_hover_text(&formatted_date);

                                                            let mut last_modified_by = files
                                                                .last_modified_by_recursive(
                                                                    child.id,
                                                                );
                                                            if last_modified_by == account.username
                                                            {
                                                                last_modified_by = "you";
                                                            }
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "by {}",
                                                                    last_modified_by
                                                                ))
                                                                .weak(),
                                                            );
                                                        });
                                                    }
                                                }

                                                // Usage
                                                if !self.landing_page.flatten_tree {
                                                    ui.label(RichText::new({
                                                        bytes_to_human(
                                                            files.size_bytes_recursive(child.id)
                                                                as _,
                                                        )
                                                    }));

                                                    // Usage bar chart
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add_space(20.);
                                                            let (_, mut rect) =
                                                                ui.allocate_space(Vec2::new(
                                                                    ui.available_width(),
                                                                    ui.available_height(),
                                                                ));
                                                            let target_width = rect.width()
                                                                * files
                                                                    .usage_portion_scaled(child.id);
                                                            let excess_width =
                                                                rect.width() - target_width;
                                                            rect.max.x -= excess_width;

                                                            ui.painter().rect_filled(
                                                                rect,
                                                                2.0,
                                                                ui.visuals()
                                                                    .widgets
                                                                    .active
                                                                    .bg_fill
                                                                    .gamma_multiply(0.8),
                                                            );
                                                        },
                                                    );
                                                }

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
                        });
                    });
                });
        });

        if let Some(id) = open_file {
            if files.files.get_by_id(id).unwrap().is_document() {
                self.open_file(id, true, false);
            } else {
                self.focused_parent = Some(id);
                self.out.selected_file = Some(id)
            }
        }
        if create_note {
            self.create_doc(false);
        }
        if create_drawing {
            self.create_doc(true);
        }
        if create_folder {
            self.create_folder();
        }

        // Persist landing page if it changed
        if self.landing_page != initial_landing_page {
            self.cfg.set_landing_page(self.landing_page.clone());
        }
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if self.current_tab().is_some() && self.show_tabs {
                self.show_tab_strip(ui);
            }

            ui.centered_and_justified(|ui| {
                let mut open_id = None;
                let mut new_tab = false;
                if let Some(tab) = self.current_tab_mut() {
                    let id = tab.id();
                    match &mut tab.content {
                        ContentState::Loading(_) => {
                            ui.spinner();
                        }
                        ContentState::Failed(fail) => {
                            ui.label(fail.msg());
                        }
                        ContentState::Open(content) => {
                            match content {
                                TabContent::Markdown(md) => {
                                    let initialized = md.initialized;
                                    let resp = md.show(ui);
                                    // The editor signals a text change when the buffer is initially
                                    // loaded. Since we use that signal to trigger saves, we need to
                                    // check that this change was not from the initial frame.
                                    if !tab.read_only && resp.text_updated && initialized {
                                        tab.last_changed = Instant::now();
                                    }

                                    self.out.open_camera = resp.open_camera;

                                    if resp.text_updated {
                                        self.out.markdown_editor_text_updated = true;
                                        self.out.markdown_editor_selection_updated = true;
                                    }
                                    if resp.selection_updated {
                                        self.out.markdown_editor_selection_updated = true;
                                    }
                                    if resp.scroll_updated {
                                        self.out.markdown_editor_scroll_updated = true;
                                    }
                                }
                                TabContent::Image(img) => {
                                    if let Err(err) = img.show(ui) {
                                        tab.content = ContentState::Failed(err.into());
                                    }
                                }
                                TabContent::Pdf(pdf) => pdf.show(ui),
                                TabContent::Svg(svg) => {
                                    let res = svg.show(ui);
                                    if res.request_save {
                                        tab.last_changed = Instant::now();
                                    }
                                }
                                TabContent::MindMap(mm) => {
                                    let response = mm.show(ui);
                                    if let Some(value) = response {
                                        self.open_file(value, true, false);
                                    }
                                }
                                TabContent::SpaceInspector(sv) => {
                                    sv.show(ui);
                                }
                            };
                        }
                    }

                    ui.ctx().output_mut(|w| {
                        if let Some(url) = &w.open_url {
                            // only intercept open urls for tabs representing files
                            let Some(id) = id else {
                                return;
                            };

                            // lookup this file so we can get the parent
                            let Ok(file) = self.core.get_file_by_id(id) else {
                                return;
                            };

                            // evaluate relative path based on parent location
                            let Ok(file) =
                                core_get_by_relative_path(&self.core, file.parent, &url.url)
                            else {
                                return;
                            };

                            // if all that found something then open within lockbook
                            open_id = Some(file.id);
                            new_tab = url.new_tab;

                            w.open_url = None;
                        }
                    });
                }
                if let Some(id) = open_id {
                    self.open_file(id, true, new_tab);
                }
            });
        });
    }

    fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
        let active_tab_changed = self.current_tab_changed;
        self.current_tab_changed = false;

        let mut back = false;
        let mut forward = false;

        let cursor = ui
            .horizontal(|ui| {
                if IconButton::new(Icon::ARROW_LEFT)
                    .disabled(
                        self.current_tab()
                            .map(|tab| tab.back.is_empty())
                            .unwrap_or_default(),
                    )
                    .size(37.)
                    .tooltip("Go Back")
                    .show(ui)
                    .clicked()
                {
                    back = true;
                }
                if IconButton::new(Icon::ARROW_RIGHT)
                    .disabled(
                        self.current_tab()
                            .map(|tab| tab.forward.is_empty())
                            .unwrap_or_default(),
                    )
                    .size(37.)
                    .tooltip("Go Forward")
                    .show(ui)
                    .clicked()
                {
                    forward = true;
                }

                egui::ScrollArea::horizontal()
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        let mut responses = HashMap::new();
                        for i in 0..self.tabs.len() {
                            if let Some(resp) =
                                self.tab_label(ui, i, self.current_tab == i, active_tab_changed)
                            {
                                responses.insert(i, resp);
                            }
                        }

                        // handle responses after showing all tabs because closing a tab invalidates tab indexes
                        for (i, resp) in responses {
                            match resp {
                                TabLabelResponse::Clicked => {
                                    if self.current_tab == i {
                                        // we should rename the file.

                                        self.out.tab_title_clicked = true;
                                        let active_name = self.tab_title(&self.tabs[i]);

                                        let mut rename_edit_state =
                                            egui::text_edit::TextEditState::default();
                                        rename_edit_state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange {
                                                primary: egui::text::CCursor::new(
                                                    active_name
                                                        .rfind('.')
                                                        .unwrap_or(active_name.len()),
                                                ),
                                                secondary: egui::text::CCursor::new(0),
                                            },
                                        ));
                                        egui::TextEdit::store_state(
                                            ui.ctx(),
                                            egui::Id::new("rename_tab"),
                                            rename_edit_state,
                                        );
                                        self.tabs[i].rename = Some(active_name);
                                    } else {
                                        self.tabs[i].rename = None;
                                        self.make_current(i);
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    if let Some(id) = self.tabs[i].id() {
                                        self.rename_file((id, name.clone()), true);
                                    }
                                }
                                TabLabelResponse::Reordered { src, mut dst } => {
                                    let current = self.current_tab_id();

                                    let tab = self.tabs.remove(src);
                                    if src < dst {
                                        dst -= 1;
                                    }
                                    self.tabs.insert(dst, tab);

                                    if let Some(current) = current {
                                        self.make_current_by_id(current);
                                    }
                                }
                            }
                            ui.ctx().request_repaint();
                        }
                    });
                ui.cursor()
            })
            .inner;

        ui.style_mut().animation_time = 2.0;

        let end_of_tabs = cursor.min.x;
        let available_width = ui.available_width();
        let remaining_rect = Rect::from_x_y_ranges(
            Rangef { min: end_of_tabs, max: end_of_tabs + available_width },
            cursor.y_range(),
        );
        let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;

        let bg_color = get_apple_bg_color(ui);
        ui.painter().rect_filled(remaining_rect, 0.0, bg_color);

        ui.painter()
            .hline(remaining_rect.x_range(), cursor.max.y, sep_stroke);

        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
    }

    fn process_keys(&mut self) {
        const APPLE: bool = cfg!(target_vendor = "apple");
        const COMMAND: Modifiers = Modifiers::COMMAND;
        const CTRL: Modifiers = Modifiers::CTRL;
        const SHIFT: Modifiers = Modifiers::SHIFT;
        const ALT: Modifiers = Modifiers::ALT;
        const NUM_KEYS: [Key; 10] = [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        // Ctrl-N pressed while new file modal is not open.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::N))
        {
            self.create_doc(false);
        }

        // Ctrl-S to save current tab.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::S))
        {
            self.save_tab(self.current_tab);
        }

        // Ctrl-M to open mind map
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::M))
        {
            self.upsert_mind_map(self.core.clone());
        }

        // Ctrl-W to close current tab.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::W))
            && !self.is_empty()
        {
            self.close_tab(self.current_tab);
            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                self.current_tab_title().unwrap_or("Lockbook".to_owned()),
            ));

            self.out.selected_file = self.current_tab_id();
        }

        // Ctrl-shift-W to close all tabs
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND | SHIFT, egui::Key::W))
            && !self.is_empty()
        {
            for i in 0..self.tabs.len() {
                self.close_tab(i);
            }

            self.out.selected_file = None;
            self.ctx
                .send_viewport_cmd(ViewportCommand::Title("Lockbook".into()));
        }

        // reorder tabs
        // non-apple: ctrl+shift+pg down / up
        // apple: command+control+shift [ ]
        let change: i32 = self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::OpenBracket) {
                    -1
                } else if input
                    .consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::CloseBracket)
                {
                    1
                } else {
                    0
                }
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageUp) {
                -1
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageDown) {
                1
            } else {
                0
            }
        });
        if change != 0 {
            let old = self.current_tab as i32;
            let new = old + change;
            if new >= 0 && new < self.tabs.len() as i32 {
                self.tabs.swap(old as usize, new as usize);
                self.make_current(new as usize);
            }
        }

        // tab navigation
        let mut goto_tab = None;
        self.ctx.input_mut(|input| {
            // Cmd+1 through Cmd+8 to select tab by cardinal index
            for (i, &key) in NUM_KEYS.iter().enumerate().skip(1).take(8) {
                if input.consume_key_exact(COMMAND, key)
                    || (!APPLE && input.consume_key_exact(Modifiers::ALT, key))
                {
                    goto_tab = Some(i.min(self.tabs.len()) - 1);
                }
            }

            // Cmd+9 to go to last tab
            if input.consume_key_exact(COMMAND, Key::Num9)
                || (!APPLE && input.consume_key_exact(Modifiers::ALT, Key::Num9))
            {
                goto_tab = Some(self.tabs.len() - 1);
            }

            // Cmd+Shift+[ or ctrl shift tab to go to previous tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::OpenBracket))
                || (!APPLE && input.consume_key_exact(CTRL | SHIFT, Key::Tab)))
                && self.current_tab != 0
            {
                goto_tab = Some(self.current_tab - 1);
            }

            // Cmd+Shift+] or ctrl tab to go to next tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::CloseBracket))
                || (!APPLE && input.consume_key_exact(CTRL, Key::Tab)))
                && self.current_tab != self.tabs.len() - 1
            {
                goto_tab = Some(self.current_tab + 1);
            }
        });

        if let Some(goto_tab) = goto_tab {
            self.make_current(goto_tab);
        }

        // forward/back
        // non-apple: alt + arrows
        // apple: command + brackets
        let mut back = false;
        let mut forward = false;
        self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(COMMAND, Key::OpenBracket) {
                    back = true;
                }
                if input.consume_key_exact(COMMAND, Key::CloseBracket) {
                    forward = true;
                }
            } else {
                if input.consume_key_exact(ALT, Key::ArrowLeft) {
                    back = true;
                }
                if input.consume_key_exact(ALT, Key::ArrowRight) {
                    forward = true;
                }
            }
        });

        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
    }

    fn tab_label(
        &mut self, ui: &mut egui::Ui, t: usize, is_active: bool, active_tab_changed: bool,
    ) -> Option<TabLabelResponse> {
        let mut result = None;
        let icon_size = 15.0;
        let x_icon = Icon::CLOSE.size(icon_size);
        let status = self.tab_status(t);

        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::new(14.0, egui::FontFamily::Proportional));

        let tab_bg =
            if is_active { ui.style().visuals.extreme_bg_color } else { get_apple_bg_color(ui) };

        let tab_padding = egui::Margin::symmetric(10.0, 10.0);

        let tab_label = egui::Frame::default()
            .fill(tab_bg)
            .inner_margin(tab_padding)
            .show(ui, |ui| {
                ui.add_visible_ui(self.tabs[t].rename.is_none(), |ui| {
                    let start = ui.available_rect_before_wrap().min;

                    // create galleys - text layout

                    // tab label - the actual file name
                    let text: egui::WidgetText = self.tab_title(&self.tabs[t]).into();
                    let text = text.into_galley(
                        ui,
                        Some(TextWrapMode::Truncate),
                        200.0,
                        egui::TextStyle::Body,
                    );

                    // tab marker - tab status / tab number
                    let tab_marker = if status == TabStatus::Clean {
                        (t + 1).to_string()
                    } else {
                        "*".to_string()
                    };
                    let tab_marker: egui::WidgetText = egui::RichText::new(tab_marker)
                        .font(egui::FontId::monospace(12.0))
                        .color(if status == TabStatus::Clean {
                            ui.style().visuals.weak_text_color()
                        } else {
                            ui.style().visuals.warn_fg_color
                        })
                        .into();
                    let tab_marker = tab_marker.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Body,
                    );

                    // close button - the x
                    let close_button: egui::WidgetText = egui::RichText::new(x_icon.icon)
                        .font(egui::FontId::monospace(10.))
                        .into();
                    let close_button = close_button.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        f32::INFINITY,
                        egui::TextStyle::Body,
                    );

                    // create rects - place these relative to one another
                    let marker_rect = centered_galley_rect(&tab_marker);
                    let marker_rect = Align2::LEFT_TOP.anchor_size(
                        start
                            + egui::vec2(
                                0.0,
                                text.rect.height() / 2.0 - marker_rect.height() / 2.0,
                            ),
                        marker_rect.size(),
                    );

                    let text_rect = egui::Align2::LEFT_TOP.anchor_size(
                        start + egui::vec2(tab_marker.rect.width() + 7.0, 0.0),
                        text.size(),
                    );

                    let close_button_rect = centered_galley_rect(&close_button);
                    let close_button_rect = egui::Align2::LEFT_TOP.anchor_size(
                        text_rect.right_top()
                            + vec2(5.0, (text.rect.height() - close_button_rect.height()) / 2.0),
                        close_button_rect.size(),
                    );

                    // tab label rect represents the whole tab label
                    let left_top = start - tab_padding.left_top();
                    let right_bottom =
                        close_button_rect.right_bottom() + tab_padding.right_bottom();
                    let tab_label_rect = Rect::from_min_max(left_top, right_bottom);

                    // uncomment to see geometry debug views
                    // let s = egui::Stroke::new(1., egui::Color32::RED);
                    // ui.painter().rect_stroke(marker_rect, 1., s);
                    // ui.painter().rect_stroke(text_rect, 1., s);
                    // ui.painter().rect_stroke(close_button_rect, 1., s);
                    // ui.painter().rect_stroke(tab_label_rect, 1., s);

                    // render & process input
                    let touch_mode =
                        matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);

                    ui.painter().galley(
                        marker_rect.left_top(),
                        tab_marker.clone(),
                        ui.visuals().text_color(),
                    );

                    let mut tab_label_resp = ui.interact(
                        tab_label_rect,
                        Id::new("tab label").with(t),
                        Sense { click: true, drag: true, focusable: false },
                    );

                    let pointer_pos = ui.input(|i| i.pointer.interact_pos().unwrap_or_default());
                    let close_button_interact_rect =
                        close_button_rect.expand(if touch_mode { 4. } else { 2. });
                    let close_button_pointed = close_button_interact_rect.contains(pointer_pos);
                    let close_button_hovered = tab_label_resp.hovered() && close_button_pointed;
                    let close_button_clicked = tab_label_resp.clicked() && close_button_pointed;

                    tab_label_resp.clicked &= !close_button_clicked;

                    let text_color = if is_active {
                        ui.visuals().text_color()
                    } else {
                        ui.visuals()
                            .widgets
                            .noninteractive
                            .fg_stroke
                            .color
                            .linear_multiply(0.8)
                    };

                    // draw the tab text
                    ui.painter().galley(text_rect.min, text, text_color);

                    if close_button_clicked || tab_label_resp.middle_clicked() {
                        result = Some(TabLabelResponse::Closed);
                    }
                    if close_button_hovered {
                        ui.painter().rect(
                            close_button_interact_rect,
                            2.0,
                            ui.visuals().code_bg_color,
                            egui::Stroke::NONE,
                        );
                    }

                    let show_close_button = touch_mode || tab_label_resp.hovered() || is_active;
                    if show_close_button {
                        ui.painter().galley(
                            close_button_rect.min,
                            close_button,
                            ui.visuals().text_color(),
                        );
                    }
                    if tab_label_resp.clicked() {
                        result = Some(TabLabelResponse::Clicked);
                    }
                    tab_label_resp.context_menu(|ui| {
                        if ui.button("Close tab").clicked() {
                            result = Some(TabLabelResponse::Closed);
                            ui.close_menu();
                        }
                    });

                    ui.advance_cursor_after_rect(text_rect.union(close_button_rect));

                    // drag 'n' drop
                    {
                        // when drag starts, dragged tab sets dnd payload
                        if tab_label_resp.dragged() && !DragAndDrop::has_any_payload(ui.ctx()) {
                            DragAndDrop::set_payload(ui.ctx(), t);
                        }

                        if let (Some(pointer), true) = (
                            ui.input(|i| i.pointer.interact_pos()),
                            DragAndDrop::has_any_payload(ui.ctx()),
                        ) {
                            let contains_pointer = tab_label_rect.contains(pointer);
                            if contains_pointer {
                                // during drag, drop target renders indicator
                                let drop_left_side = pointer.x < tab_label_rect.center().x;
                                let stroke = ui.style().visuals.widgets.active.fg_stroke;
                                let x = if drop_left_side {
                                    tab_label_rect.min.x
                                } else {
                                    tab_label_rect.max.x
                                };
                                let y_range = tab_label_rect.y_range();

                                ui.with_layer_id(
                                    LayerId::new(
                                        Order::PanelResizeLine,
                                        Id::from("tab_reorder_drop_indicator"),
                                    ),
                                    |ui| {
                                        ui.painter().vline(x, y_range, stroke);
                                    },
                                );

                                // when drag ends, dropped-on tab consumes dnd payload
                                if let Some(drag_index) =
                                    tab_label_resp.dnd_release_payload::<usize>()
                                {
                                    let drop_index = if drop_left_side { t } else { t + 1 };
                                    result = Some(TabLabelResponse::Reordered {
                                        src: *drag_index,
                                        dst: drop_index,
                                    });
                                }
                            }
                        }
                    }

                    tab_label_resp
                })
            });

        // renaming
        if let Some(ref mut str) = self.tabs[t].rename {
            let res = ui
                .allocate_ui_at_rect(tab_label.response.rect, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(str)
                            .font(TextStyle::Small)
                            .frame(false)
                            .id(egui::Id::new("rename_tab")),
                    )
                })
                .inner;

            if !res.has_focus() && !res.lost_focus() {
                // request focus on the first frame (todo: wrong but works)
                res.request_focus();
            }
            if res.has_focus() {
                // focus lock filter must be set every frame
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        res.id,
                        EventFilter {
                            tab: true, // suppress 'tab' behavior
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: false, // press 'esc' to release focus
                        },
                    )
                })
            }

            // submit
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                result = Some(TabLabelResponse::Renamed(str.to_owned()));
                // t.rename = None; is done by code processing this response
            }

            // release focus to cancel ('esc' or click elsewhere)
            if res.lost_focus() {
                self.tabs[t].rename = None;
            }
        }

        if is_active && active_tab_changed {
            tab_label.response.scroll_to_me(None);
        }

        if !is_active && tab_label.response.hovered() {
            ui.painter().rect_filled(
                tab_label.response.rect,
                0.0,
                egui::Color32::WHITE.linear_multiply(0.002),
            );
        }

        if is_active && active_tab_changed {
            tab_label.response.scroll_to_me(None);
        }

        // draw separators
        let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        if !is_active {
            ui.painter().hline(
                tab_label.response.rect.x_range(),
                tab_label.response.rect.max.y,
                sep_stroke,
            );
        }
        ui.painter().vline(
            tab_label.response.rect.max.x,
            tab_label.response.rect.y_range(),
            sep_stroke,
        );

        tab_label.response.on_hover_ui(|ui| {
            let text = self.tab_status(t).summary();
            let text: egui::WidgetText = RichText::from(text).size(15.0).into();
            let text = text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Body);
            ui.add(egui::Label::new(text));

            let last_saved = self.tabs[t].last_saved.elapsed_human_string();
            let text: egui::WidgetText = RichText::from(format!("last saved {last_saved}"))
                .size(12.0)
                .into();
            let text = text.into_galley(ui, Some(TextWrapMode::Extend), 0., egui::TextStyle::Body);
            ui.add(egui::Label::new(text));

            ui.ctx().request_repaint_after_secs(1.0);
        });

        result
    }
}

/// get the color for the native apple title bar
fn get_apple_bg_color(ui: &mut egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(57, 57, 56)
    } else {
        egui::Color32::from_rgb(240, 240, 239)
    }
}

/// egui, when rendering a single monospace symbol character doesn't seem to be able to center a character vertically
/// this fn takes into account where the text was positioned within the galley and computes a size using mesh_bounds
/// and retruns a rect with uniform padding.
fn centered_galley_rect(galley: &Galley) -> Rect {
    let min = galley.rect.min;
    let offset = galley.rect.min - galley.mesh_bounds.min;
    let max = galley.mesh_bounds.max - offset;

    Rect { min, max }
}

enum TabLabelResponse {
    Clicked,
    Closed,
    Renamed(String),
    Reordered { src: usize, dst: usize },
}

// The only difference from count_and_consume_key is that here we use matches_exact instead of matches_logical,
// preserving the behavior before egui 0.25.0. The documentation for the 0.25.0 count_and_consume_key says
// "you should match most specific shortcuts first", but this doesn't go well with egui's usual pattern where widgets
// process input in the order in which they're drawn, with parent widgets (e.g. workspace) drawn before children
// (e.g. editor). Using this older way of doing things affects matching keyboard shortcuts with shift included e.g. '+'
pub trait InputStateExt {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize;
    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool;
}

impl InputStateExt for egui::InputState {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize {
        let mut count = 0usize;

        self.events.retain(|event| {
            let is_match = matches!(
                event,
                egui::Event::Key {
                    key: ev_key,
                    modifiers: ev_mods,
                    pressed: true,
                    ..
                } if *ev_key == logical_key && ev_mods.matches_exact(modifiers)
            );

            count += is_match as usize;

            !is_match
        });

        count
    }

    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool {
        self.count_and_consume_key_exact(modifiers, logical_key) > 0
    }
}

trait ElapsedHumanString {
    fn elapsed_human_string(&self) -> String;
}

impl ElapsedHumanString for time::Duration {
    fn elapsed_human_string(&self) -> String {
        let minutes = self.whole_minutes();
        let seconds = self.whole_seconds();
        if seconds > 0 && minutes == 0 {
            if seconds <= 1 { "1 second ago".to_string() } else { format!("{seconds} seconds ago") }
        } else {
            self.format_human().to_string()
        }
    }
}

impl ElapsedHumanString for std::time::Duration {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.as_millis() as _).elapsed_human_string()
    }
}

impl ElapsedHumanString for Instant {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.elapsed().as_millis() as _).elapsed_human_string()
    }
}

impl ElapsedHumanString for u64 {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(lb_rs::model::clock::get_time().0 - *self as i64)
            .elapsed_human_string()
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum DocType {
    PlainText,
    Markdown,
    SVG,
    Image,
    ImageUnsupported,
    Code,
    PDF,
    Unknown,
}

impl Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocType::PlainText => write!(f, "Plain Text"),
            DocType::Markdown => write!(f, "Markdown"),
            DocType::SVG => write!(f, "SVG"),
            DocType::Image => write!(f, "Image"),
            DocType::ImageUnsupported => write!(f, "Image (Unsupported)"),
            DocType::Code => write!(f, "Code"),
            DocType::PDF => write!(f, "PDF"),
            DocType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name.split('.').next_back().unwrap_or_default();
        match ext {
            "draw" | "svg" => Self::SVG,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "cr2" => Self::ImageUnsupported,
            "go" => Self::Code,
            "pdf" => Self::PDF,
            _ if image_viewer::is_supported_image_fmt(ext) => Self::Image,
            _ => Self::Unknown,
        }
    }

    pub fn to_icon(&self) -> Icon {
        match self {
            DocType::Markdown => Icon::DOC_MD,
            DocType::PlainText => Icon::DOC_TEXT,
            DocType::SVG => Icon::DRAW,
            DocType::Image => Icon::IMAGE,
            DocType::Code => Icon::CODE,
            DocType::PDF => Icon::DOC_PDF,
            _ => Icon::DOC_UNKNOWN,
        }
    }

    pub fn hide_ext(&self) -> bool {
        match self {
            DocType::PlainText => false,
            DocType::Markdown => true,
            DocType::SVG => true,
            DocType::Image => false,
            DocType::ImageUnsupported => false,
            DocType::Code => false,
            DocType::PDF => true,
            DocType::Unknown => false,
        }
    }
}
