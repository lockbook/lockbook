use std::{sync::mpsc, thread};

use lb::{blocking::Lb, service::activity::RankingWeights, Uuid};

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
}
