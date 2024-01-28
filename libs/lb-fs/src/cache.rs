use crate::fs_impl::Drive;
use lb_rs::File;
use nfsserve::nfs::{fattr3, ftype3, nfstime3};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct FileEntry {
    pub file: File,
    pub fattr: fattr3,
}

impl FileEntry {
    pub fn from_file(file: File, size: u64) -> Self {
        let ftype = if file.is_folder() { ftype3::NF3DIR } else { ftype3::NF3REG };

        // todo this deserves some scrutiny and cross platform testing
        let mode = if file.is_folder() { 0o755 } else { 0o644 };

        let fileid = file.id.as_u64_pair().0;
        // intereREADDIR3resstingly a number of key read operations rely on this being correct
        let size = if file.is_folder() { 0 } else { size };

        let atime = Self::ts_from_u64(0);
        let mtime = Self::ts_from_u64(file.last_modified);
        let ctime = Self::ts_from_u64(file.last_modified);

        let fattr = fattr3 {
            ftype,
            mode,
            nlink: Default::default(), // hard links to this file
            uid: 501,                  // todo: evaluate owner field? not resolved by this lib
            gid: 20,                   // group id
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

    pub fn now() -> u64 {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        time.as_millis() as u64
    }
}

impl Drive {
    // todo: probably need a variant of this that is more suitable post sync cache updates
    pub async fn prepare_caches(&self) {
        let sizes = self.ac.get_sizes().await;
        let files = self.ac.list_metadata().await;

        let mut data = self.data.lock().await;
        for file in files {
            let id = file.id;
            let entry = FileEntry::from_file(file, *sizes.get(&id).unwrap() as u64);
            data.insert(entry.fattr.fileid, entry);
        }
    }
}
