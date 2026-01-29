use std::ffi::c_char;
use std::ptr;

use lb_rs::Uuid;
use lb_rs::model::file::{File, Share, ShareMode};
use lb_rs::model::file_metadata::FileType;

use crate::LbUuid;
use crate::ffi_utils::{carray, cstring};

#[repr(C)]
pub struct LbFileList {
    pub list: *mut LbFile,
    pub count: usize,
}

#[repr(C)]
pub struct LbFile {
    pub id: LbUuid,
    pub parent: LbUuid,
    pub name: *mut c_char,
    pub typ: LbFileType,
    pub lastmod_by: *mut c_char,
    pub lastmod: u64,
    pub shares: LbShareList,
}

/// The zero value represents a document.
#[repr(C)]
#[derive(Default)]
pub struct LbFileType {
    pub tag: LbFileTypeTag,
    pub link_target: LbUuid,
}

// todo: remove this once cbindgen.toml todo addressed
#[allow(clippy::enum_variant_names)]
#[repr(C)]
#[derive(Default)]
pub enum LbFileTypeTag {
    #[default]
    LbDocument,
    LbFolder,
    LbLink,
}

#[repr(C)]
pub struct LbShareList {
    pub list: *mut LbShare,
    pub count: usize,
}

#[repr(C)]
pub struct LbShare {
    pub by: *mut c_char,
    pub with: *mut c_char,
    pub mode: ShareMode,
}

impl From<LbFileType> for FileType {
    fn from(value: LbFileType) -> Self {
        match value.tag {
            LbFileTypeTag::LbDocument => Self::Document,
            LbFileTypeTag::LbFolder => Self::Folder,
            LbFileTypeTag::LbLink => Self::Link { target: value.link_target.into() },
        }
    }
}

impl From<FileType> for LbFileType {
    fn from(value: FileType) -> Self {
        let mut ret = Self { tag: LbFileTypeTag::LbDocument, link_target: Uuid::nil().into() };

        match value {
            FileType::Document => ret.tag = LbFileTypeTag::LbDocument,
            FileType::Folder => ret.tag = LbFileTypeTag::LbFolder,
            FileType::Link { target } => {
                ret.tag = LbFileTypeTag::LbLink;
                ret.link_target = target.into();
            }
        }

        ret
    }
}

impl From<Share> for LbShare {
    fn from(value: Share) -> Self {
        Self { by: cstring(value.shared_by), with: cstring(value.shared_with), mode: value.mode }
    }
}

impl From<Vec<Share>> for LbShareList {
    fn from(value: Vec<Share>) -> Self {
        let mut new_vec: Vec<LbShare> = Vec::with_capacity(value.len());
        for val in value {
            new_vec.push(val.into());
        }

        let (list, count) = carray(new_vec);

        Self { count, list }
    }
}

impl From<File> for LbFile {
    fn from(value: File) -> Self {
        Self {
            id: value.id.into(),
            parent: value.parent.into(),
            name: cstring(value.name),
            typ: value.file_type.into(),
            lastmod_by: cstring(value.last_modified_by),
            lastmod: value.last_modified,
            shares: value.shares.into(),
        }
    }
}

impl Default for LbFile {
    fn default() -> Self {
        LbFile {
            id: Default::default(),
            parent: Default::default(),
            name: ptr::null_mut(),
            typ: Default::default(),
            lastmod_by: ptr::null_mut(),
            lastmod: Default::default(),
            shares: Default::default(),
        }
    }
}

impl Default for LbShareList {
    fn default() -> Self {
        Self { count: Default::default(), list: ptr::null_mut() }
    }
}

impl Default for LbFileList {
    fn default() -> Self {
        Self { list: ptr::null_mut(), count: Default::default() }
    }
}

impl From<Vec<File>> for LbFileList {
    fn from(files: Vec<File>) -> Self {
        let mut new_vec = Vec::with_capacity(files.len());

        for file in files {
            new_vec.push(file.into());
        }

        let (list, count) = carray(new_vec);

        Self { list, count }
    }
}
