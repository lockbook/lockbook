use super::data::{Data, StorageCell, StorageTree};
use crate::theme::icons::Icon;
use crate::widgets::Button;
use color_art;
use colors_transform::{self, Color};
use egui::{
    self, Color32, Context, Id, LayerId, Pos2, Rect, Rounding, Sense, Stroke, TextWrapMode, Ui,
    UiBuilder, menu,
};
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbErr;
use lb_rs::model::file::File;
use lb_rs::model::usage::bytes_to_human;
use lb_rs::{LbErrKind, Uuid, spawn};
use std::sync::{Arc, Mutex};

/// Responsible for tracking on screen locations for folders
#[derive(Debug)]
struct DrawHelper {
    id: Uuid,
    starting_position: f32,
}

/// Responsible for keeping colors consistent
struct ColorHelper {
    id: Uuid,
    color: Color32,
}

pub struct SpaceInspector {
    lb: Lb,
    state: Arc<Mutex<AppState>>,
    data: Data,
    layer_height: f32,
    paint_order: Vec<StorageCell>,
    colors: Vec<ColorHelper>,
    current_rect: Rect,
}

#[derive(Default, Debug)]
enum AppState {
    #[default]
    Loading,
    Ready(Data),
    Error(LbErr),
}

impl SpaceInspector {
    pub fn new(lb: &Lb, potential_root: Option<File>, ctx: Context) -> Self {
        let bg_lb = lb.clone();
        let state: Arc<Mutex<AppState>> = Default::default();
        let bg_state = state.clone();
        spawn!({
            let usage = bg_lb.get_usage();
            let meta_data = bg_lb.list_metadatas();

            match (usage, meta_data) {
                (Ok(usage_result), Ok(metadata_result)) => {
                    let mut lock = bg_state.lock().unwrap();
                    *lock = AppState::Ready(Data::init(
                        potential_root,
                        usage_result.usages,
                        metadata_result,
                    ));
                }
                (Err(err), _) | (_, Err(err)) => {
                    let mut lock = bg_state.lock().unwrap();
                    *lock = AppState::Error(err);
                }
            }
            ctx.request_repaint();
        });

        Self {
            state,
            data: Default::default(),
            paint_order: vec![],
            layer_height: 60.0,
            colors: vec![],
            current_rect: Rect::NOTHING,
            lb: lb.clone(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Start of pre ui checks
        let window = ui.available_rect_before_wrap();
        if self.data == Default::default() {
            match &*self.state.lock().unwrap() {
                AppState::Loading => {
                    ui.allocate_new_ui(
                        UiBuilder::new().max_rect(Rect {
                            min: Pos2 { x: window.center().x - 30.0, y: window.center().y - 30.0 },
                            max: Pos2 { x: window.center().x + 30.0, y: window.center().y + 30.0 },
                        }),
                        |ui| {
                            Button::default()
                                .text("LOADING")
                                .icon(&Icon::SYNC)
                                .is_loading(true)
                                .frame(false)
                                .text_style(egui::TextStyle::Body)
                                .show(ui);
                        },
                    );

                    return;
                }
                AppState::Ready(data) => {
                    self.data = data.clone();
                }
                AppState::Error(lb_err) => {
                    match lb_err.kind {
                        LbErrKind::ClientUpdateRequired => {
                            Button::default()
                                .text("Client Update Required")
                                .icon(&Icon::BRING_TO_FRONT)
                                .icon_color(Color32::RED)
                                .frame(false)
                                .indent(window.width() / 2.0)
                                .text_style(egui::TextStyle::Body)
                                .show(ui);
                        }
                        LbErrKind::ServerDisabled => {
                            Button::default()
                                .text("Server Disabled")
                                .icon(&Icon::SYNC_PROBLEM)
                                .icon_color(Color32::RED)
                                .frame(false)
                                .indent(window.width() / 2.0)
                                .text_style(egui::TextStyle::Body)
                                .show(ui);
                        }
                        LbErrKind::ServerUnreachable => {
                            Button::default()
                                .text("Server Unreachable")
                                .icon(&Icon::SYNC_PROBLEM)
                                .icon_color(Color32::RED)
                                .frame(false)
                                .indent(window.width() / 2.0)
                                .text_style(egui::TextStyle::Body)
                                .show(ui);
                        }
                        _ => {
                            Button::default()
                                .text("Unknown Error")
                                .icon(&Icon::BUG)
                                .icon_color(Color32::RED)
                                .frame(false)
                                .indent(window.width() / 2.0)
                                .text_style(egui::TextStyle::Body)
                                .show(ui);
                        }
                    };
                    return;
                }
            }
        }

        if self.paint_order.is_empty() || window != self.current_rect {
            self.current_rect = window;
            self.paint_order = self.data.get_paint_order();
        }

        let root_color =
            if ui.visuals().dark_mode { Color32::from_gray(255) } else { Color32::from_gray(10) };
        let root_text_color =
            if ui.visuals().dark_mode { Color32::from_gray(10) } else { Color32::from_gray(255) };

        // Top buttons
        ui.scope_builder(
            UiBuilder::new().layer_id(LayerId { order: egui::Order::Foreground, id: Id::new(1) }),
            |ui| {
                let top_left_rect = Rect { min: window.left_top(), max: window.center_top() };
                ui.allocate_new_ui(UiBuilder::new().max_rect(top_left_rect), |ui| {
                    menu::bar(ui, |ui| {
                        if ui.button("Reset Root").clicked() {
                            self.reset_root();
                            self.paint_order = vec![];
                        }

                        ui.menu_button("Layer Size", |ui| {
                            ui.add(egui::Slider::new(&mut self.layer_height, 1.0..=100.0));
                        });
                    });
                });
            },
        );

        // Root drawing logic
        let root_draw_anchor = Rect {
            min: Pos2 { x: self.current_rect.min.x, y: self.current_rect.max.y - 40.0 },
            max: self.current_rect.max,
        };

        let root_text = Rect { min: root_draw_anchor.center(), max: root_draw_anchor.center() };

        let painter = ui.painter();
        painter
            .clone()
            .rect_filled(root_draw_anchor, 0.0, root_color);

        // Root text logic

        let display_size = bytes_to_human(self.data.folder_sizes[&self.data.focused_folder]);

        let tab_intel: egui::WidgetText = egui::RichText::new(display_size.clone())
            .font(egui::FontId::monospace(15.0))
            .color(root_text_color)
            .into();

        let tab_intel_galley = tab_intel.into_galley(
            ui,
            Some(TextWrapMode::Extend),
            root_text.width(),
            egui::TextStyle::Body,
        );

        let tab_intel_rect = egui::Align2::LEFT_TOP.anchor_size(
            Pos2 { x: root_text.left_center().x - 25.0, y: root_text.left_center().y - 20.0 },
            tab_intel_galley.size(),
        );

        ui.painter().galley(
            tab_intel_rect.left_center(),
            tab_intel_galley,
            ui.visuals().text_color(),
        );

        // Hover interaction

        let response = ui.interact(root_draw_anchor, Id::new(-1), Sense::hover());

        response.on_hover_text(
            "Name:\n".to_owned()
                + &self.data.all_files[&self.data.focused_folder]
                    .file
                    .name
                    .to_string()
                + "\nSize:\n"
                + &display_size,
        );

        // Starts drawing the rest of the folders and files
        let potential_new_root = self.follow_paint_order(ui, root_draw_anchor, window);
        // assigning a new root if selected
        if let Some(new_root) = potential_new_root {
            self.change_root(new_root);
        }
    }

    pub fn change_root(&mut self, new_root: Uuid) {
        self.data.focused_folder = new_root;
        self.paint_order = vec![];
    }

    pub fn reset_root(&mut self) {
        self.data.focused_folder = self.data.root;
        self.paint_order = vec![];
    }

    pub fn get_color(&self, curr_id: Uuid, mut layer: usize, mut child_number: usize) -> Color32 {
        let big_table = [
            // red
            [
                Color32::from_rgb(128, 15, 47),
                Color32::from_rgb(164, 19, 60),
                Color32::from_rgb(201, 24, 74),
                Color32::from_rgb(255, 77, 109),
                Color32::from_rgb(255, 117, 143),
                Color32::from_rgb(255, 143, 163),
            ],
            // green
            [
                Color32::from_rgb(27, 67, 50),
                Color32::from_rgb(45, 106, 79),
                Color32::from_rgb(64, 145, 108),
                Color32::from_rgb(82, 183, 136),
                Color32::from_rgb(116, 198, 157),
                Color32::from_rgb(116, 198, 157),
            ],
            // blue
            [
                Color32::from_rgb(2, 62, 138),
                Color32::from_rgb(0, 119, 182),
                Color32::from_rgb(0, 150, 199),
                Color32::from_rgb(0, 180, 216),
                Color32::from_rgb(72, 202, 228),
                Color32::from_rgb(144, 224, 239),
            ],
        ];
        if layer == 1 {
            if child_number > 2 {
                child_number %= 3;
            }
            return big_table[child_number][0];
        }

        let parent_color = self
            .colors
            .iter()
            .find(|item| item.id == self.data.all_files[&curr_id].file.parent)
            .unwrap()
            .color;

        let parent_type = big_table
            .iter()
            .enumerate()
            .find_map(|(row_index, row)| {
                row.iter()
                    .position(|&x| x == parent_color)
                    .map(|col_index| (row_index, col_index))
            })
            .unwrap()
            .0;

        layer -= 1;

        if layer > 5 {
            layer %= 6;
        }

        big_table[parent_type][layer]
    }

    /// Responsible for drawing each file / folder. Also gives them interactivity with the root functionality and context menu
    pub fn follow_paint_order(
        &mut self, ui: &mut Ui, root_anchor: Rect, window: Rect,
    ) -> Option<Uuid> {
        let mut changed_focused_folder: Option<Uuid> = None;
        let mut current_layer = 0;
        let mut current_position = root_anchor.min.x;
        let mut child_number = 1;
        let mut visited_folders: Vec<DrawHelper> = vec![];
        let mut current_parent =
            DrawHelper { id: self.data.focused_folder, starting_position: 0.0 };

        let mut deleted_id: Option<Uuid> = None;

        for (i, item) in self.paint_order.iter().enumerate() {
            let item_filerow = &self.data.all_files[&item.id];

            if current_layer != item.layer {
                current_position = root_anchor.min.x;
                current_layer = item.layer;
            }

            if item_filerow.file.parent != current_parent.id {
                child_number = 1;
                current_position = visited_folders
                    .iter()
                    .find(|parent| parent.id == item_filerow.file.parent)
                    .unwrap()
                    .starting_position;
                current_parent = DrawHelper {
                    id: self.data.all_files[&item.id].file.parent,
                    starting_position: current_position,
                };
            }
            let painter = ui.painter();
            let paint_rect = Rect {
                min: Pos2 {
                    x: current_position,
                    y: root_anchor.min.y - (current_layer as f32) * self.layer_height,
                },
                max: Pos2 {
                    x: current_position + (item.portion * (root_anchor.max.x - root_anchor.min.x)),
                    y: root_anchor.min.y - ((current_layer - 1) as f32) * self.layer_height,
                },
            };

            let current_color = self
                .colors
                .iter()
                .find_map(|element| if element.id == item.id { Some(element.color) } else { None })
                .unwrap_or(SpaceInspector::get_color(
                    self,
                    item.id,
                    current_layer as usize,
                    child_number - 1,
                ));

            // Folder text logic
            if window.contains(paint_rect.min) {
                let tab_intel: egui::WidgetText = egui::RichText::new(&item.name)
                    .font(egui::FontId::monospace(0.2 * self.layer_height))
                    .color({
                        let hsl_color = colors_transform::Rgb::from(
                            current_color.r().into(),
                            current_color.g().into(),
                            current_color.b().into(),
                        )
                        .to_hsl();
                        let luminance: f32 = if hsl_color.get_lightness() > 50.0 {
                            (hsl_color.get_lightness() - 50.0) / 100.0
                        } else {
                            (hsl_color.get_lightness() + 50.0) / 100.0
                        };
                        Color32::from_hex(
                            &(color_art::color!(
                                HSL,
                                hsl_color.get_hue(),
                                hsl_color.get_saturation() / 100.0,
                                luminance
                            ))
                            .hex(),
                        )
                        .unwrap_or(Color32::DEBUG_COLOR)
                    })
                    .into();

                let tab_intel_galley = tab_intel.into_galley(
                    ui,
                    Some(TextWrapMode::Truncate),
                    paint_rect.width() - 5.0,
                    egui::TextStyle::Body,
                );

                let tab_intel_rect = egui::Align2::LEFT_TOP.anchor_size(
                    Pos2 { x: paint_rect.left_center().x + 5.0, y: paint_rect.left_center().y },
                    tab_intel_galley.size(),
                );

                // painting info
                painter.clone().rect(
                    paint_rect,
                    Rounding::ZERO,
                    current_color,
                    Stroke { width: 0.5, color: ui.visuals().extreme_bg_color },
                );

                if paint_rect.width() >= 50.0 {
                    ui.painter().galley(
                        tab_intel_rect.left_center() - egui::vec2(0.0, 5.5),
                        tab_intel_galley,
                        ui.visuals().text_color(),
                    );
                }

                // Click and hover logic

                let display_size = if item_filerow.file.is_folder() {
                    bytes_to_human(self.data.folder_sizes[&item.id])
                } else {
                    bytes_to_human(item_filerow.size)
                };

                let hover_text = "Name:\n".to_owned()
                    + &self.data.all_files[&item.id].file.name.to_string()
                    + "\nSize:\n"
                    + &display_size;

                let response = ui.interact(paint_rect, Id::new(i), Sense::click());

                if response.clicked() && item_filerow.file.is_folder() {
                    changed_focused_folder = Some(item.id);
                }
                // Context menu
                response.context_menu(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);

                    ui.label(self.data.all_files[&item.id].file.name.to_string());
                    ui.label(&display_size);
                    ui.separator();

                    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                        ui.close_menu();
                    }
                    if item_filerow.file.is_folder() && ui.button("Focus File").clicked() {
                        changed_focused_folder = Some(item.id);
                        ui.close_menu();
                    }

                    if ui.button("Delete").clicked() {
                        let lb = self.lb.clone();
                        let id = item_filerow.file.id;
                        deleted_id = Some(id);
                        spawn!({
                            lb.delete_file(&id).unwrap();
                        });
                    }
                });
                response.on_hover_text(hover_text);
            }

            if item_filerow.file.is_folder() {
                visited_folders
                    .push(DrawHelper { id: item.id, starting_position: current_position });
            }
            self.colors
                .push(ColorHelper { id: item.id, color: current_color });

            current_position += item.portion * (root_anchor.max.x - root_anchor.min.x);
            child_number += 1;
        }

        // handles visual deletion
        if let Some(id) = deleted_id {
            // Updates the size of all parents of deleted file
            let deleted_size = Data::get_size(&self.data, &id);
            self.update_parent_sizes(&id, deleted_size);

            // Handles deletion of selected file and its children
            let deleted_children = Data::get_children(&self.data, &id);
            self.delete_children(deleted_children);
            if Data::is_folder(&self.data, &id) {
                self.data.folder_sizes.remove(&id);
            }
            self.data.all_files.remove(&id);
            if let Some(pos) = self.paint_order.iter().position(|cell| cell.id == id) {
                self.paint_order.remove(pos);
            }
        };
        changed_focused_folder
    }

    /// Recursive function that goes up to update folder sizes of parents
    fn update_parent_sizes(&mut self, child: &Uuid, size_difference: u64) {
        let parent_id = self.data.all_files[child].file.parent;
        if parent_id == *child {
            return;
        }

        let original_size = self.data.folder_sizes[&parent_id];
        self.data
            .folder_sizes
            .insert(parent_id, original_size - size_difference);
        self.update_parent_sizes(&parent_id, size_difference);
    }

    /// Recursive function that is called when file is deleted
    fn delete_children(&mut self, trees: Vec<StorageTree>) {
        for tree in trees {
            if Data::is_folder(&self.data, &tree.id) {
                self.data.folder_sizes.remove(&tree.id);
            }
            self.data.all_files.remove(&tree.id);
            if let Some(pos) = self.paint_order.iter().position(|cell| cell.id == tree.id) {
                self.paint_order.remove(pos);
            }
            if !tree.children.is_empty() {
                self.delete_children(tree.children);
            }
        }
    }
}
