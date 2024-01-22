use std::{collections::HashMap, sync::Mutex};

use lb_rs::{Core, File, FileType, Uuid};
use nfsserve::nfs::{fattr3, fileid3, filename3, ftype3};
use tokio::task::spawn_blocking;

// todo: this is not ideal, realistically core should just get the async await treatment
pub struct AsyncCore {
    core: Core,

    f3_uid: Mutex<HashMap<fileid3, Uuid>>,
    sizes: Mutex<HashMap<Uuid, usize>>,
}

impl AsyncCore {
    pub fn init() -> Self {
        let writeable_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

        let core =
            Core::init(&lb_rs::Config { writeable_path, logs: true, colored_logs: true }).unwrap();

        let mut ac = Self { core, f3_uid: Default::default(), sizes: Default::default() };

        if ac.core.get_account().is_ok() {
            println!("preparing cache (are you in a release build?)");
            let files = ac.core.list_metadatas().unwrap();
            let sizes = ac.core.get_uncompressed_usage_breakdown().unwrap();
            ac.populate_caches(&files, sizes);
            println!("cache prepared");
        }

        ac
    }

    pub fn get_root(&self) -> File {
        self.core.get_root().unwrap()
    }

    pub fn populate_caches(&mut self, files: &[File], sizes: HashMap<Uuid, usize>) {
        for file in files {
            let id = file.id;
            let first_part = id.as_u64_pair().0;
            self.f3_uid.lock().unwrap().insert(first_part, file.id);
        }

        *self.sizes.lock().unwrap() = sizes;
    }

    pub async fn get_all_files(&self) -> Vec<File> {
        let core = self.c();
        spawn_blocking(move || core.list_metadatas().unwrap())
            .await
            .unwrap()
    }

    pub async fn get_by_id(&self, id: fileid3) -> File {
        let core = self.c();
        let id = *self.f3_uid.lock().unwrap().get(&id).unwrap();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn get_children(&self, id: fileid3) -> Vec<File> {
        let core = self.c();
        let id = *self.f3_uid.lock().unwrap().get(&id).unwrap();

        spawn_blocking(move || core.get_children(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn import_account(&self, account_string: &str) {
        let core = self.c();
        let account_string = account_string.to_string();

        spawn_blocking(move || core.import_account(&account_string).unwrap())
            .await
            .unwrap();
    }

    pub async fn read_document(&self, id: fileid3) -> Vec<u8> {
        let core = self.c();
        let id = *self.f3_uid.lock().unwrap().get(&id).unwrap();

        spawn_blocking(move || core.read_document(id).unwrap())
            .await
            .unwrap()
    }

    // the lock exists on sizes because this needs to be &self because of the trait
    pub async fn write_document(&self, id: fileid3, data: Vec<u8>) {
        let core = self.c();
        let id = *self.f3_uid.lock().unwrap().get(&id).unwrap();
        let new_size = data.len();

        spawn_blocking(move || core.write_document(id, &data).unwrap())
            .await
            .unwrap();

        self.sizes.lock().unwrap().insert(id, new_size);
    }

    pub async fn get_file_by_id(&self, id: fileid3) -> File {
        let core = self.c();
        let id = *self.f3_uid.lock().unwrap().get(&id).unwrap();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn create_file(&self, parent: fileid3, file_type: FileType, name: String) -> File {
        let core = self.c();
        let parent = *self.f3_uid.lock().unwrap().get(&parent).unwrap();

        let file = spawn_blocking(move || core.create_file(&name, parent, file_type).unwrap())
            .await
            .unwrap();

        if file_type == FileType::Document {
            self.sizes.lock().unwrap().insert(file.id, 0);
        }

        self.f3_uid
            .lock()
            .unwrap()
            .insert(file.id.as_u64_pair().0, file.id);

        file
    }

    pub async fn rename_file(&self, id: Uuid, name: String) {
        let core = self.c();

        spawn_blocking(move || core.rename_file(id, &name).unwrap())
            .await
            .unwrap();
    }

    pub async fn move_file(&self, id: Uuid, parent: fileid3) {
        let parent = *self.f3_uid.lock().unwrap().get(&parent).unwrap();
        let core = self.c();
        spawn_blocking(move || core.move_file(id, parent).unwrap())
            .await
            .unwrap();
    }

    pub async fn sync(&self) {
        let core = self.c();
        // todo figure out logging more generally here and in the platform
        // todo also generally figure out error handling
        spawn_blocking(move || {
            core.sync(Some(Box::new(|msg| println!("{}", msg.msg))))
                .unwrap()
        })
        .await
        .unwrap();
    }

    pub fn file_to_fattr(&self, f: &File) -> fattr3 {
        let ftype = if f.is_folder() { ftype3::NF3DIR } else { ftype3::NF3REG };

        // todo this deserves some scrutiny and cross platform testing
        let mode = if f.is_folder() { 0o755 } else { 0o644 };

        let fileid = f.id.as_u64_pair().0;
        // intereREADDIR3resstingly a number of key read operations rely on this being correct
        let size =
            if f.is_folder() { 0 } else { *self.sizes.lock().unwrap().get(&f.id).unwrap() as u64 };

        fattr3 {
            ftype,
            mode,
            nlink: Default::default(), // hard links to this file
            uid: 501,                  // owner field? not resolved by this lib
            gid: 20,                   // group id
            size,
            used: size,               // ?
            rdev: Default::default(), // ?
            fsid: Default::default(), // file system id
            fileid,
            atime: Default::default(), // access time
            mtime: Default::default(), // todo modify time could populate with last_updated
            ctime: Default::default(), // create time
        }
    }

    fn c(&self) -> Core {
        self.core.clone()
    }
}
