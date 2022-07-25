use crate::model::repo::RepoSource;
use crate::OneKey;
use crate::{Config, CoreError, CoreResult, RequestContext};
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::FileType::{Document, Folder};
use lockbook_shared::tree_like::Stagable;

impl RequestContext<'_, '_> {
    pub fn create_at_path(&mut self, path: &str) -> CoreResult<File> {
        let pub_key = self.get_public_key()?;
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let root = self
            .tx
            .root
            .get(&OneKey {})
            .ok_or(CoreError::RootNonexistent)?;

        let (mut tree, id) = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy()
            .create_at_path(path, *root, account, &pub_key)?;

        let ui_file = tree.finalize(&id, account)?;

        Ok(ui_file)
    }

    // pub fn get_by_path(&mut self, path: &str) -> Result<CoreFile, CoreError> {
    //     let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
    //     let paths = split_path(path);
    //
    //     let mut current = files.find_ref(self.root_id()?)?;
    //
    //     for value in paths {
    //         let children = files.find_children_ref(current.id);
    //         let mut found_child = false;
    //
    //         for child in children.values() {
    //             if child.decrypted_name == value {
    //                 current = child;
    //                 found_child = true;
    //             }
    //         }
    //
    //         if !found_child {
    //             return Err(CoreError::FileNonexistent);
    //         }
    //     }
    //
    //     Ok(current.clone())
    // }
    //
    // pub fn get_path_by_id(&mut self, id: Uuid) -> Result<String, CoreError> {
    //     let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
    //     Self::path_by_id_helper(&files, id)
    // }
    //
    // pub fn path_by_id_helper(files: &DecryptedFiles, id: Uuid) -> Result<String, CoreError> {
    //     let mut current_metadata = files.find_ref(id)?;
    //     let mut path = String::from("");
    //
    //     let is_folder = current_metadata.is_folder();
    //
    //     while !current_metadata.is_root() {
    //         path = format!("{}/{}", current_metadata.decrypted_name, path);
    //         current_metadata = files.find_ref(current_metadata.parent)?;
    //     }
    //
    //     path = format!("/{}", path);
    //
    //     // Remove the last forward slash if not a folder.
    //     if !is_folder {
    //         path.pop();
    //     }
    //     Ok(path)
    // }
    //
    // pub fn list_paths(&mut self, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
    //     let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
    //
    //     let mut filtered_files = files.clone();
    //
    //     if let Some(filter) = &filter {
    //         match filter {
    //             Filter::DocumentsOnly => filtered_files.retain(|_, f| f.is_document()),
    //             Filter::FoldersOnly => filtered_files.retain(|_, f| f.is_folder()),
    //             Filter::LeafNodesOnly => filtered_files.retain(|parent_id, _parent| {
    //                 !files.iter().any(|child| &child.1.parent == parent_id)
    //             }),
    //         }
    //     }
    //
    //     let mut paths: Vec<String> = vec![];
    //     for (_id, file) in filtered_files {
    //         let mut current = file.clone();
    //         let mut current_path = String::from("");
    //         while !current.is_root() {
    //             if current.is_document() {
    //                 current_path = current.decrypted_name;
    //             } else {
    //                 current_path = format!("{}/{}", current.decrypted_name, current_path);
    //             }
    //             current = files.find(current.parent)?;
    //         }
    //
    //         current_path = format!("/{}", current_path);
    //         paths.push(current_path.to_string());
    //     }
    //
    //     Ok(paths)
    // }
}

#[derive(Debug, PartialEq)]
pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

pub fn filter_from_str(input: &str) -> Result<Option<Filter>, CoreError> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(CoreError::Unexpected(String::from("unknown filter"))),
    }
}
