use crate::file_like::FileLike;
use crate::{SharedError, SharedResult};

pub fn file_name(name: &str) -> SharedResult<()> {
    if name.is_empty() {
        return Err(SharedError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(SharedError::FileNameContainsSlash);
    }
    Ok(())
}

pub fn not_root<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_root() {
        Err(SharedError::RootModificationInvalid)
    } else {
        Ok(())
    }
}

pub fn is_folder<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_folder() {
        Ok(())
    } else {
        Err(SharedError::FileNotFolder)
    }
}

pub fn is_document<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_document() {
        Ok(())
    } else {
        Err(SharedError::FileNotFolder)
    }
}

pub fn path(path: &str) -> SharedResult<()> {
    if path.contains("//") || path.is_empty() {
        return Err(SharedError::PathContainsEmptyFileName);
    }

    Ok(())
}
