use std::collections::HashSet;

use libsecp256k1::PublicKey;
use uuid::Uuid;

use crate::access_info::UserAccessMode;
use crate::account::Account;
use crate::file_like::FileLike;
use crate::file_metadata::{FileType, Owner};
use crate::lazy::LazyStaged1;
use crate::signed_file::SignedFile;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::{symkey, validate, SharedError, SharedResult};

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: TreeLike<F = SignedFile>,
    Local: TreeLike<F = Base::F>,
{
    pub fn path_to_id(&mut self, path: &str, root: &Uuid, account: &Account) -> SharedResult<Uuid> {
        let mut current = *root;
        'path: for name in split_path(path) {
            'child: for child in self.children(&if let FileType::Link { target } =
                self.find(&current)?.file_type()
            {
                target
            } else {
                current
            })? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name_using_links(&child, account)? == name {
                    current = child;
                    continue 'path;
                }
            }

            return Err(SharedError::FileNonexistent);
        }

        Ok(current)
    }

    pub fn id_to_path(&mut self, id: &Uuid, account: &Account) -> SharedResult<String> {
        let meta = self.find(id)?;

        if meta.is_root() {
            return Ok("/".to_string());
        }

        let mut path = match meta.file_type() {
            FileType::Document => "",
            FileType::Folder => "/",
            FileType::Link { target } => match self.find(&target)?.file_type() {
                FileType::Document | FileType::Link { .. } => "",
                FileType::Folder => "/",
            },
        }
        .to_string();

        let mut current = *meta.id();
        loop {
            let current_meta = if let Some(link) = self.link(&current)? {
                self.find(&link)?
            } else {
                self.find(&current)?
            };
            if self.maybe_find(current_meta.parent()).is_none() {
                return Err(SharedError::FileParentNonexistent);
            }
            if current_meta.is_root() {
                return Ok(path);
            }
            let next = *current_meta.parent();
            let current_name = self.name_using_links(&current, account)?;
            path = format!("/{}{}", current_name, path);
            current = next;
        }
    }

    pub fn list_paths(
        &mut self, filter: Option<Filter>, account: &Account,
    ) -> SharedResult<Vec<String>> {
        // Deal with filter
        let filtered = match filter {
            Some(Filter::DocumentsOnly) => {
                let mut ids = HashSet::new();
                for id in self.ids() {
                    if self.find(id)?.is_document() {
                        ids.insert(*id);
                    }
                }
                ids
            }
            Some(Filter::FoldersOnly) => {
                let mut ids = HashSet::new();
                for id in self.ids() {
                    if self.find(id)?.is_folder() {
                        ids.insert(*id);
                    }
                }
                ids
            }
            Some(Filter::LeafNodesOnly) => {
                let mut retained = self.owned_ids();
                for id in self.ids() {
                    retained.remove(self.find(id)?.parent());
                }
                retained
            }
            None => self.owned_ids(),
        };

        // remove deleted; include links not linked files
        let mut paths = vec![];
        for id in filtered.clone() {
            let id = match self.link(&id)? {
                None => id,
                Some(link) => {
                    if filtered.contains(&link) {
                        continue;
                    }
                    link
                }
            };

            if !self.calculate_deleted(&id)? && !self.in_pending_share(&id)? {
                paths.push(self.id_to_path(&id, account)?);
            }
        }

        Ok(paths)
    }
}

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: TreeLike<F = SignedFile>,
    Local: TreeLikeMut<F = Base::F>,
{
    pub fn create_link_at_path(
        &mut self, path: &str, target_id: Uuid, root: &Uuid, account: &Account, pub_key: &PublicKey,
    ) -> SharedResult<Uuid> {
        validate::path(path)?;
        let file_type = FileType::Link { target: target_id };
        let path_components = split_path(path);
        self.create_at_path_helper(file_type, path_components, root, account, pub_key)
    }

    pub fn create_at_path(
        &mut self, path: &str, root: &Uuid, account: &Account, pub_key: &PublicKey,
    ) -> SharedResult<Uuid> {
        validate::path(path)?;
        let file_type = if path.ends_with('/') { FileType::Folder } else { FileType::Document };
        let path_components = split_path(path);
        self.create_at_path_helper(file_type, path_components, root, account, pub_key)
    }

    fn create_at_path_helper(
        &mut self, file_type: FileType, path_components: Vec<&str>, root: &Uuid, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<Uuid> {
        let mut current = *root;
        'path: for index in 0..path_components.len() {
            'child: for child in self.children(&current)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name_using_links(&child, account)? == path_components[index] {
                    if index == path_components.len() - 1 {
                        return Err(SharedError::PathTaken);
                    }

                    current = match self.find(&child)?.file_type() {
                        FileType::Document => {
                            return Err(SharedError::FileNotFolder);
                        }
                        FileType::Folder => child,
                        FileType::Link { target } => {
                            let current = self.find(&target)?;
                            if current.access_mode(&Owner(*pub_key)) < Some(UserAccessMode::Write) {
                                return Err(SharedError::InsufficientPermission);
                            }
                            *current.id()
                        }
                    };
                    continue 'path;
                }
            }

            // Child does not exist, create it
            let this_file_type =
                if index != path_components.len() - 1 { FileType::Folder } else { file_type };

            current = self.create(
                Uuid::new_v4(),
                symkey::generate_key(),
                &current,
                path_components[index],
                this_file_type,
                account,
            )?;
        }

        Ok(current)
    }
}

#[derive(Debug, PartialEq, Eq)]
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
