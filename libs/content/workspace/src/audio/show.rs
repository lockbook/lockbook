use egui::{Area, Frame, Id, Order, Pos2, Ui};
use egui_player::{TranscriptionProgress, player::Player};
use lb_rs::{
    Uuid,
    blocking::Lb,
    model::{file::File, file_metadata::FileType},
};
use serde_json;
use tokio::runtime::Runtime;

/// Gets the file that pre-run transcription is in. If none present, will create a folder and file if data is provided
pub fn import_transcription(lb: &Lb, file_id: Uuid, data: Option<&[u8]>) -> Option<File> {
    let file = lb
        .get_file_by_id(file_id)
        .expect("get lockbook file for transcription");
    let siblings = lb
        .get_children(&file.parent)
        .expect("get lockbook siblings for transcription");

    let file_name = file.name;

    let imports_folder = {
        let mut imports_folder = None;
        for sibling in siblings {
            if sibling.name == "imports" {
                imports_folder = Some(sibling);
                break;
            }
        }
        if imports_folder.is_none() && data.is_none() {
            return None;
        }

        imports_folder.unwrap_or_else(|| {
            lb.create_file("imports", &file.parent, FileType::Folder)
                .expect("create lockbook folder for transcription")
        })
    };

    let imports = lb.get_children(&imports_folder.id).unwrap();
    for import in imports {
        if import.name == file_name {
            return Some(import);
        }
    }

    match data {
        Some(bytes) => {
            let file_extension = "transcript";

            let file = lb
                .create_file(
                    &format!("{file_name}.{file_extension}"),
                    &imports_folder.id,
                    FileType::Document,
                )
                .expect("create lockbook file for transcription");
            lb.write_document(file.id, bytes)
                .expect("write lockbook file for transcription");

            return Some(file);
        }
        None => {
            return None;
        }
    }
}

pub struct Audio {
    pub id: Uuid,
    pub player: Player,
    pub rt: Runtime,
    pub lb: Lb,
    pub guard: bool
}

impl Audio {
    pub fn new(id: Uuid, bytes: Vec<u8>, lb: Lb) -> Self {
        let mut player = Player::from_bytes(bytes);
        player.set_transcript_settings(egui_player::TranscriptionSettings::TranscriptLabel);
        player.set_model_download_path(lb.get_config().writeable_path);

        let potential_transcription_file = import_transcription(&lb, id, None);

        match potential_transcription_file {
            Some(trancription_file) => {
                let bytes = lb.read_document(trancription_file.id, false).unwrap();
                player.transcript = serde_json::from_slice(&bytes).unwrap();
            }
            None => {}
        }

        Audio { id, player, rt: Runtime::new().unwrap(), lb, guard: false }
    }
    pub fn show(&mut self, ui: &mut Ui) {
        Area::new(Id::new(1))
            .order(Order::Background)
            .fixed_pos(Pos2 { x: ui.max_rect().min.x, y: 50.0 })
            .show(ui.ctx(), |ui| {
                Frame::none().show(ui, |ui| {
                    self.rt.block_on(async {
                        self.player.ui(ui);
                    });
                    match self.player.transcription_progress {
                        TranscriptionProgress::Reading => {self.guard = true;}
                        TranscriptionProgress::Finished => {
                            if self.guard {
                                self.guard = false;
                                let data = serde_json::to_vec(&self.player.transcript).unwrap();
                                import_transcription(&self.lb.clone(), self.id, Some(&data));
                            }
                        }
                        _ => {}
                    }
                });
            });
    }
}
