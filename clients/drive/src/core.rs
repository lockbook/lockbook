use std::collections::HashMap;

use lb_rs::{Core, File, Uuid};
use nfsserve::nfs::{fattr3, fileid3, ftype3};
use tokio::task::spawn_blocking;

// todo: this is not ideal, realistically core should just get the async await treatment
pub struct AsyncCore {
    core: Core,

    f3_uid: HashMap<fileid3, Uuid>,
}

impl AsyncCore {
    pub fn init() -> Self {
        let writeable_path = format!("{}/.lockbook/drive", std::env::var("HOME").unwrap());

        let core =
            Core::init(&lb_rs::Config { writeable_path, logs: true, colored_logs: true }).unwrap();

        let mut ac = Self { core, f3_uid: Default::default() };

        let files = ac.core.list_metadatas().unwrap();

        ac.populate_caches(&files);

        ac
    }

    pub fn get_root(&self) -> File {
        self.core.get_root().unwrap()
    }

    pub fn populate_caches(&mut self, files: &[File]) {
        for file in files {
            let id = file.id;
            let first_part = id.as_u64_pair().0;
            self.f3_uid.insert(first_part, file.id);
        }
    }

    pub async fn get_all_files(&self) -> Vec<File> {
        let core = self.c();
        spawn_blocking(move || core.list_metadatas().unwrap())
            .await
            .unwrap()
    }

    pub async fn get_by_id(&self, id: fileid3) -> File {
        let core = self.c();
        let id = *self.f3_uid.get(&id).unwrap();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
    }

    pub async fn get_children(&self, id: fileid3) -> Vec<File> {
        let core = self.c();
        let id = *self.f3_uid.get(&id).unwrap();

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

    pub async fn get_file_by_id(&self, id: fileid3) -> File {
        let core = self.c();
        let id = *self.f3_uid.get(&id).unwrap();

        spawn_blocking(move || core.get_file_by_id(id).unwrap())
            .await
            .unwrap()
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

    fn c(&self) -> Core {
        self.core.clone()
    }
}

pub fn file_to_attr(f: File) -> fattr3 {
    let ftype = if f.is_folder() { ftype3::NF3DIR } else { ftype3::NF3REG };
    let fileid = f.id.as_u64_pair().0;

    fattr3 {
        ftype,
        mode: Default::default(), // file protection bit (could be used for locking)
        nlink: Default::default(), // hard links to this file
        uid: Default::default(),  // owner field? not resolved by this lib
        gid: Default::default(),  // group id
        size: Default::default(), // todo: compute via cache
        used: Default::default(), // ?
        rdev: Default::default(), // ?
        fsid: Default::default(), // file system id
        fileid,
        atime: Default::default(), // access time
        mtime: Default::default(), // todo modify time could populate with last_updated
        ctime: Default::default(), // create time
    }
}
