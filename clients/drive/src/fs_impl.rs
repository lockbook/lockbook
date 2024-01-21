use async_trait::async_trait;
use nfsserve::{
    nfs::{fattr3, fileid3, filename3, ftype3, nfspath3, nfsstat3, sattr3},
    tcp::{NFSTcp, NFSTcpListener},
    vfs::{NFSFileSystem, ReadDirResult, VFSCapabilities},
};

use crate::{core::file_to_attr, Drive};

#[async_trait]
impl NFSFileSystem for Drive {
    fn root_dir(&self) -> fileid3 {
        let root = self.ac.get_root().id;
        root.as_u64_pair().0
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    // todo replace actual todos with Err(nfsstat3::NFS3ERR_NOTSUPP)
    async fn write(&self, id: fileid3, offset: u64, data: &[u8]) -> Result<fattr3, nfsstat3> {
        todo!()
    }

    async fn create(
        &self, dirid: fileid3, filename: &filename3, _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        todo!()
        // let name = String::from_utf8(filename.0.clone()).unwrap();
        // let files = self.core.list_metadatas().unwrap();
        // let mut uuid = Uuid::nil();
        // for file in files {
        //     let id = file.id;
        //     if u_to_f3(id) == dirid {
        //         uuid = id;
        //     }
        // }

        // if uuid == Uuid::nil() {
        //     panic!("UUID was not found");
        // }

        // let newfile = self
        //     .core
        //     .create_file(&name, uuid, FileType::Document)
        //     .unwrap();
        // let mut attr = fattr3::default();
        // let retid = u_to_f3(newfile.id);
        // attr.fileid = retid;
        // attr.ftype = ftype3::NF3REG;
        // //attr.mtime = newfile.last_modified;
        // Ok((retid, attr))
    }

    async fn create_exclusive(
        &self, _dirid: fileid3, _filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        todo!()
    }

    async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
        println!("LOOKUP, {}, {}", dirid, String::from_utf8(filename.0.clone()).unwrap());
        todo!()
    }
    async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
        let file = self.ac.get_file_by_id(id).await;
        let file = file_to_attr(file);
        // todo: handle error
        Ok(file)
    }
    async fn setattr(&self, id: fileid3, setattr: sattr3) -> Result<fattr3, nfsstat3> {
        todo!()
    }

    async fn read(
        &self, id: fileid3, offset: u64, count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        todo!()
    }

    async fn readdir(
        &self, dirid: fileid3, start_after: fileid3, max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        todo!()
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    #[allow(unused)]
    async fn remove(&self, dirid: fileid3, filename: &filename3) -> Result<(), nfsstat3> {
        todo!()
    }

    /// Removes a file.
    /// If not supported dur to readonly file system
    /// this should return Err(nfsstat3::NFS3ERR_ROFS)
    #[allow(unused)]
    async fn rename(
        &self, from_dirid: fileid3, from_filename: &filename3, to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        todo!()
    }

    #[allow(unused)]
    async fn mkdir(
        &self, _dirid: fileid3, _dirname: &filename3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        todo!()
    }

    async fn symlink(
        &self, _dirid: fileid3, _linkname: &filename3, _symlink: &nfspath3, _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        todo!()
    }
    async fn readlink(&self, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        todo!()
    }
}
