use lb::blocking::Lb;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

use workspace_rs::show::DocType;

pub struct AcceptShareModal {
    requests: Vec<File>,
    username: String,
}

pub struct AcceptShareParams {
    pub target: File,
    pub is_accept: bool,
}

impl AcceptShareModal {
    pub fn new(core: &Lb) -> Self {
        Self {
            requests: core.get_pending_shares().unwrap_or_default(),
            username: core.get_account().unwrap().username.clone(),
        }
    }
}

impl super::Modal for AcceptShareModal {
    type Response = Option<AcceptShareParams>;

    fn title(&self) -> &str {
        "Incoming Share Requests"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let max_height = 400.0;
        ui.set_max_height(max_height);

        ui.add_space(20.0);
        let response = egui::ScrollArea::vertical()
            .max_height(max_height) // set the max height on both the container and scrollarea to avoid weird layout shifts
            .show(ui, |ui| {
                for req in self.requests.iter() {
                    let response = sharer_info(ui, req, self.username.clone());
                    if response.is_some() {
                        return response;
                    }
                }
                None
            })
            .inner;

        if self.requests.is_empty() {
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                Icon::EMPTY_INBOX.size(60.0).show(ui);
                ui.add_space(20.0);
                ui.heading("You have no incoming shares");
                ui.label(
                    egui::RichText::new("Your friends can share their notes with you here")
                        .size(15.0)
                        .color(egui::Color32::GRAY),
                );
            });
            ui.add_space(10.0);
        }

        ui.add_space(20.0);

        response
    }
}
fn sharer_info(ui: &mut egui::Ui, req: &File, username: String) -> Option<AcceptShareParams> {
    egui::Frame::default()
        .fill(ui.style().visuals.faint_bg_color)
        .stroke(egui::Stroke { width: 0.1, color: ui.visuals().text_color() })
        .inner_margin(egui::Margin::same(15.0))
        .outer_margin(egui::Margin::symmetric(25.0, 12.0))
        .rounding(egui::Rounding::same(5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = match req.file_type {
                    FileType::Folder => Icon::FOLDER,
                    _ => DocType::from_name(&req.name).to_icon(),
                };

                let sharer = req
                    .shares
                    .iter()
                    .find(|s| s.shared_with == username)
                    .unwrap()
                    .clone()
                    .shared_by;

                icon.size(40.0).show(ui);
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    ui.label(&req.name);

                    ui.label(
                        egui::RichText::new(format!("shared by {sharer}"))
                            .size(15.0)
                            .color(egui::Color32::GRAY),
                    );
                });

                let others_with_access = req
                    .shares
                    .iter()
                    .filter(|f| f.shared_with != sharer && f.shared_with != username)
                    .map(|s| s.shared_with.clone())
                    .collect::<Vec<String>>();

                if !others_with_access.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        Icon::GROUP
                            .color(egui::Color32::GRAY)
                            .show(ui)
                            .on_hover_text(format!(
                                "also shared with {}",
                                others_with_access.join(", ")
                            ))
                    });
                }
            });

            ui.add_space(30.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let button_stroke =
                    egui::Stroke { color: ui.visuals().hyperlink_color, ..Default::default() };
                ui.visuals_mut().widgets.inactive.fg_stroke = button_stroke;
                ui.visuals_mut().widgets.hovered.fg_stroke = button_stroke;
                ui.visuals_mut().widgets.active.fg_stroke = button_stroke;

                if Button::default().text("Accept").show(ui).clicked() {
                    return Some(AcceptShareParams { target: req.clone(), is_accept: true });
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.visuals_mut().widgets.inactive.fg_stroke =
                        egui::Stroke { color: egui::Color32::GRAY, ..Default::default() };
                    ui.visuals_mut().widgets.hovered.fg_stroke =
                        egui::Stroke { color: ui.visuals().error_fg_color, ..Default::default() };
                    ui.visuals_mut().widgets.active.fg_stroke =
                        egui::Stroke { color: ui.visuals().error_fg_color, ..Default::default() };

                    if Button::default()
                        .text("Del ")
                        .icon(&Icon::DELETE)
                        .show(ui)
                        .clicked()
                    {
                        return Some(AcceptShareParams { target: req.clone(), is_accept: false });
                    }
                    None
                })
                .inner
            })
        })
        .inner
        .inner
}
