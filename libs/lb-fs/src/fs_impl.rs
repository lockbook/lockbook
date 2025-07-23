use crate::cache::FileEntry;
use crate::utils::{fmt, get_string};
use async_trait::async_trait;
use lb_rs::model::file_metadata::FileType;
use lb_rs::{Lb, Uuid};
use nfsserve::nfs::{
    fattr3, fileid3, filename3, nfspath3, nfsstat3, nfsstring, sattr3, set_atime, set_gid3,
    set_mode3, set_mtime, set_size3, set_uid3,
};
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, instrument, warn};

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
    pub data: Arc<Mutex<HashMap<fileid3, FileEntry>>>,
}

#[async_trait]
impl NFSFileSystem for Drive {
    #[instrument(skip(self))]
    fn root_dir(&self) -> fileid3 {
        let root = self.root;
        let half = root.as_u64_pair().0;

        info!("ret={root}");
        half
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    #[instrument(skip(self), fields(id = fmt(id), buffer = buffer.len()))]
    async fn write(&self, id: fileid3, offset: u64, buffer: &[u8]) -> Result<fattr3, nfsstat3> {
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

        Ok(entry.fattr)
    }

    // todo this should create a file regardless of whether it exists
    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn create(
        &self, dirid: fileid3, filename: &filename3, attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let filename = get_string(filename);
        let parent = self.data.lock().await.get(&dirid).unwrap().file.id;
        let file = self
            .lb
            .create_file(&filename, &parent, FileType::Document)
            .await
            .unwrap();

        let entry = FileEntry::from_file(file, 0);
        let id = entry.fattr.fileid;
        self.data.lock().await.insert(entry.fattr.fileid, entry);

        let file = self.setattr(id, attr).await.unwrap();

        info!("({}, size={})", fmt(file.fileid), file.size);
        Ok((id, file))
    }

    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn create_exclusive(
        &self, dirid: fileid3, filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
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
        let id = entry.fattr.fileid;
        info!("({}, size={})", fmt(id), entry.fattr.size);
        self.data.lock().await.insert(entry.fattr.fileid, entry);

        return Ok(id);
    }

    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let dir = self.data.lock().await.get(&dirid).unwrap().file.clone();

        if dir.is_document() {
            info!("NOTDIR");
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        }

        // if looking for dir/. its the current directory
        if filename[..] == [b'.'] {
            info!(". == {}", fmt(dirid));
            return Ok(dirid);
        }

        // if looking for dir/.. its the parent directory
        if filename[..] == [b'.', b'.'] {
            info!(".. == {}", dir.parent);
            return Ok(dir.parent.as_u64_pair().0);
        }

        let children = self.lb.get_children(&dir.id).await.unwrap();
        let file_name = String::from_utf8(filename.0.clone()).unwrap();

        for child in children {
            if file_name == child.name {
                info!("{}", child.id);
                return Ok(child.id.as_u64_pair().0);
            }
        }

        info!("NOENT");
        Err(nfsstat3::NFS3ERR_NOENT)
    }

    #[instrument(skip(self), fields(id = fmt(id)))]
    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        let file = self.data.lock().await.get(&id).unwrap().fattr;
        info!("fattr = {:?}", file);
        Ok(file)
    }

    #[instrument(skip(self), fields(id = fmt(id)))]
    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let mut data = self.data.lock().await;
        let now = FileEntry::now();
        let entry = data.get_mut(&id).unwrap();

        match setattr.size {
            set_size3::Void => {}
            set_size3::size(new) => {
                if entry.fattr.size != new {
                    let mut doc = self.lb.read_document(entry.file.id, false).await.unwrap();
                    doc.resize(new as usize, 0);
                    self.lb.write_document(entry.file.id, &doc).await.unwrap();
                    entry.fattr.mtime = FileEntry::ts_from_u64(now);
                    entry.fattr.ctime = FileEntry::ts_from_u64(now);
                }
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
                entry.fattr.mtime = ts;
                entry.fattr.ctime = ts;
            }
        }

        match setattr.uid {
            set_uid3::Void => {}
            set_uid3::uid(uid) => {
                entry.fattr.uid = uid;
                entry.fattr.ctime = FileEntry::ts_from_u64(now);
            }
        }

        match setattr.gid {
            set_gid3::Void => {}
            set_gid3::gid(gid) => {
                entry.fattr.gid = gid;
                entry.fattr.ctime = FileEntry::ts_from_u64(now);
            }
        }

        match setattr.mode {
            set_mode3::Void => {}
            set_mode3::mode(mode) => {
                entry.fattr.mode = mode;
                entry.fattr.ctime = FileEntry::ts_from_u64(now);
            }
        }

        info!("fattr = {:?}", entry.fattr);

        return Ok(entry.fattr);
    }

    #[instrument(skip(self), fields(id = fmt(id), offset, count))]
    async fn read(
        &self, id: fileid3, offset: u64, count: u32,
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

    /// they will provide a start_after of 0 for no id
    #[instrument(skip(self), fields(dirid = fmt(dirid), start_after, max_entries))]
    async fn readdir(
        &self, dirid: fileid3, start_after: fileid3, max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        let data = self.data.lock().await;
        let dirid = data.get(&dirid).unwrap().file.id;
        let mut children = self.lb.get_children(&dirid).await.unwrap();

        children.sort_by(|a, b| a.id.cmp(&b.id));

        // this is how the example does it, we'd never return a fileid3 of 0
        let mut start_index = 0;
        if start_after > 0 {
            for (idx, child) in children.iter().enumerate() {
                if child.id.as_u64_pair().0 == start_after {
                    start_index = idx + 1;
                }
            }
        }

        let mut ret = ReadDirResult::default();

        if start_index >= children.len() {
            ret.end = true;
            info!("[] done");
            return Ok(ret);
        }

        let end_index = if start_index + max_entries >= children.len() {
            ret.end = true;
            children.len()
        } else {
            start_index + max_entries
        };

        for child in &children[start_index..end_index] {
            let fileid = child.id.as_u64_pair().0;
            let name = nfsstring(child.name.clone().into_bytes());
            let attr = data.get(&fileid).unwrap().fattr;

            ret.entries.push(DirEntry { fileid, name, attr });
        }

        info!("|{}| done={}", ret.entries.len(), ret.end);

        Ok(ret)
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    #[allow(unused)]
    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        let mut data = self.data.lock().await;
        let dirid = data.get(&dirid).unwrap().file.id;

        let children = self.lb.get_children(&dirid).await.unwrap();
        let file_name = String::from_utf8(filename.0.clone()).unwrap();

        for child in children {
            if file_name == child.name {
                info!("deleted");
                self.lb.delete(&child.id).await;
                data.remove(&child.id.as_u64_pair().0);
                return Ok(());
            }
        }

        info!("NOENT");
        return Err(nfsstat3::NFS3ERR_NOENT);
    }

    /// either an overwrite rename or move
    #[instrument(skip(self), fields(from_dirid = fmt(from_dirid), from_filename = get_string(from_filename), to_dirid = fmt(to_dirid), to_filename = get_string(to_filename)))]
    #[allow(unused)]
    async fn rename(
        &self, from_dirid: fileid3, from_filename: &filename3, to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        let mut data = self.data.lock().await;

        let from_filename = String::from_utf8(from_filename.0.clone()).unwrap();
        let to_filename = String::from_utf8(to_filename.0.clone()).unwrap();

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

                let mut entry = data.get_mut(&id.as_u64_pair().0).unwrap();
                entry.fattr.size = doc_len;

                data.remove(&from_id.as_u64_pair().0);
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

                let mut entry = data.get_mut(&from_id.as_u64_pair().0).unwrap();

                let file = self.lb.get_file_by_id(from_id).await.unwrap();
                entry.file = file;

                info!("ok");
            }
        }

        return Ok(());
    }

    #[instrument(skip(self), fields(dirid = fmt(dirid), dirname = get_string(dirname)))]
    #[allow(unused)]
    async fn mkdir(
        &self, dirid: fileid3, dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let filename = get_string(dirname);
        let parent = self.data.lock().await.get(&dirid).unwrap().file.id;
        let file = self
            .lb
            .create_file(&filename, &parent, FileType::Folder)
            .await
            .unwrap();

        let entry = FileEntry::from_file(file, 0);
        let id = entry.fattr.fileid;
        let fattr = entry.fattr;
        self.data.lock().await.insert(entry.fattr.fileid, entry);

        info!("({}, fattr={:?})", fmt(id), fattr);
        Ok((id, fattr))
    }

    async fn symlink(
        &self, _dirid: fileid3, _linkname: &filename3, _symlink: &nfspath3, _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        info!("symlink NOTSUPP");
        return Err(nfsstat3::NFS3ERR_NOTSUPP);
    }
    async fn readlink(&self, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        info!("readklink NOTSUPP");
        return Err(nfsstat3::NFS3ERR_NOTSUPP);
    }
}
