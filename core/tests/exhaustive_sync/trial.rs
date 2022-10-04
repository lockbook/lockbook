use crate::exhaustive_sync::experiment::ThreadID;
use crate::exhaustive_sync::trial::Action::*;
use crate::exhaustive_sync::trial::Status::{Failed, Ready, Running, Succeeded};
use crate::exhaustive_sync::utils::{find_by_name, random_filename, random_utf8};
use lockbook_core::model::errors::MoveFileError;
use lockbook_core::service::api_service::no_network::{CoreIP, InProcess};
use lockbook_core::Error::UiError;
use lockbook_server_lib::config::AdminConfig;
use lockbook_shared::file_metadata::FileType::{Document, Folder};
use std::fmt::{Debug, Formatter};
use std::time::Instant;
use std::{fs, thread};
use test_utils::*;
use uuid::Uuid;
use variant_count::VariantCount;

#[derive(VariantCount, Debug, Clone)]
pub enum Action {
    NewDocument { user_index: usize, device_index: usize, parent: String, name: String },
    NewMarkdownDocument { user_index: usize, device_index: usize, parent: String, name: String },
    NewFolder { user_index: usize, device_index: usize, parent: String, name: String },
    UpdateDocument { user_index: usize, device_index: usize, name: String, new_content: String },
    RenameFile { user_index: usize, device_index: usize, name: String, new_name: String },
    MoveDocument { user_index: usize, device_index: usize, doc_name: String, destination_name: String },
    AttemptFolderMove { user_index: usize, device_index: usize, folder_name: String, destination_name: String },
    DeleteFile { user_index: usize, device_index: usize, name: String },
    SyncAndCheck,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone)]
pub struct Trial {
    pub id: Uuid,
    pub devices_by_user: Vec<Vec<CoreIP>>,
    pub target_devices_by_user: Vec<usize>,
    pub target_steps: usize,
    pub steps: Vec<Action>,
    pub completed_steps: usize,
    pub status: Status,
    pub start_time: Instant,
    pub end_time: Instant,
}

impl Debug for Trial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = &mut f.debug_struct("Trial");

        result = result.field("id", &self.id);
        result = result.field("target_clients", &self.target_devices_by_user);
        result = result.field("target_steps", &self.target_steps);
        result = result.field("steps", &self.steps);
        result = result.field("completed_steps", &self.completed_steps);
        result = result.field("status", &self.status);
        result = result.field("start_time", &self.start_time);
        result = result.field("end_time", &self.end_time);

        result.finish()
    }
}

impl Trial {
    fn create_clients(&mut self) -> Result<(), Status> {
        let mut usernames = Vec::new();
        for _user_index in 0..self.target_devices_by_user.len() {
            usernames.push(random_name());
        }
        let server = InProcess::init(
            test_config(),
            AdminConfig { admins: usernames.iter().cloned().collect() },
        );
        for user_index in 0..self.target_devices_by_user.len() {
            let mut devices_by_user = Vec::new();
            let mut maybe_account_string: Option<String> = None;
            for _device_index in 0..self.target_devices_by_user[user_index] {
                let device = CoreIP::init_in_process(&test_config(), server.clone());
                if let Some(ref account_string) = maybe_account_string {
                    device
                        .import_account(account_string)
                        .map_err(|err| Failed(format!("{:#?}", err)))?;
                    device
                        .sync(None)
                        .map_err(|err| Failed(format!("{:#?}", err)))?;
                } else {
                    device.create_account(&usernames[user_index], &url()).map_err(|err| Failed(format!("failed to create account: {:#?}", err)))?;
                    maybe_account_string = Some(device.export_account().unwrap());
                }
                devices_by_user.push(device);
            }
            self.devices_by_user.push(devices_by_user);
        }

        Ok(())
    }

    fn perform_all_known_actions(&mut self) {
        let mut additional_completed_steps = 0;
        'steps: for index in self.completed_steps..self.steps.len() {
            let step = self.steps[index].clone();
            additional_completed_steps += 1;
            match step {
                NewDocument { user_index, device_index, parent, name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let parent = find_by_name(db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                NewMarkdownDocument { user_index, device_index, parent, name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let parent = find_by_name(db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Document) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                NewFolder { user_index, device_index, parent, name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let parent = find_by_name(db, &parent).id;
                    if let Err(err) = db.create_file(&name, parent, Folder) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                UpdateDocument { user_index, device_index, name, new_content } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let doc = find_by_name(db, &name).id;
                    if let Err(err) = db.write_document(doc, new_content.as_bytes()) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                RenameFile { user_index, device_index, name, new_name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let doc = find_by_name(db, &name).id;
                    if let Err(err) = db.rename_file(doc, &new_name) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                MoveDocument { user_index, device_index, doc_name, destination_name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let doc = find_by_name(db, &doc_name).id;
                    let dest = find_by_name(db, &destination_name).id;

                    if let Err(err) = db.move_file(doc, dest) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                AttemptFolderMove { user_index, device_index, folder_name, destination_name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let folder = find_by_name(db, &folder_name).id;
                    let destination_folder = find_by_name(db, &destination_name).id;

                    let move_file_result = db.move_file(folder, destination_folder);
                    match move_file_result {
                        Ok(()) | Err(UiError(MoveFileError::FolderMovedIntoItself)) => {}
                        Err(err) => {
                            self.status = Failed(format!("{:#?}", err));
                            break 'steps;
                        }
                    }
                }
                DeleteFile { user_index, device_index, name } => {
                    let db = &self.devices_by_user[user_index][device_index];
                    let file = find_by_name(db, &name).id;
                    if let Err(err) = db.delete_file(file) {
                        self.status = Failed(format!("{:#?}", err));
                        break 'steps;
                    }
                }
                SyncAndCheck => {
                    for _ in 0..2 {
                        for user_index in 0..self.target_devices_by_user.len() {
                            for device_index in 0..self.target_devices_by_user[user_index] {
                                let device = &self.devices_by_user[user_index][device_index];
                                if let Err(err) = device.sync(None) {
                                    self.status = Failed(format!("{:#?}", err));
                                    break 'steps;
                                }
                            }
                        }
                    }

                    for user_index in 0..self.target_devices_by_user.len() {
                        for device_index in 0..self.target_devices_by_user[user_index] {
                            let device = &self.devices_by_user[user_index][device_index];
                            if let Err(err) = device.validate() {
                                self.status = Failed(format!("Repo integrity compromised: {:#?}", err));
                                break 'steps;
                            }

                            if !device.calculate_work().unwrap().work_units.is_empty() {
                                self.status = Failed(format!(
                                    "work units not empty, client: {}",
                                    device.config.writeable_path
                                ));
                                break 'steps;
                            }

                            for compare_device_index in 0..self.target_devices_by_user[user_index] {
                                let compare_device = &self.devices_by_user[user_index][compare_device_index];
                                assert_dbs_equal(device, compare_device);
                                if !dbs_equal(device, compare_device) {
                                    self.status = Failed(format!(
                                        "db {} is not equal to {} after a sync. Server is {} & {}",
                                        device.config.writeable_path,
                                        compare_device.config.writeable_path,
                                        device.client.config.writeable_path,
                                        compare_device.client.config.writeable_path
                                    ));
                                    break 'steps;
                                }
                            }
                        }
                    }

                    match self.devices_by_user[0][0].admin_validate_server() {
                        Ok(validations) => {
                            if validations != Default::default() {
                                self.status = Failed(format!(
                                    "Server reported validation failures: {:#?}",
                                    validations
                                ));
                                break 'steps;
                            }
                        }
                        Err(err) => {
                            self.status = Failed(format!("{:#?}", err));
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

        for user_index in 0..self.target_devices_by_user.len() {
            for device_index in 0..self.target_devices_by_user[user_index] {
                let client = &self.devices_by_user[user_index][device_index];
                let all_files = client.list_metadatas().unwrap();

                let mut folders = all_files.clone();
                folders.retain(|f| f.is_folder());

                let mut docs = all_files.clone();
                docs.retain(|f| f.is_document());

                for file in all_files {
                    if file.id != file.parent {
                        mutants.push(self.create_mutation(RenameFile {
                            user_index,
                            device_index,
                            name: file.name.clone(),
                            new_name: random_filename(),
                        }));

                        mutants.push(
                            self.create_mutation(DeleteFile {
                                user_index,
                                device_index, name: file.name }),
                        );
                    }
                }

                for folder in folders.clone() {
                    let parent_name =
                        if folder.id == folder.parent { "root".to_string() } else { folder.name };

                    mutants.push(self.create_mutation(NewDocument {
                        user_index,
                        device_index,
                        parent: parent_name.clone(),
                        name: random_filename(),
                    }));

                    mutants.push(self.create_mutation(NewMarkdownDocument {
                        user_index,
                        device_index,
                        parent: parent_name.clone(),
                        name: random_filename() + ".md",
                    }));

                    mutants.push(self.create_mutation(NewFolder {
                        user_index,
                        device_index,
                        parent: parent_name.clone(),
                        name: random_filename(),
                    }));

                    for doc in docs.clone() {
                        mutants.push(self.create_mutation(MoveDocument {
                            user_index,
                            device_index,
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
                                user_index,
                                device_index,
                                folder_name: parent_name.clone(),
                                destination_name: folder2_name,
                            }))
                        }
                    }
                }

                for doc in docs.clone() {
                    mutants.push(self.create_mutation(UpdateDocument {
                        user_index,
                        device_index,
                        name: doc.name.clone(),
                        new_content: random_utf8(),
                    }));
                }
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
        // Delete server
        fs::remove_dir_all(&self.devices_by_user[0][0].client.config.writeable_path).unwrap_or_else(|err| {
            println!(
                "failed to cleanup file: {}, error: {}",
                &self.devices_by_user[0][0].client.config.writeable_path, err
            )
        });

        // Delete account locally
        for user_index in 0..self.target_devices_by_user.len() {
            for device_index in 0..self.target_devices_by_user[user_index] {
                let client = &self.devices_by_user[user_index][device_index];
                fs::remove_dir_all(&client.config.writeable_path).unwrap_or_else(|err| {
                    println!("failed to cleanup file: {}, error: {}", client.config.writeable_path, err)
                });
            }
        }
    }

    fn create_mutation(&self, new_action: Action) -> Trial {
        let mut clone = self.clone();
        clone.steps.push(new_action);
        clone.status = Ready;
        clone.completed_steps = 0;
        clone.start_time = Instant::now();
        clone.end_time = Instant::now();
        clone.devices_by_user = vec![];
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
            devices_by_user: vec![],
            target_devices_by_user: vec![2, 2],
            target_steps: 10,
            steps: vec![],
            completed_steps: 0,
            status: Ready,
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
