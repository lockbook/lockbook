use std::{sync::mpsc, thread};

use lb::{blocking::Lb, service::activity::RankingWeights, Uuid};

use crate::model::DocType;
enum SuggestedUpdate {
    Error(String),
    Done(Vec<SuggestedFile>),
}

pub struct SuggestedDocs {
    update_tx: mpsc::Sender<SuggestedUpdate>,
    update_rx: mpsc::Receiver<SuggestedUpdate>,
    recs: Vec<SuggestedFile>,
    err_msg: Option<String>,
}

struct SuggestedFile {
    name: String,
    path: String,
    id: Uuid,
}

impl SuggestedDocs {
    pub fn new(core: &Lb) -> Self {
        let (update_tx, update_rx) = mpsc::channel();
        Self::calc(core, &update_tx);
        Self { update_tx, update_rx, recs: vec![], err_msg: None }
    }

    pub fn recalc_and_redraw(&mut self, ctx: &egui::Context, core: &Lb) {
        Self::calc(core, &self.update_tx);
        ctx.request_repaint();
    }

    fn calc(core: &Lb, update_tx: &mpsc::Sender<SuggestedUpdate>) {
        let core = core.clone();
        let update_tx = update_tx.clone();

        thread::spawn(move || {
            let suggested_docs = core.suggested_docs(RankingWeights::default());

            if suggested_docs.is_err() {
                update_tx
                    .send(SuggestedUpdate::Error(
                        suggested_docs
                            .map_err(|err| format!("{:?}", err))
                            .unwrap_err(),
                    ))
                    .unwrap();
            } else {
                let recs = suggested_docs
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|id| {
                        let file = core.get_file_by_id(*id);
                        if file.is_err() {
                            return None;
                        };
                        let path = core.get_path_by_id(*id).unwrap_or_default();

                        Some(SuggestedFile { name: file.unwrap().name, path, id: *id })
                    })
                    .take(10)
                    .collect();
                update_tx.send(SuggestedUpdate::Done(recs)).unwrap();
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<lb::Uuid> {
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                SuggestedUpdate::Error(err) => self.err_msg = Some(err),
                SuggestedUpdate::Done(suggested_files) => self.recs = suggested_files,
            }
        }

        if self.recs.len() < 6 {
            return None;
        }

        egui::CollapsingHeader::new("Suggested")
            .default_open(true)
            .show(ui, |ui| {
                if self.err_msg.is_some() {
                    ui.label(
                        egui::RichText::new(self.err_msg.as_ref().unwrap().to_string())
                            .color(ui.visuals().error_fg_color),
                    );
                    return None;
                }
                egui::ScrollArea::horizontal()
                    .id_source("suggested_documents")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for f in self.recs.iter() {
                                let r = egui::Frame::default()
                                    .outer_margin(egui::Margin::symmetric(10.0, 15.0))
                                    .show(ui, |ui| Self::suggested_card(ui, f));
                                if r.inner.is_some() {
                                    return r.inner;
                                }
                            }
                            None
                        })
                    })
                    .inner
                    .inner
            })
            .body_returned
            .unwrap_or_default()
    }

    fn suggested_card(ui: &mut egui::Ui, f: &SuggestedFile) -> Option<lb::Uuid> {
        let response = egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(10.0, 20.0))
            .rounding(egui::Rounding::same(5.0))
            .fill(ui.visuals().code_bg_color)
            .show(ui, |ui| {
                ui.set_min_width(130.0);
                ui.set_max_width(170.0);
                ui.vertical(|ui| {
                    DocType::from_name(&f.name).to_icon().show(ui);
                    ui.horizontal_wrapped(|ui| {
                        let mut job = egui::text::LayoutJob::single_section(
                            f.name.clone(),
                            egui::TextFormat::simple(
                                egui::FontId::proportional(18.0),
                                ui.visuals().text_color(),
                            ),
                        );

                        job.wrap = egui::epaint::text::TextWrapping {
                            overflow_character: Some('…'),
                            max_rows: 1,
                            break_anywhere: true,
                            ..Default::default()
                        };
                        ui.label(job);
                    });

                    let path_parent_index = f.path.rfind('/').unwrap_or_default();
                    let path: String = f.path.chars().take(path_parent_index).collect();

                    ui.horizontal_wrapped(|ui| {
                        let mut job = egui::text::LayoutJob::single_section(
                            path,
                            egui::TextFormat::simple(
                                egui::FontId::proportional(15.0),
                                egui::Color32::GRAY,
                            ),
                        );

                        job.wrap = egui::epaint::text::TextWrapping {
                            overflow_character: Some('…'),
                            max_rows: 1,
                            break_anywhere: true,
                            ..Default::default()
                        };
                        ui.label(job);
                    });
                });
            })
            .response;

        let response = ui.interact(
            response.rect,
            egui::Id::from(format!("suggested_card_{}", f.path)),
            egui::Sense::click(),
        );
        if response.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            return Some(f.id);
        }
        None
    }
}
