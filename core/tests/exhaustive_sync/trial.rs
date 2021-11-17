use std::{fs, thread};

use uuid::Uuid;
use variant_count::VariantCount;

use lockbook_core::model::state::Config;
use lockbook_core::service::integrity_service::test_repo_integrity;
use lockbook_core::service::test_utils::{dbs_equal, random_username, test_config, url};
use lockbook_core::Error::UiError;
use lockbook_core::{
    calculate_work, create_account, delete_file, export_account, import_account, move_file,
    rename_file, sync_all, write_document, MoveFileError,
};
use lockbook_core::{create_file, list_metadatas};
use lockbook_crypto::clock_service::get_time;
use lockbook_models::file_metadata::FileType::{Document, Folder};

use crate::exhaustive_sync::trial::Action::{
    AttemptFolderMove, DeleteFile, MoveDocument, NewDocument, NewFolder, NewMarkdownDocument,
    RenameFile, SyncAndCheck, UpdateDocument,
};
use crate::exhaustive_sync::trial::Status::{Failed, Ready, Running, Succeeded};
use crate::exhaustive_sync::utils::{find_by_name, random_filename, random_utf8};

#[derive(VariantCount, Debug, Clone)]
pub enum Action {
    NewDocument {
        client: usize,
        parent: String,
        name: String,
    },
    NewMarkdownDocument {
        client: usize,
        parent: String,
        name: String,
    },
    NewFolder {
        client: usize,
        parent: String,
        name: String,
    },
    UpdateDocument {
        client: usize,
        name: String,
        new_content: String,
    },
    RenameFile {
        client: usize,
        name: String,
        new_name: String,
    },
    MoveDocument {
        client: usize,
        doc_name: String,
        destination_name: String,
    },
    AttemptFolderMove {
        client: usize,
        folder_name: String,
        destination_name: String,
    },
    DeleteFile {
        client: usize,
        name: String,
    },
    SyncAndCheck,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Ready,
    Running,
    Succeeded,
    Failed(String),
}

impl Status {
    pub fn failed(&self) -> bool {
        match self {
            Ready | Running | Succeeded => false,
            Failed(_) => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Trial {
    pub id: Uuid,
    pub clients: Vec<Config>,
    pub target_clients: usize,
    pub target_steps: usize,
    pub steps: Vec<Action>,
    pub completed_steps: usize,
    pub status: Status,
    pub start_time: i64,
    pub end_time: i64,
}

impl Drop for Trial {
    fn drop(&mut self) {
        if thread::panicking() {
            println!("{} is stuck in {:?}", self.id, self.status);
        }
    }
}

impl Trial {
    fn create_clients(&mut self) -> Result<(), Status> {
        for _ in 0..self.target_clients {
            self.clients.push(test_config());
        }

        create_account(&self.clients[0], &random_username(), &url())
            .map_err(|err| Failed(format!("{:#?}", err)))?;
        let account_string = export_account(&self.clients[0]).unwrap();

        for client in &self.clients[1..] {
            import_account(&client, &account_string)
                .map_err(|err| Failed(format!("{:#?}", err)))?;
            sync_all(&client, None).map_err(|err| Failed(format!("{:#?}", err)))?;
        }

        Ok(())
    }

    fn perform_all_known_actions(&mut self) {
        let mut additional_completed_steps = 0;
        'steps: for index in self.completed_steps..self.steps.len() {
            let step = self.steps[index].clone();
            additional_completed_steps += 1;
            match step {
                Action::NewDocument {
                    client,
                    parent,
                    name,
                } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = create_file(&db, &name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::NewMarkdownDocument {
                    client,
                    parent,
                    name,
                } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = create_file(&db, &name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::NewFolder {
                    client,
                    parent,
                    name,
                } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = create_file(&db, &name, parent, Folder) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::UpdateDocument {
                    client,
                    name,
                    new_content,
                } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &name).id;
                    if let Err(err) = write_document(&db, doc, new_content.as_bytes()) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::RenameFile {
                    client,
                    name,
                    new_name,
                } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &name).id;
                    if let Err(err) = rename_file(&db, doc, &new_name) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::MoveDocument {
                    client,
                    doc_name,
                    destination_name,
                } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &doc_name).id;
                    let dest = find_by_name(&db, &destination_name).id;

                    if let Err(err) = move_file(&db, doc, dest) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::AttemptFolderMove {
                    client,
                    folder_name,
                    destination_name,
                } => {
                    let db = self.clients[client].clone();
                    let folder = find_by_name(&db, &folder_name).id;
                    let destination_folder = find_by_name(&db, &destination_name).id;

                    let move_file_result = move_file(&db, folder, destination_folder);
                    match move_file_result {
                        Ok(()) | Err(UiError(MoveFileError::FolderMovedIntoItself)) => {}
                        Err(err) => {
                            self.status = Failed(format!("{:#?}", err));
                            break 'steps;
                        }
                    }
                }
                Action::DeleteFile { client, name } => {
                    let db = self.clients[client].clone();
                    let file = find_by_name(&db, &name).id;
                    if let Err(err) = delete_file(&db, file) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::SyncAndCheck => {
                    for _ in 0..2 {
                        for client in &self.clients {
                            if let Err(err) = sync_all(&client, None) {
                                self.status = Failed(format!("{:#?}", err));
                                break 'steps;
                            }
                        }
                    }

                    for row in &self.clients {
                        for col in &self.clients {
                            if !dbs_equal(row, col) {
                                self.status = Failed(format!(
                                    "db {} is not equal to {} after a sync",
                                    row.writeable_path, col.writeable_path
                                ));
                                break 'steps;
                            }
                        }
                        if let Err(err) = test_repo_integrity(row) {
                            self.status = Failed(format!("Repo integrity compromised: {:#?}", err));
                            break 'steps;
                        }

                        if !calculate_work(row).unwrap().work_units.is_empty() {
                            self.status = Failed(format!(
                                "work units not empty, client: {}",
                                row.writeable_path
                            ));
                            break 'steps;
                        }
                    }
                }
            }
        }
        self.completed_steps += additional_completed_steps;
        if self.status == Running && self.completed_steps == self.target_steps {
            self.status = Succeeded;
        }
    }

    fn generate_mutations(&self) -> Vec<Trial> {
        let mut mutants: Vec<Trial> = vec![];

        if self.status != Running {
            return mutants;
        }

        for client_index in 0..self.clients.len() {
            let client = self.clients[client_index].clone();
            let all_files = list_metadatas(&client).unwrap();

            let mut folders = all_files.clone();
            folders.retain(|f| f.file_type == Folder);

            let mut docs = all_files.clone();
            docs.retain(|f| f.file_type == Document);

            for file in all_files {
                if file.id != file.parent {
                    mutants.push(self.create_mutation(RenameFile {
                        client: client_index,
                        name: file.decrypted_name.clone(),
                        new_name: random_filename(),
                    }));

                    mutants.push(self.create_mutation(DeleteFile {
                        client: client_index,
                        name: file.decrypted_name,
                    }));
                }
            }

            for folder in folders.clone() {
                let parent_name = if folder.id == folder.parent {
                    "root".to_string()
                } else {
                    folder.decrypted_name
                };

                mutants.push(self.create_mutation(NewDocument {
                    parent: parent_name.clone(),
                    client: client_index,
                    name: random_filename(),
                }));

                mutants.push(self.create_mutation(NewMarkdownDocument {
                    parent: parent_name.clone(),
                    client: client_index,
                    name: random_filename() + ".md",
                }));

                mutants.push(self.create_mutation(NewFolder {
                    parent: parent_name.clone(),
                    client: client_index,
                    name: random_filename(),
                }));

                for doc in docs.clone() {
                    mutants.push(self.create_mutation(MoveDocument {
                        client: client_index,
                        doc_name: doc.decrypted_name.clone(),
                        destination_name: parent_name.clone(),
                    }))
                }

                for folder2 in folders.clone() {
                    if folder.id != folder.parent {
                        let folder2_name = if folder2.id == folder2.parent {
                            "root".to_string()
                        } else {
                            folder2.decrypted_name
                        };
                        mutants.push(self.create_mutation(AttemptFolderMove {
                            client: client_index,
                            folder_name: parent_name.clone(),
                            destination_name: folder2_name,
                        }))
                    }
                }
            }

            for doc in docs.clone() {
                mutants.push(self.create_mutation(UpdateDocument {
                    client: client_index,
                    name: doc.decrypted_name.clone(),
                    new_content: random_utf8(),
                }));
            }
        }
        mutants.push(self.create_mutation(SyncAndCheck));

        mutants
    }

    pub fn execute(&mut self) -> Vec<Trial> {
        self.start_time = get_time().0;
        self.status = if let Err(err) = self.create_clients() {
            err
        } else {
            Running
        };

        let mut all_mutations = vec![];

        while self.status == Running {
            self.perform_all_known_actions();
            let mut mutations = self.generate_mutations();
            if let Some(next_action) = mutations.pop() {
                self.steps.push(next_action.steps.last().unwrap().clone());
                all_mutations.extend(mutations);
            }
        }

        for client in &self.clients {
            fs::remove_dir_all(&client.writeable_path).unwrap_or_else(|err| {
                println!(
                    "failed to cleanup file: {}, error: {}",
                    client.writeable_path, err
                )
            });
        }

        self.end_time = get_time().0;
        all_mutations
    }

    fn create_mutation(&self, new_action: Action) -> Trial {
        let mut clone = self.clone();
        clone.steps.push(new_action);
        clone.status = Ready;
        clone.completed_steps = 0;
        clone.start_time = 0;
        clone.end_time = 0;
        clone.clients = vec![];
        clone.id = Uuid::new_v4();
        clone
    }
}
