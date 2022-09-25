use crate::exhaustive_sync::experiment::ThreadID;
use crate::exhaustive_sync::trial::Action::*;
use crate::exhaustive_sync::trial::Status::{Failed, Ready, Running, Succeeded};
use crate::exhaustive_sync::utils::{find_by_name, random_filename, random_utf8};
use lockbook_core::model::errors::MoveFileError;
use lockbook_core::service::api_service::Requester;
use lockbook_core::Core;
use lockbook_core::Error::UiError;
use lockbook_shared::api::DeleteAccountRequest;
use lockbook_shared::file_metadata::FileType::{Document, Folder};
use std::time::Instant;
use std::{fs, thread};
use test_utils::*;
use uuid::Uuid;
use variant_count::VariantCount;

#[derive(VariantCount, Debug, Clone)]
pub enum Action {
    NewDocument { client: usize, parent: String, name: String },
    NewMarkdownDocument { client: usize, parent: String, name: String },
    NewFolder { client: usize, parent: String, name: String },
    UpdateDocument { client: usize, name: String, new_content: String },
    RenameFile { client: usize, name: String, new_name: String },
    MoveDocument { client: usize, doc_name: String, destination_name: String },
    AttemptFolderMove { client: usize, folder_name: String, destination_name: String },
    DeleteFile { client: usize, name: String },
    SyncAndCheck,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    Running,
    Succeeded,
    Failed(String), // Add support for re-attempts here?
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
    pub clients: Vec<Core>,
    pub target_clients: usize,
    pub target_steps: usize,
    pub steps: Vec<Action>,
    pub completed_steps: usize,
    pub status: Status,
    pub start_time: Instant,
    pub end_time: Instant,
}

impl Trial {
    fn create_clients(&mut self) -> Result<(), Status> {
        for _ in 0..self.target_clients {
            self.clients.push(test_core());
        }

        self.clients[0]
            .create_account(&random_name(), &url())
            .map_err(|err| Failed(format!("failed to create account: {:#?}", err)))?;
        let account_string = &self.clients[0].export_account().unwrap();

        for client in &self.clients[1..] {
            client
                .import_account(account_string)
                .map_err(|err| Failed(format!("{:#?}", err)))?;
            client
                .sync(None)
                .map_err(|err| Failed(format!("{:#?}", err)))?;
        }

        Ok(())
    }

    fn perform_all_known_actions(&mut self) {
        let mut additional_completed_steps = 0;
        'steps: for index in self.completed_steps..self.steps.len() {
            let step = self.steps[index].clone();
            additional_completed_steps += 1;
            match step {
                Action::NewDocument { client, parent, name } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::NewMarkdownDocument { client, parent, name } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::NewFolder { client, parent, name } => {
                    let db = self.clients[client].clone();
                    let parent = find_by_name(&db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Folder) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::UpdateDocument { client, name, new_content } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &name).id;
                    if let Err(err) = db.write_document(doc, new_content.as_bytes()) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::RenameFile { client, name, new_name } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &name).id;
                    if let Err(err) = db.rename_file(doc, &new_name) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::MoveDocument { client, doc_name, destination_name } => {
                    let db = self.clients[client].clone();
                    let doc = find_by_name(&db, &doc_name).id;
                    let dest = find_by_name(&db, &destination_name).id;

                    if let Err(err) = db.move_file(doc, dest) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::AttemptFolderMove { client, folder_name, destination_name } => {
                    let db = self.clients[client].clone();
                    let folder = find_by_name(&db, &folder_name).id;
                    let destination_folder = find_by_name(&db, &destination_name).id;

                    let move_file_result = db.move_file(folder, destination_folder);
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
                    if let Err(err) = db.delete_file(file) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                Action::SyncAndCheck => {
                    for _ in 0..2 {
                        for client in &self.clients {
                            if let Err(err) = client.sync(None) {
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
                                    row.config.writeable_path, col.config.writeable_path
                                ));
                                break 'steps;
                            }
                        }
                        if let Err(err) = row.validate() {
                            self.status = Failed(format!("Repo integrity compromised: {:#?}", err));
                            break 'steps;
                        }

                        if !row.calculate_work().unwrap().work_units.is_empty() {
                            self.status = Failed(format!(
                                "work units not empty, client: {}",
                                row.config.writeable_path
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
            let all_files = client.list_metadatas().unwrap();

            let mut folders = all_files.clone();
            folders.retain(|f| f.is_folder());

            let mut docs = all_files.clone();
            docs.retain(|f| f.is_document());

            for file in all_files {
                if file.id != file.parent {
                    mutants.push(self.create_mutation(RenameFile {
                        client: client_index,
                        name: file.name.clone(),
                        new_name: random_filename(),
                    }));

                    mutants.push(
                        self.create_mutation(DeleteFile { client: client_index, name: file.name }),
                    );
                }
            }

            for folder in folders.clone() {
                let parent_name =
                    if folder.id == folder.parent { "root".to_string() } else { folder.name };

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
                        doc_name: doc.name.clone(),
                        destination_name: parent_name.clone(),
                    }))
                }

                for folder2 in folders.clone() {
                    if folder.id != folder.parent {
                        let folder2_name = if folder2.id == folder2.parent {
                            "root".to_string()
                        } else {
                            folder2.name
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
                    name: doc.name.clone(),
                    new_content: random_utf8(),
                }));
            }
        }
        mutants.push(self.create_mutation(SyncAndCheck));

        mutants
    }

    pub fn execute(&mut self, th_id: usize) -> Vec<Trial> {
        self.start_time = Instant::now();
        self.status = if let Err(err) = self.create_clients() { err } else { Running };
        self.persist(th_id);
        let mut all_mutations = vec![];

        while self.status == Running {
            self.perform_all_known_actions();
            let mut mutations = self.generate_mutations();
            if let Some(next_action) = mutations.pop() {
                self.steps.push(next_action.steps.last().unwrap().clone());
                all_mutations.extend(mutations);
            }
        }

        self.end_time = Instant::now();
        self.cleanup();

        all_mutations
    }

    fn cleanup(&self) {
        if let Ok(account) = &self.clients[0].get_account() {
            // Delete account in server
            self.clients[0]
                .client
                .request(account, DeleteAccountRequest {})
                .unwrap_or_else(|err| {
                    println!("Failed to delete account: {} error : {:?}", account.username, err)
                });

            // Delete account locally
            for client in &self.clients {
                fs::remove_dir_all(&client.config.writeable_path).unwrap_or_else(|err| {
                    println!(
                        "failed to cleanup file: {}, error: {}",
                        client.config.writeable_path, err
                    )
                });
            }
        } else {
            eprintln!("no account to cleanup!");
        }
    }

    fn create_mutation(&self, new_action: Action) -> Trial {
        let mut clone = self.clone();
        clone.steps.push(new_action);
        clone.status = Ready;
        clone.completed_steps = 0;
        clone.start_time = Instant::now();
        clone.end_time = Instant::now();
        clone.clients = vec![];
        clone.id = Uuid::new_v4();
        clone
    }
}

impl Trial {
    pub fn file_name(&self, thread: ThreadID) -> String {
        if self.failed() {
            format!("trials/{}/{}.fail", thread, self.id)
        } else {
            format!("trials/{}/{}", thread, self.id)
        }
    }
    pub fn persist(&self, thread: ThreadID) {
        fs::write(self.file_name(thread), format!("{:#?}", self)).unwrap_or_else(|err| {
            eprintln!("Unable to write file: {}/{:?}, {:?}", thread, self, err)
        });
    }

    pub fn maybe_cleanup(&self, thread: ThreadID) {
        match self.status {
            Failed(_) => self.persist(thread),
            _ => fs::remove_file(self.file_name(thread)).unwrap_or_else(|err| {
                eprintln!("Unable to cleanup file: {}/{}, {:?}", thread, self.id, err)
            }),
        }
    }
}

impl Default for Trial {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            clients: vec![],
            target_clients: 2,
            target_steps: 7,
            steps: vec![],
            completed_steps: 0,
            status: Status::Ready,
            start_time: Instant::now(),
            end_time: Instant::now(),
        }
    }
}

impl Drop for Trial {
    fn drop(&mut self) {
        if thread::panicking() {
            println!("{} is stuck in {:?}", self.id, self.status);
        }
    }
}

impl Trial {
    pub fn failed(&self) -> bool {
        matches!(self.status, Failed(_))
    }
}
