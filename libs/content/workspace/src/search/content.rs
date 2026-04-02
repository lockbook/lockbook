use std::time::Instant;

use lb_rs::model::file::File;

use crate::search::SearhExecutor;

pub struct ContentSearch {
}

impl SearhExecutor for ContentSearch {
    fn search_type(&self) -> super::SearchType {
        todo!()
    }

    fn handle_query(&mut self, query: &str) {
        todo!()
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) {
        todo!()
    }

    fn show_preview(&mut self, ui: &mut egui::Ui) {
        todo!()
    }
}

impl ContentSearch {
    pub fn new() -> Self {
        Self {} 
    }
}


#[derive(Default)]
pub struct ContentIngestState {
    ingest_start: Option<Instant>,
    uningested_files: Vec<File>,
    files_ingested: u32,
    ingest_target: u32,
    ignored_files: u32,
    ingest_end: Option<Instant>,
}

//                     for meta in metas {
//                         if !meta.is_document() {
//                             continue;
//                         }
// 
//                         if !meta.name.ends_with(".md") {
//                             ingest_state.ignored_files += 1;
//                             continue;
//                         }
// 
//                         ingest_state.uningested_files.push(meta);
//                     }
// 
//                     ingest_state.ingest_target = ingest_state.uningested_files.len() as u32;
// 
//                     let s = SearchSession {
//                         search_type: SearchType::Content,
//                         engine: Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1),
//                         submitted_query: String::new(),
//                         ingest_state: Arc::new(RwLock::new(ingest_state)),
//                     };
// 
//                     let injector = s.engine.injector();
// 
//                     for _ in 0..available_parallelism().map(|p| p.get()).unwrap_or(4) {
//                         let ingest = s.ingest_state.clone();
//                         let lb = self.core.clone();
//                         let injector = injector.clone();
//                         let id_paths = id_paths.clone();
//                         thread::spawn(move || {
//                             let mut unlocked = ingest.write().unwrap();
//                             let Some(meta) = unlocked.uningested_files.pop() else {
//                                 return;
//                             };
//                             drop(unlocked);
// 
//                             let id = meta.id;
//                             let doc = lb
//                                 .read_document(id, false)
//                                 .ok()
//                                 .and_then(|bytes| String::from_utf8(bytes).ok());
// 
//                             let success = match doc {
//                                 Some(doc) => {
//                                     injector.push(
//                                         Entry {
//                                             file: meta,
//                                             path: id_paths
//                                                 .iter()
//                                                 .find(|(i, _)| *i == id)
//                                                 .map(|(_, path)| path)
//                                                 .unwrap()
//                                                 .clone(),
//                                             matched_region: vec![],
//                                         },
//                                         |e, cols| {},
//                                     );
//                                     true
//                                 }
//                                 None => false,
//                             };
// 
//                             let mut unlocked = ingest.write().unwrap();
//                             if success {
//                                 unlocked.files_ingested += 1;
//                             } else {
//                                 unlocked.ignored_files += 1;
//                             }
// 
//                             if unlocked.uningested_files.is_empty() {
//                                 unlocked.ingest_end = Some(Instant::now());
//                             }
//                         });
//                     }
//                 }
//  
