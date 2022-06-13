use crate::model::repo::RepoSource;
use crate::pure_functions::files;
use crate::{Config, CoreError, Tx};
use lockbook_models::file_metadata::FileType::{Document, Folder};
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles};
use lockbook_models::tree::{FileMetaMapExt, FileMetadata};
use uuid::Uuid;

impl Tx<'_> {
    pub fn create_at_path(
        &mut self, config: &Config, path_and_name: &str,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        if path_and_name.contains("//") {
            return Err(CoreError::PathContainsEmptyFileName);
        }

        let path_components = split_path(path_and_name);

        let is_folder = path_and_name.ends_with('/');

        let mut files = self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut current = files.find(self.root_id()?)?;
        let root_id = current.id;
        let account = self.get_account()?;

        if current.decrypted_name != path_components[0] {
            return Err(CoreError::PathStartsWithNonRoot);
        }

        if path_components.len() == 1 {
            return Err(CoreError::PathTaken);
        }

        // We're going to look ahead, and find or create the right child
        'path: for index in 0..path_components.len() - 1 {
            let children = files.find_children(current.id);

            let next_name = path_components[index + 1];

            for (_, child) in children {
                if child.decrypted_name == next_name {
                    // If we're at the end and we find this child, that means this path already exists
                    if child.id != root_id && index == path_components.len() - 2 {
                        return Err(CoreError::PathTaken);
                    }

                    if child.is_folder() {
                        current = child;
                        continue 'path; // Child exists, onto the next one
                    } else {
                        return Err(CoreError::FileNotFolder);
                    }
                }
            }

            // Child does not exist, create it
            let file_type =
                if is_folder || index != path_components.len() - 2 { Folder } else { Document };

            current = files::apply_create(
                &files,
                file_type,
                current.id,
                next_name,
                &account.public_key(),
            )?;
            files.insert(current.id, current.clone());
            self.insert_metadatum(config, RepoSource::Local, &current)?;
        }
        Ok(current)
    }

    pub fn get_by_path(&self, path: &str) -> Result<DecryptedFileMetadata, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let paths = split_path(path);

        let mut current = files.find(self.root_id()?)?;

        for (i, &value) in paths.iter().enumerate() {
            if value != current.decrypted_name {
                return Err(CoreError::FileNonexistent);
            }

            if i + 1 == paths.len() {
                return Ok(current);
            }

            let children = files.find_children(current.id);
            let mut found_child = false;

            for (_, child) in children {
                if child.decrypted_name == paths[i + 1] {
                    current = child;
                    found_child = true;
                }
            }

            if !found_child {
                return Err(CoreError::FileNonexistent);
            }
        }

        Ok(current)
    }

    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        Self::path_by_id_helper(&files, id)
    }

    pub fn path_by_id_helper(files: &DecryptedFiles, id: Uuid) -> Result<String, CoreError> {
        let mut current_metadata = files.find(id)?;
        let mut path = String::from("");

        let is_folder = current_metadata.is_folder();

        while current_metadata.parent != current_metadata.id {
            path = format!("{}/{}", current_metadata.decrypted_name, path);
            current_metadata = files.find(current_metadata.parent)?;
        }

        {
            path = format!("{}/{}", current_metadata.decrypted_name, path);
        }
        // Remove the last forward slash if not a folder.
        if !is_folder {
            path.pop();
        }
        Ok(path)
    }

    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut filtered_files = files.clone();

        if let Some(filter) = &filter {
            match filter {
                Filter::DocumentsOnly => filtered_files.retain(|_, f| f.is_document()),
                Filter::FoldersOnly => filtered_files.retain(|_, f| f.is_folder()),
                Filter::LeafNodesOnly => filtered_files.retain(|parent_id, _parent| {
                    !files.iter().any(|child| &child.1.parent == parent_id)
                }),
            }
        }

        let mut paths: Vec<String> = vec![];
        for (_id, file) in filtered_files {
            let mut current = file.clone();
            let mut current_path = String::from("");
            while current.id != current.parent {
                if current.is_document() {
                    current_path = current.decrypted_name;
                } else {
                    current_path = format!("{}/{}", current.decrypted_name, current_path);
                }
                current = files.find(current.parent)?;
            }

            current_path = format!("{}/{}", current.decrypted_name, current_path);
            paths.push(current_path.to_string());
        }

        Ok(paths)
    }
}

#[derive(Debug)]
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

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
