use crate::model::state::Config;
use crate::repo::{file_metadata_repo, local_changes_repo};
use crate::service::file_encryption_service::GenerateClientFileMetadataError;
use crate::service::file_service::NewFileError;
use crate::service::path_service::NewFileFromPathError::FailedToCreateChild;
use crate::service::{file_encryption_service, file_service};
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType::{Document, Folder};

#[derive(Debug)]
pub enum NewFileFromPathError {
    DbError(file_metadata_repo::DbError),
    NoRoot,
    PathDoesntStartWithRoot,
    PathContainsEmptyFile,
    FailedToCreateChild(NewFileError),
    FailedToRecordChange(local_changes_repo::DbError),
    FileAlreadyExists,
    GetNameOfFileError(file_encryption_service::GetNameOfFileError),
    GenerateClientFileMetadataError(GenerateClientFileMetadataError),
}

pub fn create_at_path(
    config: &Config,
    path_and_name: &str,
) -> Result<FileMetadata, NewFileFromPathError> {
    if path_and_name.contains("//") {
        return Err(NewFileFromPathError::PathContainsEmptyFile);
    }

    debug!("Creating path at: {}", path_and_name);
    let path_components = split_path(path_and_name);

    let is_folder = path_and_name.ends_with('/');

    let mut current = file_metadata_repo::get_root(config)
        .map_err(NewFileFromPathError::DbError)?
        .ok_or(NewFileFromPathError::NoRoot)?;

    if file_encryption_service::get_name(&config, &current)
        .map_err(NewFileFromPathError::GetNameOfFileError)?
        != path_components[0]
    {
        return Err(NewFileFromPathError::PathDoesntStartWithRoot);
    }

    if path_components.len() == 1 {
        return Err(NewFileFromPathError::FileAlreadyExists);
    }

    // We're going to look ahead, and find or create the right child
    'path: for index in 0..path_components.len() - 1 {
        let children = file_metadata_repo::get_children_non_recursively(config, current.id)
            .map_err(NewFileFromPathError::DbError)?;

        let next_name = path_components[index + 1];
        debug!("child we're searching for: {}", next_name);

        for child in children {
            if file_encryption_service::get_name(&config, &child)
                .map_err(NewFileFromPathError::GetNameOfFileError)?
                == next_name
            {
                // If we're at the end and we find this child, that means this path already exists
                if index == path_components.len() - 2 {
                    return Err(NewFileFromPathError::FileAlreadyExists);
                }

                if child.file_type == Folder {
                    current = child;
                    continue 'path; // Child exists, onto the next one
                }
            }
        }

        // Child does not exist, create it
        let file_type = if is_folder || index != path_components.len() - 2 {
            Folder
        } else {
            Document
        };

        current = file_service::create(config, next_name, current.id, file_type)
            .map_err(FailedToCreateChild)?;
    }

    Ok(current)
}

#[derive(Debug)]
pub enum GetByPathError {
    FileNotFound(String),
    NoRoot,
    FileMetadataError(file_metadata_repo::DbError),
    NameDecryptionError(file_encryption_service::GetNameOfFileError),
    GenerateClientFileMetadataError(GenerateClientFileMetadataError),
}

pub fn get_by_path(config: &Config, path: &str) -> Result<FileMetadata, GetByPathError> {
    let root = file_metadata_repo::get_root(&config)
        .map_err(GetByPathError::FileMetadataError)?
        .ok_or(GetByPathError::NoRoot)?;

    let paths = split_path(path);
    let mut current = root;

    for (i, value) in paths.iter().enumerate() {
        if *value
            != file_encryption_service::get_name(&config, &current)
                .map_err(GetByPathError::NameDecryptionError)?
        {
            return Err(GetByPathError::FileNotFound(paths[0..i].join("/")));
        }

        if i + 1 == paths.len() {
            return Ok(current);
        }

        let children = file_metadata_repo::get_children_non_recursively(&config, current.id)
            .map_err(GetByPathError::FileMetadataError)?;
        let mut found_child = false;

        for child in children {
            let child_name = file_encryption_service::get_name(&config, &child)
                .map_err(GetByPathError::NameDecryptionError)?;

            if child_name == paths[i + 1] {
                current = child;
                found_child = true;
            }
        }

        if !found_child {
            return Err(GetByPathError::FileNotFound(paths[0..i + 1].join("/")));
        }
    }

    Ok(current)
}

pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

#[derive(Debug)]
pub enum StringToFilterError {
    UnknownFilter,
}

pub fn filter_from_str(input: &str) -> Result<Option<Filter>, StringToFilterError> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(StringToFilterError::UnknownFilter),
    }
}

#[derive(Debug)]
pub enum GetAllPathsError {
    FileMetadataError(file_metadata_repo::DbError),
    FileMetadataGetError(file_metadata_repo::GetError),
    GetNameError(file_encryption_service::GetNameOfFileError),
}

pub fn get_all_paths(
    config: &Config,
    filter: Option<Filter>,
) -> Result<Vec<String>, GetAllPathsError> {
    let files =
        file_metadata_repo::get_all(&config).map_err(GetAllPathsError::FileMetadataError)?;

    let mut filtered_files = files.clone();

    if let Some(filter) = filter {
        match filter {
            Filter::DocumentsOnly => filtered_files.retain(|f| f.file_type == Document),
            Filter::FoldersOnly => filtered_files.retain(|f| f.file_type == Folder),
            Filter::LeafNodesOnly => {
                filtered_files.retain(|parent| !files.iter().any(|child| child.parent == parent.id))
            }
        }
    }

    let mut paths: Vec<String> = vec![];
    for file in filtered_files {
        let mut current = file.clone();
        let mut current_path = String::from("");
        while current.id != current.parent {
            let current_name = file_encryption_service::get_name(&config, &current)
                .map_err(GetAllPathsError::GetNameError)?;
            if current.file_type == Document {
                current_path = current_name;
            } else {
                current_path = format!("{}/{}", current_name, current_path);
            }
            current = file_metadata_repo::get(&config, current.parent)
                .map_err(GetAllPathsError::FileMetadataGetError)?;
        }

        let root_name = file_encryption_service::get_name(&config, &current)
            .map_err(GetAllPathsError::GetNameError)?;
        current_path = format!("{}/{}", root_name, current_path);
        paths.push(current_path.to_string());
    }

    Ok(paths)
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
