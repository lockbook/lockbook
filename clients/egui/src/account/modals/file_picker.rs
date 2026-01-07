use lb::blocking::Lb;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

use workspace_rs::show::DocType;

pub struct FilePicker {
    core: Lb,
    panels: Vec<Panel>,
    action: FilePickerAction,
}

pub struct FilePickerParams {
    pub parent: File,
    pub action: FilePickerAction,
}

#[derive(Clone)]
struct Panel {
    root: File,
    children: Vec<File>,
}

#[derive(Clone)]
pub enum FilePickerAction {
    AcceptShare(File),
    DroppedFiles(Vec<egui::DroppedFile>),
}

impl FilePicker {
    pub fn new(core: &Lb, action: FilePickerAction) -> Self {
        let core = core.clone();
        let root = core.get_root().unwrap();
        let root_panel =
            Panel { root: root.clone(), children: core.get_children(&root.id).unwrap() };

        Self { core, panels: vec![root_panel], action }
    }

    fn target_type(&self) -> FileType {
        match &self.action {
            FilePickerAction::AcceptShare(file) => file.file_type,
            FilePickerAction::DroppedFiles(drops) => {
                for drop in drops {
                    if let Some(path) = &drop.path {
                        return if path.is_dir() { FileType::Folder } else { FileType::Document };
                    }
                }

                // should be unreachable, as this code is only invoked if at least one drop has a path
                FileType::Folder
            }
        }
    }

    fn target_name(&self) -> String {
        match &self.action {
            FilePickerAction::AcceptShare(file) => file.name.clone(),
            FilePickerAction::DroppedFiles(drops) => {
                let drops = drops
                    .iter()
                    .filter(|d| d.path.is_some())
                    .collect::<Vec<_>>();
                let drops_count = drops.len();
                let first_drop_name = drops
                    .first()
                    .unwrap()
                    .path
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(); // what a time to be alive

                if drops_count == 1 {
                    first_drop_name
                } else {
                    format!("{} (+{} more)", first_drop_name, drops_count - 1)
                }
            }
        }
    }
}

impl super::Modal for FilePicker {
    type Response = Option<FilePickerParams>;

    fn title(&self) -> &str {
        "File Picker"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        ui.set_max_width(750.0);
        egui::ScrollArea::horizontal()
            .stick_to_right(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(350.0);
                    ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                    for (i, panel) in self.panels.clone().iter().enumerate() {
                        show_file_panel(ui, self, panel, i);
                        ui.separator();
                    }
                });
            });

        ui.separator();

        egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(20.0, 10.0))
            .show(ui, |ui| show_bottom_bar(ui, self))
            .inner
    }
}

fn show_file_panel(
    ui: &mut egui::Ui, file_picker: &mut FilePicker, panel: &Panel, file_panel_index: usize,
) {
    egui::ScrollArea::vertical()
        .id_salt(format!("{}{}", panel.root.name.clone(), file_panel_index))
        .show(ui, |ui| {
            ui.set_width(235.0);
            ui.add_space(15.0);
            ui.with_layout(
                egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
                |ui| {
                    ui.add_space(15.0);

                    let mut children: Vec<&File> = panel
                        .children
                        .iter()
                        .filter(|f| f.file_type == FileType::Folder)
                        .collect();
                    children.sort_by(|a, b| a.name.cmp(&b.name));

                    for child in children {
                        show_node(ui, file_picker, child, file_panel_index, NodeMode::Panel);
                    }
                },
            );
        });
}

fn show_bottom_bar(ui: &mut egui::Ui, file_picker: &mut FilePicker) -> Option<FilePickerParams> {
    ui.horizontal(|ui| {
        egui::ScrollArea::horizontal()
            .max_width(ui.available_width() - 100.0) // allow some room for the cta
            .show(ui, |ui| {
                for (i, f) in file_picker.panels.clone().iter().enumerate() {
                    show_node(ui, file_picker, &f.root, i, NodeMode::BottomBar);

                    ui.label(
                        egui::RichText::new(">")
                            .size(15.0)
                            .color(egui::Color32::GRAY),
                    );
                }

                let icon = match file_picker.target_type() {
                    FileType::Folder => Icon::FOLDER,
                    _ => DocType::from_name(&file_picker.target_name()).to_icon(),
                };

                icon.show(ui);

                ui.label(file_picker.target_name());
            });
        ui.spacing_mut().button_padding = egui::vec2(25.0, 5.0);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui.button("Select").clicked() {
                return Some(FilePickerParams {
                    parent: file_picker.panels.last().unwrap().root.clone(), // there's always one panel (the root), so th unwrap is safe
                    action: file_picker.action.clone(),
                });
            }
            None
        })
    })
    .inner
    .inner
}

enum NodeMode {
    Panel,
    BottomBar,
}

fn show_node(
    ui: &mut egui::Ui, file_picker: &mut FilePicker, node: &File, file_panel_index: usize,
    mode: NodeMode,
) {
    let mut icon_style = (*ui.ctx().style()).clone();
    let icon_stroke = egui::Stroke { color: ui.visuals().hyperlink_color, ..Default::default() };
    icon_style.visuals.widgets.inactive.fg_stroke = icon_stroke;
    icon_style.visuals.widgets.active.fg_stroke = icon_stroke;
    icon_style.visuals.widgets.hovered.fg_stroke = icon_stroke;

    let is_child_open = file_picker.panels.iter().any(|f| f.root.eq(node));
    let is_node_grayed_out = match mode {
        NodeMode::Panel => !is_child_open && file_panel_index != file_picker.panels.len() - 1,
        NodeMode::BottomBar => file_panel_index < file_picker.panels.len().saturating_sub(2),
    };

    if is_node_grayed_out {
        let icon_stroke = egui::Stroke {
            color: ui.visuals().hyperlink_color.linear_multiply(0.3),
            ..Default::default()
        };
        icon_style.visuals.widgets.inactive.fg_stroke = icon_stroke;
        icon_style.visuals.widgets.active.fg_stroke = icon_stroke;
        icon_style.visuals.widgets.hovered.fg_stroke = icon_stroke;

        ui.visuals_mut().widgets.inactive.fg_stroke =
            egui::Stroke { color: egui::Color32::GRAY, ..Default::default() };
    }

    if Button::default()
        .text(node.name.clone().as_str())
        .icon(&Icon::FOLDER)
        .show(ui)
        .clicked()
    {
        let drain_index = match mode {
            NodeMode::Panel => file_panel_index + 1,
            NodeMode::BottomBar => file_panel_index,
        };

        file_picker.panels.drain((drain_index)..);
        file_picker.panels.push(Panel {
            root: node.clone(),
            children: file_picker.core.get_children(&node.id).unwrap(),
        });
    };
}
