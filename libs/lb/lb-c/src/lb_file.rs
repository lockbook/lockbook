use std::{ffi::c_char, ptr};

use lb_rs::{
    model::{
        file::{File, Share, ShareMode},
        file_metadata::FileType,
    },
    Uuid,
};

use crate::ffi_utils::{carray, cstring};

#[repr(C)]
pub struct LbFileList {
    list: *mut LbFile,
    count: usize,
}

#[repr(C)]
pub struct LbFile {
    id: Uuid,
    parent: Uuid,
    name: *mut c_char,
    typ: LbFileType,
    lastmod_by: *mut c_char,
    lastmod: u64,
    shares: LbShareList,
}

/// The zero value represents a document.
#[repr(C)]
#[derive(Default)]
pub struct LbFileType {
    tag: LbFileTypeTag,
    link_target: Uuid,
}

#[repr(C)]
#[derive(Default)]
pub enum LbFileTypeTag {
    #[default]
    Document,
    Folder,
    Link,
}

#[repr(C)]
pub struct LbShareList {
    list: *mut LbShare,
    count: usize,
}

#[repr(C)]
pub struct LbShare {
    by: *mut c_char,
    with: *mut c_char,
    mode: ShareMode,
}

impl From<LbFileType> for FileType {
    fn from(value: LbFileType) -> Self {
        match value.tag {
            LbFileTypeTag::Document => Self::Document,
            LbFileTypeTag::Folder => Self::Folder,
            LbFileTypeTag::Link => Self::Link { target: value.link_target },
        }
    }
}

impl From<FileType> for LbFileType {
    fn from(value: FileType) -> Self {
        let mut ret = Self { tag: LbFileTypeTag::Document, link_target: Uuid::nil() };

        match value {
            FileType::Document => ret.tag = LbFileTypeTag::Document,
            FileType::Folder => ret.tag = LbFileTypeTag::Folder,
            FileType::Link { target } => {
                ret.tag = LbFileTypeTag::Link;
                ret.link_target = target;
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
            id: value.id,
            parent: value.parent,
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
