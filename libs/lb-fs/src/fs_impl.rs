use crate::cache::FileEntry;
use crate::file_handle::UuidFileHandle;
use crate::utils::{file_id, get_string};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::{Lb, Uuid};
use nfs3_server::nfs3_types::nfs3::{
    Nfs3Option, fattr3, filename3, nfspath3, nfsstat3, sattr3, set_atime, set_mtime,
};
use nfs3_server::vfs::{
    DirEntry, DirEntryPlus, NfsFileSystem, NfsReadFileSystem, ReadDirIterator, ReadDirPlusIterator,
};
use std::collections::HashMap;
use std::iter::Iterator as StdIterator;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, instrument, warn};

type EntriesMap = Arc<Mutex<HashMap<UuidFileHandle, FileEntry>>>;

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
    pub data: EntriesMap,
}

impl Drive {
    /// Loads the child entries of a directory, beginning after the specified cookie.
    ///
    /// The cookie corresponds to the file ID of the last entry returned by a previous `readdir` or
    /// `readdirplus` call. A cookie value of `0` indicates that iteration should begin from the
    /// start of the directory.
    ///
    /// Note: The file ID used as a cookie represents half of the file’s UUID.
    /// While rare, collisions between file IDs can occur, meaning two distinct files may share
    /// the same ID. In such cases, some entries might be skipped. This issue affects only large
    /// datasets when `readdir` or `readdirplus` is invoked multiple times. The possible solution
    /// might be to use an index as the cookie instead of the file ID.
    async fn load_children(
        &self, dirid: &UuidFileHandle, cookie: u64,
    ) -> impl StdIterator<Item = File> + 'static {
        let mut children = self.lb.get_children(dirid.as_uuid()).await.unwrap();

        children.sort_by(|a, b| a.id.cmp(&b.id));

        let mut start_index = 0;
        if cookie > 0 {
            start_index = children
                .iter()
                .position(|child| file_id(child) > cookie)
                .unwrap_or_else(|| {
                    warn!("cookie {cookie} not found");
                    children.len()
                });
        }

        children.into_iter().skip(start_index)
    }
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
        let dir = self.data.lock().await.get(dirid).unwrap().file.clone();

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

        let doc = self.lb.read_document(*id.as_uuid(), false).await.unwrap();

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
        let iter = self.load_children(dirid, cookie).await;
        Ok(Iterator { inner: iter })
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), start_after = cookie))]
    async fn readdirplus(
        &self, dirid: &Self::Handle, cookie: u64,
    ) -> Result<impl ReadDirPlusIterator<UuidFileHandle>, nfsstat3> {
        let iter = self.load_children(dirid, cookie).await;
        let data = self.data.lock().await;

        let iter = iter
            .map(move |file| {
                let id: UuidFileHandle = file.id.into();
                let name = file.name.as_bytes().to_vec().into();
                let fattr = data.get(&id).map(|entry| entry.fattr.clone());
                DirEntryPlus {
                    fileid: id.fileid(),
                    name,
                    cookie: id.fileid(),
                    name_attributes: fattr,
                    name_handle: Some(id),
                }
            })
            .collect::<Vec<_>>()
            .into_iter();
        Ok(IteratorPlus { inner: iter })
    }

    async fn readlink(&self, _id: &Self::Handle) -> Result<nfspath3<'_>, nfsstat3> {
        info!("readlink NOTSUPP");
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}

impl NfsFileSystem for Drive {
    #[instrument(skip(self), fields(id = id.to_string()))]
    async fn setattr(&self, id: &Self::Handle, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        let mut data = self.data.lock().await;
        let now = FileEntry::now();
        let entry = data.get_mut(id).unwrap();

        if let Nfs3Option::Some(new) = setattr.size {
            if entry.fattr.size != new {
                let mut doc = self.lb.read_document(*id.as_uuid(), false).await.unwrap();
                doc.resize(new as usize, 0);
                self.lb.write_document(*id.as_uuid(), &doc).await.unwrap();
                entry.fattr.mtime = now;
                entry.fattr.ctime = now;
            }
        }

        match setattr.atime {
            set_atime::DONT_CHANGE => {}
            set_atime::SET_TO_SERVER_TIME => {
                entry.fattr.atime = now;
            }
            set_atime::SET_TO_CLIENT_TIME(ts) => {
                entry.fattr.atime = ts;
            }
        }

        match setattr.mtime {
            set_mtime::DONT_CHANGE => {}
            set_mtime::SET_TO_SERVER_TIME => {
                entry.fattr.mtime = now;
                entry.fattr.ctime = now;
            }
            set_mtime::SET_TO_CLIENT_TIME(ts) => {
                entry.fattr.mtime = ts;
                entry.fattr.ctime = ts;
            }
        }

        if let Nfs3Option::Some(uid) = setattr.uid {
            entry.fattr.uid = uid;
            entry.fattr.ctime = now;
        }

        if let Nfs3Option::Some(gid) = setattr.gid {
            entry.fattr.gid = gid;
            entry.fattr.ctime = now;
        }

        if let Nfs3Option::Some(mode) = setattr.mode {
            entry.fattr.mode = mode;
            entry.fattr.ctime = now;
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
        let entry = data.get_mut(id).unwrap();

        let mut doc = self.lb.read_document(*id.as_uuid(), false).await.unwrap();
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
        self.lb.write_document(*id.as_uuid(), &doc).await.unwrap();

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
        let file = self
            .lb
            .create_file(&filename, dirid.as_uuid(), FileType::Document)
            .await
            .unwrap();

        let id = file.id.into();
        let entry = FileEntry::from_file(file, 0);
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
        let children = self.lb.get_children(dirid.as_uuid()).await.unwrap();
        for child in children {
            if child.name == filename {
                warn!("exists already");
                return Err(nfsstat3::NFS3ERR_EXIST);
            }
        }

        let file = self
            .lb
            .create_file(&filename, dirid.as_uuid(), FileType::Document)
            .await
            .unwrap();

        let id = file.id.into();
        let entry = FileEntry::from_file(file, 0);
        info!("({id}, size={})", entry.fattr.size);
        self.data.lock().await.insert(id, entry);

        Ok(id)
    }

    #[instrument(skip(self), fields(dirid = dirid.to_string(), dirname = get_string(dirname)))]
    async fn mkdir(
        &self, dirid: &Self::Handle, dirname: &filename3<'_>,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        let filename = get_string(dirname);
        let file = self
            .lb
            .create_file(&filename, dirid.as_uuid(), FileType::Folder)
            .await
            .unwrap();

        let id = file.id.into();
        let entry = FileEntry::from_file(file, 0);
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

        let children = self.lb.get_children(dirid.as_uuid()).await.unwrap();
        let file_name = get_string(filename);

        for child in children {
            if file_name == child.name {
                info!("deleted");
                let _ = self.lb.delete(&child.id).await; // ignore errors
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

        let src_children = self.lb.get_children(from_dirid.as_uuid()).await.unwrap();

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
            let dst_children = self.lb.get_children(to_dirid.as_uuid()).await.unwrap();
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

                let entry = data.get_mut(&id.into()).unwrap();
                entry.fattr.size = doc_len;

                data.remove(&from_id.into());
            }

            // we are doing a move and/or rename
            None => {
                if from_dirid != to_dirid {
                    info!("move {} -> {}\t", from_id, to_dirid);
                    self.lb
                        .move_file(&from_id, to_dirid.as_uuid())
                        .await
                        .unwrap();
                }

                if from_filename != to_filename {
                    info!("rename {} -> {}\t", from_id, to_filename);
                    self.lb.rename_file(&from_id, &to_filename).await.unwrap();
                }

                let entry = data.get_mut(&from_id.into()).unwrap();

                let file = self.lb.get_file_by_id(from_id).await.unwrap();
                entry.file = file;

                info!("ok");
            }
        }

        Ok(())
    }

    async fn symlink<'a>(
        &self, _dirid: &Self::Handle, _linkname: &filename3<'a>, _symlink: &nfspath3<'a>,
        _attr: &sattr3,
    ) -> Result<(Self::Handle, fattr3), nfsstat3> {
        info!("symlink NOTSUPP");
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}

pub struct Iterator<I>
where
    I: StdIterator<Item = File> + Send + Sync + 'static,
{
    inner: I,
}

impl<I> ReadDirIterator for Iterator<I>
where
    I: StdIterator<Item = File> + Send + Sync + 'static,
{
    async fn next(&mut self) -> nfs3_server::vfs::NextResult<DirEntry> {
        match self.inner.next() {
            Some(entry) => nfs3_server::vfs::NextResult::Ok(DirEntry {
                fileid: file_id(&entry),
                name: entry.name.as_bytes().to_vec().into(),
                cookie: 0,
            }),
            None => nfs3_server::vfs::NextResult::Eof,
        }
    }
}

pub struct IteratorPlus<I>
where
    I: StdIterator<Item = DirEntryPlus<UuidFileHandle>> + Send + Sync + 'static,
{
    inner: I,
}

impl<I> ReadDirPlusIterator<UuidFileHandle> for IteratorPlus<I>
where
    I: StdIterator<Item = DirEntryPlus<UuidFileHandle>> + Send + Sync + 'static,
{
    async fn next(&mut self) -> nfs3_server::vfs::NextResult<DirEntryPlus<UuidFileHandle>> {
        match self.inner.next() {
            Some(entry) => nfs3_server::vfs::NextResult::Ok(entry),
            None => nfs3_server::vfs::NextResult::Eof,
        }
    }
}
