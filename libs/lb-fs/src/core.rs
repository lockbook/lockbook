use lb_rs::{Core, File, FileType, SyncStatus, Uuid};
use std::collections::HashMap;
use tokio::task::spawn_blocking;

// todo: this is not ideal, realistically core should just get the async await treatment
#[derive(Clone)]
pub struct AsyncCore {
    core: Core,
}

impl AsyncCore {
    pub fn path() -> String {
        format!("{}/.lockbook/drive", std::env::var("HOME").unwrap())
    }

    pub fn init() -> Self {
        let writeable_path = Self::path();

        let core =
            Core::init(&lb_rs::Config { writeable_path, logs: false, colored_logs: true }).unwrap();

        Self { core }
    }

    pub fn get_root(&self) -> File {
        self.core.get_root().unwrap()
    }

    pub async fn get_by_id(&self, id: Uuid) -> File {
        let core = self.c();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn get_children(&self, id: Uuid) -> Vec<File> {
        let core = self.c();

        spawn_blocking(move || core.get_children(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn import_account(&self, account_string: &str) {
        let core = self.c();
        let account_string = account_string.to_string();

        spawn_blocking(move || core.import_account(&account_string, None).unwrap())
            .await
            .unwrap();
    }

    pub async fn read_document(&self, id: Uuid) -> Vec<u8> {
        let core = self.c();

        spawn_blocking(move || core.read_document(id).unwrap())
            .await
            .unwrap()
    }

    // the lock exists on sizes because this needs to be &self because of the trait
    pub async fn write_document(&self, id: Uuid, data: Vec<u8>) {
        let core = self.c();

        spawn_blocking(move || core.write_document(id, &data).unwrap())
            .await
            .unwrap();
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> File {
        let core = self.c();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn create_file(&self, parent: Uuid, file_type: FileType, name: String) -> File {
        let core = self.c();

        spawn_blocking(move || core.create_file(&name, parent, file_type).unwrap())
            .await
            .unwrap()
    }

    pub async fn rename_file(&self, id: Uuid, name: String) {
        let core = self.c();

        spawn_blocking(move || core.rename_file(id, &name).unwrap())
            .await
            .unwrap();
    }

    pub async fn move_file(&self, id: Uuid, parent: Uuid) {
        let core = self.c();
        spawn_blocking(move || core.move_file(id, parent).unwrap())
            .await
            .unwrap();
    }

    pub async fn remove(&self, id: Uuid) {
        let core = self.c();

        spawn_blocking(move || core.delete_file(id).unwrap())
            .await
            .unwrap();
    }

    pub async fn get_sizes(&self) -> HashMap<Uuid, usize> {
        let core = self.c();

        spawn_blocking(move || core.get_uncompressed_usage_breakdown().unwrap())
            .await
            .unwrap()
    }

    pub async fn list_metadata(&self) -> Vec<File> {
        let core = self.c();

        spawn_blocking(move || core.list_metadatas().unwrap())
            .await
            .unwrap()
    }

    pub async fn sync(&self) -> SyncStatus {
        let core = self.c();

        spawn_blocking(move || {
            core.sync(Some(Box::new(|msg| println!("{}", msg.msg))))
                .unwrap()
        })
        .await
        .unwrap()
    }

    fn c(&self) -> Core {
        self.core.clone()
    }
}
