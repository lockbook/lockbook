use crate::model::repo::RepoSource;
use crate::pure_functions::files;
use crate::{Config, CoreError, RequestContext};
use lockbook_shared::file_metadata::FileType::{Document, Folder};
use lockbook_shared::file_metadata::{CoreFile, DecryptedFiles};
use lockbook_shared::tree::{FileLike, FileMetaMapExt};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_at_path(
        &mut self, config: &Config, path_and_name: &str,
    ) -> Result<CoreFile, CoreError> {
        if path_and_name.contains("//") {
            return Err(CoreError::PathContainsEmptyFileName);
        }

        let path_components = split_path(path_and_name);

        let is_folder = path_and_name.ends_with('/');

        let mut files = self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut current = files.find_ref(self.root_id()?)?;
        let mut new_files = vec![];

        if path_components.is_empty() {
            return Err(CoreError::PathContainsEmptyFileName);
        }

        'path: for index in 0..path_components.len() {
            let children = files.find_children_ref(current.id);
            let name = path_components[index];

            for child in children.values() {
                if child.decrypted_name == path_components[index] {
                    if index == path_components.len() - 1 {
                        return Err(CoreError::PathTaken);
                    }

                    if child.is_folder() {
                        current = child;
                        continue 'path;
                    } else {
                        return Err(CoreError::FileNotFolder);
                    }
                }
            }

            // Child does not exist, create it
            let file_type =
                if is_folder || index != path_components.len() - 1 { Folder } else { Document };
            let new_file =
                files::apply_create(&files, file_type, current.id, name, &self.get_public_key()?)?;
            new_files.push(new_file);
            current = new_files.last().unwrap();
            files.insert(current.id, current.clone());
            self.insert_metadatum(config, RepoSource::Local, current)?;
        }

        Ok(current.clone())
    }

    pub fn get_by_path(&mut self, path: &str) -> Result<CoreFile, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let paths = split_path(path);

        let mut current = files.find_ref(self.root_id()?)?;

        for value in paths {
            let children = files.find_children_ref(current.id);
            let mut found_child = false;

            for child in children.values() {
                if child.decrypted_name == value {
                    current = child;
                    found_child = true;
                }
            }

            if !found_child {
                return Err(CoreError::FileNonexistent);
            }
        }

        Ok(current.clone())
    }

    pub fn get_path_by_id(&mut self, id: Uuid) -> Result<String, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        Self::path_by_id_helper(&files, id)
    }

    pub fn path_by_id_helper(files: &DecryptedFiles, id: Uuid) -> Result<String, CoreError> {
        let mut current_metadata = files.find_ref(id)?;
        let mut path = String::from("");

        let is_folder = current_metadata.is_folder();

        while !current_metadata.is_root() {
            path = format!("{}/{}", current_metadata.decrypted_name, path);
            current_metadata = files.find_ref(current_metadata.parent)?;
        }

        path = format!("/{}", path);

        // Remove the last forward slash if not a folder.
        if !is_folder {
            path.pop();
        }
        Ok(path)
    }

    pub fn list_paths(&mut self, filter: Option<Filter>) -> Result<Vec<String>, CoreError> {
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
            while !current.is_root() {
                if current.is_document() {
                    current_path = current.decrypted_name;
                } else {
                    current_path = format!("{}/{}", current.decrypted_name, current_path);
                }
                current = files.find(current.parent)?;
            }

            current_path = format!("/{}", current_path);
            paths.push(current_path.to_string());
        }

        Ok(paths)
    }
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

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
