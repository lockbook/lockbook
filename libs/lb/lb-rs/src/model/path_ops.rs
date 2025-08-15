use crate::model::access_info::UserAccessMode;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::lazy::{LazyStaged1, LazyTree};
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use crate::model::{symkey, validate};
use crate::service::keychain::Keychain;
use std::collections::HashSet;
use uuid::Uuid;

use super::ValidationFailure;
use super::signed_meta::SignedMeta;

impl<T> LazyTree<T>
where
    T: TreeLike<F = SignedMeta>,
{
    pub fn path_to_id(&mut self, path: &str, root: &Uuid, keychain: &Keychain) -> LbResult<Uuid> {
        let mut current = *root;
        'path: for name in split_path(path) {
            let id = if let FileType::Link { target } = self.find(&current)?.file_type() {
                target
            } else {
                current
            };
            'child: for child in self.children(&id)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name_using_links(&child, keychain)? == name {
                    current = match self.find(&child)?.file_type() {
                        FileType::Link { target } => target,
                        _ => child,
                    };

                    continue 'path;
                }
            }

            return Err(LbErrKind::FileNonexistent.into());
        }

        Ok(current)
    }

    pub fn id_to_path(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<String> {
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
            let current_meta = if let Some(link) = self.linked_by(&current)? {
                self.find(&link)?
            } else {
                self.find(&current)?
            };
            if self.maybe_find(current_meta.parent()).is_none() {
                return Err(LbErrKind::FileParentNonexistent.into());
            }
            if current_meta.is_root() {
                return Ok(path);
            }
            let next = *current_meta.parent();
            let current_name = self.name_using_links(&current, keychain)?;
            path = format!("/{current_name}{path}");
            current = next;
        }
    }

    pub fn list_paths(
        &mut self, filter: Option<Filter>, keychain: &Keychain,
    ) -> LbResult<Vec<(Uuid, String)>> {
        // Deal with filter
        let filtered = match filter {
            Some(Filter::DocumentsOnly) => {
                let mut ids = vec![];
                for id in self.ids() {
                    if self.find(&id)?.is_document() {
                        ids.push(id);
                    }
                }
                ids
            }
            Some(Filter::FoldersOnly) => {
                let mut ids = vec![];
                for id in self.ids() {
                    if self.find(&id)?.is_folder() {
                        ids.push(id);
                    }
                }
                ids
            }
            Some(Filter::LeafNodesOnly) => {
                let mut retained: Vec<_> = self.ids().into_iter().collect();
                for id in self.ids() {
                    let parent = self.find(&id)?.parent();
                    retained.retain(|fid| fid != parent);
                }
                retained
            }
            None => self.ids(),
        };

        let mut paths = vec![];
        for id in filtered {
            if !self.is_invisible_id(id)? {
                paths.push((id, self.id_to_path(&id, keychain)?));
            }
        }

        Ok(paths)
    }
}

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: TreeLike<F = SignedMeta>,
    Local: TreeLikeMut<F = Base::F>,
{
    pub fn create_link_at_path(
        &mut self, path: &str, target_id: Uuid, root: &Uuid, keychain: &Keychain,
    ) -> LbResult<Uuid> {
        validate::path(path)?;
        let file_type = FileType::Link { target: target_id };
        let path_components = split_path(path);
        self.create_at_path_helper(file_type, path_components, root, keychain)
    }

    pub fn create_at_path(
        &mut self, path: &str, root: &Uuid, keychain: &Keychain,
    ) -> LbResult<Uuid> {
        validate::path(path)?;
        let file_type = if path.ends_with('/') { FileType::Folder } else { FileType::Document };
        let path_components = split_path(path);
        self.create_at_path_helper(file_type, path_components, root, keychain)
    }

    fn create_at_path_helper(
        &mut self, file_type: FileType, path_components: Vec<&str>, root: &Uuid,
        keychain: &Keychain,
    ) -> LbResult<Uuid> {
        let mut current = *root;

        'path: for index in 0..path_components.len() {
            'child: for child in self.children(&current)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name_using_links(&child, keychain)? == path_components[index] {
                    if index == path_components.len() - 1 {
                        return Err(LbErrKind::Validation(ValidationFailure::PathConflict(
                            HashSet::from([child]),
                        )))?;
                    }

                    current = match self.find(&child)?.file_type() {
                        FileType::Document => {
                            return Err(LbErrKind::Validation(
                                ValidationFailure::NonFolderWithChildren(child),
                            ))?;
                        }
                        FileType::Folder => child,
                        FileType::Link { target } => {
                            let current = self.find(&target)?;
                            if current.access_mode(&Owner(keychain.get_pk()?))
                                < Some(UserAccessMode::Write)
                            {
                                return Err(LbErrKind::InsufficientPermission.into());
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
                keychain,
            )?;
        }

        Ok(current)
    }
}

#[derive(Debug)]
pub enum Filter {
    DocumentsOnly,
    FoldersOnly,
    LeafNodesOnly,
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
