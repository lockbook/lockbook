#![expect(unused)] // FIXME: remove this once all unused code is removed

use crate::cache::FileEntry;
use crate::utils::get_string;
use async_trait::async_trait;
use lb_rs::model::file_metadata::FileType;
use lb_rs::{Lb, Uuid};
use nfs3_server::nfs3_types::nfs3::{
    Nfs3Option, fattr3, fileid3, filename3, nfspath3, nfsstat3, sattr3, set_atime, set_gid3,
    set_mode3, set_mtime, set_size3, set_uid3,
};
use nfs3_server::vfs::{
    FileHandle, FileHandleU64, NfsFileSystem, NfsReadFileSystem, ReadDirPlusIterator,
    VFSCapabilities,
};
use std::collections::HashMap;
use std::fs::ReadDir;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, instrument, warn};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UuidFileHandle(pub Uuid);

impl std::fmt::Display for UuidFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FileHandle for UuidFileHandle {
    fn len(&self) -> usize {
        16
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Uuid::from_slice(bytes).ok().map(|id| Self(id))
    }
}

impl From<Uuid> for UuidFileHandle {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[derive(Clone)]
pub struct Drive {
    pub lb: Lb,

    /// must be not-nil before NFSFIlesSystem is mounted
    pub root: Uuid,

    /// probably this doesn't need to have a global lock, but interactions here are generally
    /// speedy, and for now we'll go for robustness over performance. Hopefully this accomplishes
    /// that and not deadlock. TBD.
    ///
    /// this is stored in memory as it's own entity and not stored in core for two reasons:
    /// 1. size computations are expensive in core
    /// 2. nfs needs to update timestamps to specified values
    /// 3. nfs models properties we don't, like file permission bits
    pub data: Arc<Mutex<HashMap<UuidFileHandle, FileEntry>>>,
}

impl NfsReadFileSystem for Drive {
    type Handle = UuidFileHandle;

    #[instrument(skip(self))]
    fn root_dir(&self) -> Self::Handle {
        self.root.into()
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), filename = get_string(filename)))]
    async fn lookup(
        &self, dirid: &Self::Handle, filename: &filename3<'_>,
    ) -> Result<Self::Handle, nfsstat3> {
        let dir = self.data.lock().await.get(&dirid).unwrap().file.clone();

        if dir.is_document() {
            info!("NOTDIR");
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        }

        // if looking for dir/. its the current directory
        if filename.as_ref() == [b'.'] {
            info!(". == {dirid}");
            return Ok(*dirid);
        }

        // if looking for dir/.. its the parent directory
        if filename.as_ref() == [b'.', b'.'] {
            info!(".. == {}", dir.parent);
            return Ok(dir.parent.into());
        }

        let children = self.lb.get_children(&dir.id).await.unwrap();
        let file_name = get_string(filename);

        for child in children {
            if file_name == child.name {
                info!("{}", child.id);
                return Ok(child.id.into());
            }
        }

        info!("NOENT");
        Err(nfsstat3::NFS3ERR_NOENT)
    }

    #[instrument(skip(self), fields(id = id.to_string()))]
    async fn getattr(&self, id: &Self::Handle) -> Result<fattr3, nfsstat3> {
        let file = self.data.lock().await.get(id).unwrap().fattr.clone();
        info!("fattr = {:?}", file);
        Ok(file)
    }

    #[instrument(skip(self), fields(id = id.to_string(), offset, count))]
    async fn read(
        &self, id: &Self::Handle, offset: u64, count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        let offset = offset as usize;
        let count = count as usize;
        let id = self.data.lock().await.get(&id).unwrap().file.id;

        let doc = self.lb.read_document(id, false).await.unwrap();

        if offset >= doc.len() {
            info!("[] EOF");
            return Ok((vec![], true));
        }

        if offset + count >= doc.len() {
            info!("|{}| EOF", doc[offset..].len());
            return Ok((doc[offset..].to_vec(), true));
        }

        info!("|{}|", count);
        return Ok((doc[offset..offset + count].to_vec(), false));
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), start_after = cookie))]
    async fn readdir(
        &self, dirid: &Self::Handle, cookie: u64,
    ) -> Result<impl nfs3_server::vfs::ReadDirIterator, nfsstat3> {
        Err::<Iterator, _>(nfsstat3::NFS3ERR_IO)
    }

    async fn readdirplus(
        &self, dirid: &Self::Handle, cookie: u64,
    ) -> Result<impl nfs3_server::vfs::ReadDirPlusIterator, nfsstat3> {
        Err::<Iterator, _>(nfsstat3::NFS3ERR_IO)
    }

    async fn readlink(&self, id: &Self::Handle) -> Result<nfspath3<'_>, nfsstat3> {
        info!("readlink NOTSUPP");
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }

    // #[instrument(skip(self), fields(id = fmt(id), offset, count))]
    // async fn read(
    //     &self, id: fileid3, offset: u64, count: u32,
    // ) -> Result<(Vec<u8>, bool), nfsstat3> {
    //     let offset = offset as usize;
    //     let count = count as usize;
    //     let id = self.data.lock().await.get(&id).unwrap().file.id;

    //     let doc = self.lb.read_document(id, false).await.unwrap();

    //     if offset >= doc.len() {
    //         info!("[] EOF");
    //         return Ok((vec![], true));
    //     }

    //     if offset + count >= doc.len() {
    //         info!("|{}| EOF", doc[offset..].len());
    //         return Ok((doc[offset..].to_vec(), true));
    //     }

    //     info!("|{}|", count);
    //     return Ok((doc[offset..offset + count].to_vec(), false));
    // }

    // /// they will provide a start_after of 0 for no id
    // #[instrument(skip(self), fields(dirid = dirid.to_string(), start_after, max_entries))]
    // async fn readdir(
    //     &self, dirid: fileid3, start_after: fileid3, max_entries: usize,
    // ) -> Result<ReadDirResult, nfsstat3> {
    //     let data = self.data.lock().await;
    //     let dirid = data.get(&dirid).unwrap().file.id;
    //     let mut children = self.lb.get_children(&dirid).await.unwrap();

    //     children.sort_by(|a, b| a.id.cmp(&b.id));

    //     // this is how the example does it, we'd never return a fileid3 of 0
    //     let mut start_index = 0;
    //     if start_after > 0 {
    //         for (idx, child) in children.iter().enumerate() {
    //             if child.id.as_u64_pair().0 == start_after {
    //                 start_index = idx + 1;
    //             }
    //         }
    //     }

    //     let mut ret = ReadDirResult::default();

    //     if start_index >= children.len() {
    //         ret.end = true;
    //         info!("[] done");
    //         return Ok(ret);
    //     }

    //     let end_index = if start_index + max_entries >= children.len() {
    //         ret.end = true;
    //         children.len()
    //     } else {
    //         start_index + max_entries
    //     };

    //     for child in &children[start_index..end_index] {
    //         let fileid = child.id.as_u64_pair().0;
    //         let name = nfsstring(child.name.clone().into_bytes());
    //         let attr = data.get(&fileid).unwrap().fattr;

    //         ret.entries.push(DirEntry { fileid, name, attr });
    //     }

    //     info!("|{}| done={}", ret.entries.len(), ret.end);

    //     Ok(ret)
    // }
}

impl NfsFileSystem for Drive {
    #[instrument(skip(self), fields(id = id.to_string()))]
    async fn setattr(&self, id: &Self::Handle, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let mut data = self.data.lock().await;
        let now = FileEntry::now();
        let entry = data.get_mut(&id).unwrap();

        if let Nfs3Option::Some(new) = setattr.size {
            if entry.fattr.size != new {
                let mut doc = self.lb.read_document(entry.file.id, false).await.unwrap();
                doc.resize(new as usize, 0);
                self.lb.write_document(entry.file.id, &doc).await.unwrap();
                entry.fattr.mtime = FileEntry::ts_from_u64(now);
                entry.fattr.ctime = FileEntry::ts_from_u64(now);
            }
        }

        match setattr.atime {
            set_atime::DONT_CHANGE => {}
            set_atime::SET_TO_SERVER_TIME => {
                entry.fattr.atime = FileEntry::ts_from_u64(now);
            }
            set_atime::SET_TO_CLIENT_TIME(ts) => {
                entry.fattr.atime = ts;
            }
        }

        match setattr.mtime {
            set_mtime::DONT_CHANGE => {}
            set_mtime::SET_TO_SERVER_TIME => {
                entry.fattr.mtime = FileEntry::ts_from_u64(now);
                entry.fattr.ctime = FileEntry::ts_from_u64(now);
            }
            set_mtime::SET_TO_CLIENT_TIME(ts) => {
                entry.fattr.mtime = ts.clone(); // FIXME: this should be copiable
                entry.fattr.ctime = ts;
            }
        }

        if let Nfs3Option::Some(uid) = setattr.uid {
            entry.fattr.uid = uid;
            entry.fattr.ctime = FileEntry::ts_from_u64(now);
        }

        if let Nfs3Option::Some(gid) = setattr.gid {
            entry.fattr.gid = gid;
            entry.fattr.ctime = FileEntry::ts_from_u64(now);
        }

        if let Nfs3Option::Some(mode) = setattr.mode {
            entry.fattr.mode = mode;
            entry.fattr.ctime = FileEntry::ts_from_u64(now);
        }

        info!("fattr = {:?}", entry.fattr);
        Ok(entry.fattr.clone())
    }

    #[instrument(skip(self), fields(id = id.to_string(), buffer = buffer.len()))]
    async fn write(
        &self, id: &Self::Handle, offset: u64, buffer: &[u8],
    ) -> Result<fattr3, nfsstat3> {
        let offset = offset as usize;

        let mut data = self.data.lock().await;
        let entry = data.get_mut(&id).unwrap();
        let id = entry.file.id;

        let mut doc = self.lb.read_document(id, false).await.unwrap();
        let mut expanded = false;
        if offset + buffer.len() > doc.len() {
            doc.resize(offset + buffer.len(), 0);
            doc[offset..].copy_from_slice(buffer);
            expanded = true;
        } else {
            for (idx, datum) in buffer.iter().enumerate() {
                doc[offset + idx] = *datum;
            }
        }
        let doc_size = doc.len();
        self.lb.write_document(id, &doc).await.unwrap();

        entry.fattr.size = doc_size as u64;

        info!("expanded={expanded}, fattr.size = {}", doc_size);

        Ok(entry.fattr.clone())
    }

    // todo this should create a file regardless of whether it exists
    #[instrument(skip(self), fields(dirid = dirid.to_string(), filename = get_string(filename)))]
    async fn create(
        &self, dirid: &Self::Handle, filename: &filename3<'_>, attr: sattr3,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let filename = get_string(filename);
        let parent = self.data.lock().await.get(&dirid).unwrap().file.id;
        let file = self
            .lb
            .create_file(&filename, &parent, FileType::Document)
            .await
            .unwrap();

        let entry = FileEntry::from_file(file, 0);
        let id = entry.file.id.into();
        self.data.lock().await.insert(id, entry);

        let file = self.setattr(&id, attr).await.unwrap();

        info!("({id}, size={})", file.size);
        Ok((id, file))
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), filename = get_string(filename)))]
    async fn create_exclusive(
        &self, dirid: &Self::Handle, filename: &filename3<'_>,
        createverf: nfs3_server::nfs3_types::nfs3::createverf3,
    ) -> Result<Self::Handle, nfsstat3> {
        let filename = get_string(filename);
        let dirid = self.data.lock().await.get(&dirid).unwrap().file.id;
        let children = self.lb.get_children(&dirid).await.unwrap();
        for child in children {
            if child.name == filename {
                warn!("exists already");
                return Err(nfsstat3::NFS3ERR_EXIST);
            }
        }

        let file = self
            .lb
            .create_file(&filename, &dirid, FileType::Document)
            .await
            .unwrap();

        let entry = FileEntry::from_file(file, 0);
        let id = entry.file.id.into();
        info!("({id}, size={})", entry.fattr.size);
        self.data.lock().await.insert(id, entry);

        Ok(id)
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), dirname = get_string(dirname)))]
    async fn mkdir(
        &self, dirid: &Self::Handle, dirname: &filename3<'_>,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let filename = get_string(dirname);
        let parent = self.data.lock().await.get(&dirid).unwrap().file.id;
        let file = self
            .lb
            .create_file(&filename, &parent, FileType::Folder)
            .await
            .unwrap();

        let entry = FileEntry::from_file(file, 0);
        let id = entry.file.id.into();
        let fattr = entry.fattr.clone();
        self.data.lock().await.insert(id, entry);

        info!("({id}, fattr={fattr:?})");
        Ok((id, fattr))
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    #[instrument(skip(self), fields(dirid = dirid.to_string(), filename = get_string(filename)))]
    async fn remove(&self, dirid: &Self::Handle, filename: &filename3<'_>) -> Result<(), nfsstat3> {
        let mut data = self.data.lock().await;
        let dirid = data.get(&dirid).unwrap().file.id;

        let children = self.lb.get_children(&dirid).await.unwrap();
        let file_name = get_string(filename);

        for child in children {
            if file_name == child.name {
                info!("deleted");
                self.lb.delete(&child.id).await;
                data.remove(&child.id.into());
                return Ok(());
            }
        }

        info!("NOENT");
        Err(nfsstat3::NFS3ERR_NOENT)
    }

    /// either an overwrite rename or move
    #[instrument(skip(self), fields(from_dirid = from_dirid.to_string(), from_filename = get_string(from_filename), to_dirid = to_dirid.to_string(), to_filename = get_string(to_filename)))]
    async fn rename<'a>(
        &self, from_dirid: &Self::Handle, from_filename: &filename3<'a>, to_dirid: &Self::Handle,
        to_filename: &filename3<'a>,
    ) -> Result<(), nfsstat3> {
        let mut data = self.data.lock().await;

        let from_filename = get_string(from_filename);
        let to_filename = get_string(to_filename);

        let from_dirid = data.get(&from_dirid).unwrap().file.id;
        let to_dirid = data.get(&to_dirid).unwrap().file.id;

        let src_children = self.lb.get_children(&from_dirid).await.unwrap();

        let mut from_id = None;
        let mut to_id = None;
        for child in src_children {
            if child.name == from_filename {
                from_id = Some(child.id);
            }

            if to_dirid == from_dirid && child.name == to_filename {
                to_id = Some(child.id);
            }
        }

        if to_dirid != from_dirid {
            let dst_children = self.lb.get_children(&to_dirid).await.unwrap();
            for child in dst_children {
                if child.name == to_filename {
                    to_id = Some(child.id);
                }
            }
        }

        let from_id = from_id.unwrap();

        match to_id {
            // we are overwriting a file
            Some(id) => {
                info!("overwrite {from_id} -> {id}");
                let from_doc = self.lb.read_document(from_id, false).await.unwrap();
                info!("|{}|", from_doc.len());
                let doc_len = from_doc.len() as u64;
                self.lb.write_document(id, &from_doc).await.unwrap();
                self.lb.delete(&from_id).await.unwrap();

                let mut entry = data.get_mut(&id.into()).unwrap();
                entry.fattr.size = doc_len;

                data.remove(&from_id.into());
            }

            // we are doing a move and/or rename
            None => {
                if from_dirid != to_dirid {
                    info!("move {} -> {}\t", from_id, to_dirid);
                    self.lb.move_file(&from_id, &to_dirid).await.unwrap();
                }

                if from_filename != to_filename {
                    info!("rename {} -> {}\t", from_id, to_filename);
                    self.lb.rename_file(&from_id, &to_filename).await.unwrap();
                }

                let mut entry = data.get_mut(&from_id.into()).unwrap();

                let file = self.lb.get_file_by_id(from_id).await.unwrap();
                entry.file = file;

                info!("ok");
            }
        }

        Ok(())
    }

    async fn symlink<'a>(
        &self, dirid: &Self::Handle, linkname: &filename3<'a>, symlink: &nfspath3<'a>,
        attr: &sattr3,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        info!("symlink NOTSUPP");
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}

pub struct Iterator {}

impl ReadDirPlusIterator for Iterator {
    async fn next(
        &mut self,
    ) -> nfs3_server::vfs::NextResult<nfs3_server::vfs::entryplus3<'static>> {
        todo!()
    }
}
