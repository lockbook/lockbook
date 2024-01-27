use async_trait::async_trait;
use lb_rs::FileType;
use nfsserve::{
    nfs::{
        fattr3, fileid3, filename3, nfspath3, nfsstat3, nfsstring, sattr3, set_atime, set_mtime,
        set_size3,
    },
    vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities},
};
use std::time::Duration;
use tracing::{info, instrument, warn};

use crate::{
    core::AsyncCore,
    utils::{fmt, get_string},
    Drive,
};

#[async_trait]
impl NFSFileSystem for Drive {
    #[instrument(skip(self))]
    fn root_dir(&self) -> fileid3 {
        let root = self.ac.get_root().id;
        let half = root.as_u64_pair().0;

        info!("ret={root}");
        half
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    #[instrument(skip(self), fields(id = fmt(id), data = data.len()))]
    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        let offset = offset as usize;

        let lock = self.write_lock.lock().await;
        let mut doc = self.ac.read_document(id).await;
        let mut expanded = false;
        if offset + data.len() > doc.len() {
            doc.resize(offset + data.len(), 0);
            doc[offset..].copy_from_slice(data);
            expanded = true;
        } else {
            for (idx, datum) in data.iter().enumerate() {
                doc[offset + idx] = *datum;
            }
        }

        self.ac.write_document(id, doc).await;
        let file = self.ac.get_file_by_id(id).await;
        let file = self.ac.file_to_fattr(&file);

        info!("expanded={expanded}, fattr.size = {}", file.size);

        drop(lock);
        Ok(file)
    }

    // todo sattr would be what chmod +x requires, but we don't deal with that for now
    // that's going to require us to hold on to fattr3s inmem and make modifications that
    // core doesn't know or care about. Could be annoying for people trying to set executables
    // those flags won't be sticky without adding fields to metadata
    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn create(
        &self, dirid: fileid3, filename: &filename3, attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let filename = get_string(filename);
        let file = self
            .ac
            .create_file(dirid, FileType::Document, filename)
            .await;
        let id = file.id.as_u64_pair().0;
        let file = self.setattr(id, attr).await.unwrap();
        info!("({}, size={})", fmt(file.fileid), file.size);
        Ok((id, file))
    }

    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn create_exclusive(
        &self, dirid: fileid3, filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        let filename = get_string(filename);
        let children = self.ac.get_children(dirid).await;
        for child in children {
            if child.name == filename {
                warn!("exists already");
                return Err(nfsstat3::NFS3ERR_EXIST);
            }
        }

        let file = self
            .ac
            .create_file(dirid, FileType::Document, filename)
            .await;
        let file = self.ac.file_to_fattr(&file);
        info!("({}, size={})", fmt(file.fileid), file.size);
        return Ok(file.fileid);
    }

    #[instrument(skip(self), fields(dirid = fmt(dirid), filename = get_string(filename)))]
    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        let file = self.ac.get_file_by_id(dirid).await;

        if file.is_document() {
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
            info!(".. == {}", file.parent);
            return Ok(file.parent.as_u64_pair().0);
        }

        let children = self.ac.get_children(dirid).await;
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
        let file = self.ac.get_file_by_id(id).await;
        let file = self.ac.file_to_fattr(&file);
        info!("fattr.size = {}", file.size);
        Ok(file)
    }

    // todo this may be how the os communicates truncation to us
    #[instrument(skip(self), fields(id = fmt(id)))]
    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        match setattr.size {
            set_size3::Void => {}
            set_size3::size(new) => {
                let new = new as usize;
                let mut doc = self.ac.read_document(id).await;
                if doc.len() != new {
                    doc.resize(new, 0);
                }
                self.ac.write_document(id, doc).await;
            }
        }

        match setattr.atime {
            set_atime::DONT_CHANGE => {}
            set_atime::SET_TO_SERVER_TIME => {
                let time = AsyncCore::now();
                self.ac.set_atime(id, time);
            }
            set_atime::SET_TO_CLIENT_TIME(ts) => {
                let d = Duration::from_secs(ts.seconds as u64)
                    + Duration::from_nanos(ts.nseconds as u64);
                let time = d.as_millis() as u64;
                self.ac.set_atime(id, time);
            }
        }

        match setattr.mtime {
            set_mtime::DONT_CHANGE => {}
            set_mtime::SET_TO_SERVER_TIME => {
                let time = AsyncCore::now();
                self.ac.content_changed(id, time);
            }
            set_mtime::SET_TO_CLIENT_TIME(ts) => {
                let d = Duration::from_secs(ts.seconds as u64)
                    + Duration::from_nanos(ts.nseconds as u64);
                let time = d.as_millis() as u64;
                self.ac.content_changed(id, time);
            }
        }

        let f = self.ac.get_file_by_id(id).await;
        let f = self.ac.file_to_fattr(&f);

        info!("fattr.size = {}", f.size);

        return Ok(f);
    }

    #[instrument(skip(self), fields(id = fmt(id), offset, count))]
    async fn read(
        &self, id: fileid3, offset: u64, count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        let offset = offset as usize;
        let count = count as usize;

        let doc = self.ac.read_document(id).await;

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

    // todo it's unclear to me whether they would be as bold to provide a start_after of 0 if
    // they don't have a particular start_after in mind yet
    // possibly fileid3 is just the wrong type and they mean a usize offset
    #[instrument(skip(self), fields(dirid = fmt(dirid), start_after, max_entries))]
    async fn readdir(
        &self, dirid: fileid3, start_after: fileid3, max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        let mut children = self.ac.get_children(dirid).await;

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
            ret.entries.push(DirEntry {
                fileid: child.id.as_u64_pair().0,
                name: nfsstring(child.name.clone().into_bytes()),
                attr: self.ac.file_to_fattr(child),
            });
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
        let children = self.ac.get_children(dirid).await;
        let file_name = String::from_utf8(filename.0.clone()).unwrap();

        for child in children {
            if file_name == child.name {
                info!("deleted");
                self.ac.remove(child.id).await;
                return Ok(());
            }
        }

        info!("NOENT");
        return Err(nfsstat3::NFS3ERR_NOENT);
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    /// either an overwrite rename or move
    #[instrument(skip(self), fields(from_dirid = fmt(from_dirid), from_filename = get_string(from_filename), to_dirid = fmt(to_dirid), to_filename = get_string(to_filename)))]
    #[allow(unused)]
    async fn rename(
        &self, from_dirid: fileid3, from_filename: &filename3, to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        let from_filename = String::from_utf8(from_filename.0.clone()).unwrap();
        let to_filename = String::from_utf8(to_filename.0.clone()).unwrap();
        let src_children = self.ac.get_children(from_dirid).await;
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
            let dst_children = self.ac.get_children(to_dirid).await;
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
                let from_doc = self.ac.read_document(from_id.as_u64_pair().0).await;
                info!("|{}|", from_doc.len());
                self.ac.write_document(id.as_u64_pair().0, from_doc).await;
            }

            // we are doing a move and/or rename
            None => {
                if from_dirid != to_dirid {
                    info!("move {} -> {}\t", from_id, to_dirid);
                    self.ac.move_file(from_id, to_dirid).await;
                }

                if from_filename != to_filename {
                    info!("rename {} -> {}\t", from_id, to_filename);
                    self.ac.rename_file(from_id, to_filename).await;
                }
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
        let file = self
            .ac
            .create_file(dirid, FileType::Folder, get_string(dirname))
            .await;
        let file = self.ac.file_to_fattr(&file);

        info!("{}", fmt(file.fileid));
        Ok((file.fileid, file))
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
