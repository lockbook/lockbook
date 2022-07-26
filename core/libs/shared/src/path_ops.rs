use crate::account::Account;
use crate::file_like::FileLike;
use crate::file_metadata::FileType;
use crate::lazy::LazyStaged1;
use crate::signed_file::SignedFile;
use crate::tree_like::{Stagable, TreeLike};
use crate::{symkey, validate, SharedError, SharedResult};
use libsecp256k1::PublicKey;
use std::collections::HashMap;
use uuid::Uuid;

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: Stagable<F = SignedFile>,
    Local: Stagable<F = Base::F>,
{
    pub fn create_at_path(
        mut self, path: &str, root: Uuid, account: &Account, pub_key: &PublicKey,
    ) -> SharedResult<(LazyStaged1<Base, Local>, Uuid)> {
        validate::path(path)?;
        let is_folder = path.ends_with('/');

        let path_components = split_path(path);
        let mut current = root;
        'path: for index in 0..path_components.len() {
            'child: for child in self.children(&current)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name(&child, account)? == path_components[index] {
                    if index == path_components.len() - 1 {
                        return Err(SharedError::PathTaken);
                    }

                    if self.find(&child)?.is_folder() {
                        current = child;
                        continue 'path;
                    } else {
                        return Err(SharedError::FileNotFolder);
                    }
                }
            }

            // Child does not exist, create it
            let file_type = if is_folder || index != path_components.len() - 1 {
                FileType::Folder
            } else {
                FileType::Document
            };

            (self, current) =
                self.create(&current, path_components[index], file_type, account, pub_key)?;
        }

        Ok((self, current))
    }

    pub fn path_to_id(&mut self, path: &str, root: Uuid, account: &Account) -> SharedResult<Uuid> {
        let paths = split_path(path);

        let mut current = root;
        'path: for path in paths {
            'child: for child in self.children(&current)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name(&child, account)? == path {
                    current = child;
                    continue 'path;
                }
            }

            return Err(SharedError::FileNonexistent);
        }

        Ok(current)
    }

    pub fn id_to_path(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        let mut path = format!("/{}", self.name(id, account)?);
        let mut current = *self.find(id)?.parent();
        loop {
            let current_meta = self.find(&current)?;
            if current_meta.is_root() {
                return Ok(path);
            }
            let next = *current_meta.parent();
            let current_name = self.name(&current, account)?;
            path = format!("/{}/{}", current_name, path);
            current = next;
        }
    }

    pub fn list_paths(
        &mut self, filter: Option<Filter>, account: &Account,
    ) -> SharedResult<Vec<String>> {
        let mut files = self.all_files()?;

        if let Some(filter) = &filter {
            match filter {
                Filter::DocumentsOnly => files.retain(|f| f.is_document()),
                Filter::FoldersOnly => files.retain(|f| f.is_folder()),
                Filter::LeafNodesOnly => {
                    let mut retained = self.owned_ids();
                    for file in &files {
                        retained.remove(file.parent());
                    }
                    files.retain(|f| retained.contains(f.id()))
                }
            }
        }

        let mut paths = vec![];

        let ids: Vec<Uuid> = files.into_iter().map(|f| *f.id()).collect();

        for id in ids {
            paths.push(self.id_to_path(&id, account)?);
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

pub fn filter_from_str(input: &str) -> SharedResult<Option<Filter>> {
    match input {
        "DocumentsOnly" => Ok(Some(Filter::DocumentsOnly)),
        "FoldersOnly" => Ok(Some(Filter::FoldersOnly)),
        "LeafNodesOnly" => Ok(Some(Filter::LeafNodesOnly)),
        "Unfiltered" => Ok(None),
        _ => Err(SharedError::Unexpected("unknown filter")),
    }
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
