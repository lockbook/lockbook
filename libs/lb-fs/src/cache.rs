use crate::{fs_impl::Drive, utils::file_id};
use lb_rs::model::file::File;
use nfs3_server::nfs3_types::nfs3::{fattr3, ftype3, nfstime3};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};
use tracing::info;

pub struct FileEntry {
    pub file: File,
    pub fattr: fattr3,
}

impl FileEntry {
    pub fn from_file(file: File, size: u64) -> Self {
        let ftype = if file.is_folder() { ftype3::NF3DIR } else { ftype3::NF3REG };

        // todo this deserves some scrutiny and cross platform testing
        let mode = if file.is_folder() { 0o755 } else { 0o644 };

        let fileid = file_id(&file);
        // interestingly a number of key read operations rely on this being correct
        let size = if file.is_folder() { 0 } else { size };

        let atime = Self::ts_from_u64(0);
        let mtime = Self::ts_from_u64(file.last_modified);
        let ctime = Self::ts_from_u64(file.last_modified);

        let fattr = fattr3 {
            type_: ftype,
            mode,
            nlink: 1, // hard links to this file
            uid: 501, // todo: evaluate owner field? not resolved by this lib
            gid: 20,  // group id
            size,
            used: size,               // ?
            rdev: Default::default(), // ?
            fsid: Default::default(), // file system id
            fileid,
            atime,
            mtime,
            ctime,
        };

        Self { file, fattr }
    }

    pub fn ts_from_u64(version: u64) -> nfstime3 {
        let time = Duration::from_millis(version);
        nfstime3 { seconds: time.as_secs() as u32, nseconds: time.subsec_nanos() }
    }

    pub fn now() -> nfstime3 {
        SystemTime::now()
            .try_into()
            .expect("failed to get current time")
    }
}

impl Drive {
    // todo: probably need a variant of this that is more suitable post sync cache updates
    pub async fn fill_cache(&self) {
        info!("preparing cache, are you release build?");
        // the mount is your tree; shares you haven't accepted have no place in it
        let pending: HashSet<_> = self
            .lb
            .get_pending_share_files()
            .await
            .unwrap()
            .into_iter()
            .map(|f| f.id)
            .collect();
        let files = self.lb.list_metadatas().await.unwrap();
        let files = files.into_iter().filter(|f| !pending.contains(&f.id));

        let mut data = self.data.lock().await;
        data.clear();
        for file in files {
            let id = file.id;
            let size = self
                .lb
                .read_document(id, false)
                .await
                .unwrap_or_default()
                .len() as u64;
            let entry = FileEntry::from_file(file, size);
            data.insert(entry.file.id.into(), entry);
        }
        info!("cache ready");
    }
}
