use async_trait::async_trait;
use lb_rs::FileType;
use nfsserve::{
    nfs::{fattr3, fileid3, filename3, nfspath3, nfsstat3, nfsstring, sattr3, set_size3},
    vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities},
};

use crate::{
    utils::{fmt, get_string},
    Drive, VERBOSE,
};

#[async_trait]
impl NFSFileSystem for Drive {
    fn root_dir(&self) -> fileid3 {
        if VERBOSE {
            print!("root_dir() ->");
        }

        let root = self.ac.get_root().id;
        let half = root.as_u64_pair().0;

        if VERBOSE {
            println!("\t{root}");
        }
        half
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    // todo replace actual todos with Err(nfsstat3::NFS3ERR_NOTSUPP)
    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        if VERBOSE {
            print!("write({}, {} |{}|) -> ", fmt(id), offset, data.len());
        }
        let offset = offset as usize;

        let mut doc = self.ac.read_document(id).await;
        if offset + data.len() > doc.len() {
            doc.resize(offset + data.len(), 0);
            doc[offset..].copy_from_slice(data);
        } else {
            for (idx, datum) in data.into_iter().enumerate() {
                doc[offset + idx] = *datum;
            }
        }

        self.ac.write_document(id, doc).await;
        let file = self.ac.get_file_by_id(id).await;
        let file = self.ac.file_to_fattr(&file);

        if VERBOSE {
            println!("\t fattr.size = {}", file.size);
        }

        Ok(file)
    }

    // todo sattr would be what chmod +x requires, but we don't deal with that for now
    // that's going to require us to hold on to fattr3s inmem and make modifications that
    // core doesn't know or care about. Could be annoying for people trying to set executables
    // those flags won't be sticky without adding fields to metadata
    async fn create(
        &self, dirid: fileid3, filename: &filename3, _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        let filename = get_string(filename);
        if VERBOSE {
            print!("create({}, {}, attr.size={:?}) -> ", fmt(dirid), filename, _attr.size);
        }
        let file = self
            .ac
            .create_file(dirid, FileType::Document, filename)
            .await;
        let file = self.ac.file_to_fattr(&file);
        if VERBOSE {
            println!("({}, size={})", fmt(file.fileid), file.size);
        }
        Ok((file.fileid, file))
    }

    async fn create_exclusive(
        &self, _dirid: fileid3, _filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        if VERBOSE {
            println!("create_exclusive({}, {}) -> \t NOTSUPP", fmt(_dirid), get_string(_filename),);
        }
        return Err(nfsstat3::NFS3ERR_NOTSUPP);
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        if VERBOSE {
            println!("lookup({}, {}) -> ", fmt(dirid), get_string(filename),);
        }
        println!("LOOKUP, {}, {}", dirid, String::from_utf8(filename.0.clone()).unwrap());
        let file = self.ac.get_file_by_id(dirid).await;

        if file.is_document() {
            return Err(nfsstat3::NFS3ERR_NOTDIR);
        }
        // if looking for dir/. its the current directory
        if filename[..] == [b'.'] {
            return Ok(dirid);
        }

        // if looking for dir/.. its the parent directory
        if filename[..] == [b'.', b'.'] {
            return Ok(file.parent.as_u64_pair().0);
        }

        let children = self.ac.get_children(dirid).await;
        let file_name = String::from_utf8(filename.0.clone()).unwrap();

        for child in children {
            if file_name == child.name {
                return Ok(child.id.as_u64_pair().0);
            }
        }

        Err(nfsstat3::NFS3ERR_NOENT)
    }

    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        println!("GETATTR");
        let file = self.ac.get_file_by_id(id).await;
        let file = self.ac.file_to_fattr(&file);
        // todo: handle error
        Ok(file)
    }

    // todo this may be how the os communicates truncation to us
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

        return self.getattr(id).await;
    }

    async fn read(
        &self, id: fileid3, offset: u64, count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        println!("READ: id: {}, offset: {}, count: {}", id, offset, count);
        let offset = offset as usize;
        let count = count as usize;

        let doc = self.ac.read_document(id).await;

        if offset >= doc.len() {
            println!("EOF == TRUE");
            return Ok((vec![], true));
        }

        if offset + count >= doc.len() {
            println!("returning {} bytes", doc[offset..].len());
            return Ok((doc[offset..].to_vec(), true));
        }

        return Ok((doc[offset..offset + count].to_vec(), false));
    }

    // todo it's unclear to me whether they would be as bold to provide a start_after of 0 if
    // they don't have a particular start_after in mind yet
    // possibly fileid3 is just the wrong type and they mean a usize offset
    async fn readdir(
        &self, dirid: fileid3, start_after: fileid3, max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        println!(
            "readdir(dirid: {}, start_after: {}, max_entries: {})",
            dirid, start_after, max_entries
        );

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

        if start_index == children.len() {
            ret.end = true;
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
                attr: self.ac.file_to_fattr(&child), // todo could probably eliminate this alloc
            });
        }

        Ok(ret)
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    #[allow(unused)]
    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        let name = String::from_utf8(filename.0.clone()).unwrap();
        println!("remove filename({name} UNSUPPORTED");
        let children = self.ac.get_children(dirid).await;
        return Ok(());
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    /// either an overwrite rename or move
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

            if to_dirid == from_dirid {
                if child.name == to_filename {
                    to_id = Some(child.id);
                }
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
                let from_doc = self.ac.read_document(from_id.as_u64_pair().0).await;
                self.ac.write_document(id.as_u64_pair().0, from_doc).await;
            }

            // we are doing a move and/or rename
            None => {
                if from_dirid != to_dirid {
                    self.ac.move_file(from_id, to_dirid).await;
                }

                if from_filename != to_filename {
                    self.ac.rename_file(from_id, to_filename).await;
                }
            }
        }

        return Ok(());
    }

    #[allow(unused)]
    async fn mkdir(
        &self, dirid: fileid3, dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        println!("mkdir(: {}, filename: {}", dirid, String::from_utf8(dirname.0.clone()).unwrap());
        let file = self
            .ac
            .create_file(dirid, FileType::Folder, get_string(dirname))
            .await;
        let file = self.ac.file_to_fattr(&file);
        Ok((file.fileid, file))
    }

    async fn symlink(
        &self, _dirid: fileid3, _linkname: &filename3, _symlink: &nfspath3, _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        println!("symlink UNSUPPORTED");
        return Err(nfsstat3::NFS3ERR_NOTSUPP);
    }
    async fn readlink(&self, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        println!("readklink UNSUPPORTED");
        return Err(nfsstat3::NFS3ERR_NOTSUPP);
    }
}
