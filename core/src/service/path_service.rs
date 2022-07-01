use crate::model::repo::RepoSource;
use crate::pure_functions::files;
use crate::{Config, CoreError, RequestContext};
use lockbook_models::file_metadata::FileType::{Folder, Link};
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles, FileType, Owner};
use lockbook_models::tree::{FileMetaMapExt, FileMetadata};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_link_at_path(
        &mut self, config: &Config, path_and_name: &str, target_id: Uuid,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        // todo(sharing): target should itself not be a link... I think. I am drunk.
        // todo(sharing): at most one link per target (this idea I had while sober)
        self.create_at_path_with_type(
            config,
            path_and_name,
            FileType::Link { linked_file: target_id },
        )
    }

    pub fn create_at_path(
        &mut self, config: &Config, path_and_name: &str,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        self.create_at_path_with_type(
            config,
            path_and_name,
            if path_and_name.ends_with('/') { FileType::Folder } else { FileType::Document },
        )
    }

    pub fn create_at_path_with_type(
        &mut self, config: &Config, path_and_name: &str, file_type: FileType,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        if path_and_name.contains("//") {
            return Err(CoreError::PathContainsEmptyFileName);
        }

        let path_components = split_path(path_and_name);

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
            let file_type = if index != path_components.len() - 2 { Folder } else { file_type };

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

    pub fn get_by_path(&mut self, path: &str) -> Result<DecryptedFileMetadata, CoreError> {
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

            let children = self.get_children(current.id)?; // note: performs link substitution
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

    pub fn get_path_by_id(&mut self, id: Uuid) -> Result<String, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        if files
            .find_ref(id)?
            .is_pending_share(&Owner(self.get_public_key()?))
        {
            return Err(CoreError::FileIsLink);
        }
        Self::path_by_id_helper(&files, id)
    }

    pub fn path_by_id_helper(files: &DecryptedFiles, id: Uuid) -> Result<String, CoreError> {
        let mut current_metadata = files.find_ref(id)?;
        let mut path = String::from("");

        let is_folder = current_metadata.is_folder();

        while !current_metadata.is_root() {
            while let Some(link) = files.maybe_find_link(current_metadata.id) {
                current_metadata = files.find_ref(link.id)?;
            }
            path = format!("{}/{}", current_metadata.decrypted_name, path);
            current_metadata = files.find_ref(current_metadata.parent)?;
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
        filtered_files
            .retain(|_, f| !matches!(f.file_type, Link { linked_file: _ }));

        let mut paths: Vec<String> = vec![];
        'outer: for (_, file) in filtered_files {
            let mut current = file.clone();
            let mut current_path = String::from("");
            while current.id != current.parent {
                while current.owner.0 != self.get_account()?.public_key() {
                    if let Some(link) = files.maybe_find_link(current.id) {
                        current = files.find(link.id)?;
                    } else {
                        continue 'outer; // this file is a pending share
                    }
                }

                if current.file_type == Folder {
                    current_path = format!("{}/{}", current.decrypted_name, current_path);
                } else {
                    current_path = current.decrypted_name;
                }

                current = files.find(current.parent)?;
            }

            current_path = format!("{}/{}", current.decrypted_name, current_path);
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
