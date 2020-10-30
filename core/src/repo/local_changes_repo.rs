use sled::Db;
use uuid::Uuid;

use crate::model::crypto::{Document, UserAccessInfo};
use crate::model::local_changes::{Edited, LocalChange, Moved, Renamed};

#[derive(Debug)]
pub enum DbError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

pub trait LocalChangesRepo {
    fn get_all_local_changes(db: &Db) -> Result<Vec<LocalChange>, DbError>;
    fn get_local_changes(db: &Db, id: Uuid) -> Result<Option<LocalChange>, DbError>;
    fn track_new_file(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn track_rename(db: &Db, id: Uuid, old_name: &str, new_name: &str) -> Result<(), DbError>;
    fn track_move(db: &Db, id: Uuid, old_parent: Uuid, new_parent: Uuid) -> Result<(), DbError>;
    fn track_edit(
        db: &Db,
        id: Uuid,
        old_version: &Document,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError>;
    fn track_delete(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_new_file(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_rename(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_move(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_edit(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn delete_if_exists(db: &Db, id: Uuid) -> Result<(), DbError>;
}

pub struct LocalChangesRepoImpl;

static LOCAL_CHANGES: &[u8; 13] = b"local_changes";

impl LocalChangesRepo for LocalChangesRepoImpl {
    fn get_all_local_changes(db: &Db) -> Result<Vec<LocalChange>, DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;
        let value = tree
            .iter()
            .map(|s| {
                let change: LocalChange = serde_json::from_slice(s.unwrap().1.as_ref()).unwrap();
                change
            })
            .collect::<Vec<LocalChange>>();

        Ok(value)
    }

    fn get_local_changes(db: &Db, id: Uuid) -> Result<Option<LocalChange>, DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;
        let maybe_value = tree.get(id.as_bytes()).map_err(DbError::SledError)?;
        match maybe_value {
            None => Ok(None),
            Some(value) => {
                let change: LocalChange =
                    serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?;
                Ok(Some(change))
            }
        }
    }

    fn track_new_file(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        let new_local_change = LocalChange {
            id,
            renamed: None,
            moved: None,
            new: true,
            content_edited: None,
            deleted: false,
        };

        tree.insert(
            id.as_bytes(),
            serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::SledError)?;
        Ok(())
    }

    fn track_rename(db: &Db, id: Uuid, old_name: &str, new_name: &str) -> Result<(), DbError> {
        if old_name == new_name {
            return Ok(());
        }

        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: Some(Renamed::from(old_name)),
                    moved: None,
                    new: false,
                    content_edited: None,
                    deleted: false,
                };

                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(mut change) => match change.renamed {
                None => {
                    change.renamed = Some(Renamed::from(old_name));
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
                Some(renamed) => {
                    if new_name == renamed.old_value {
                        Self::untrack_rename(&db, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_move(db: &Db, id: Uuid, old_parent: Uuid, new_parent: Uuid) -> Result<(), DbError> {
        if old_parent == new_parent {
            return Ok(());
        }

        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: None,
                    moved: Some(Moved::from(old_parent)),
                    new: false,
                    content_edited: None,
                    deleted: false,
                };

                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(mut change) => match change.moved {
                None => {
                    change.moved = Some(Moved::from(old_parent));
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
                Some(moved) => {
                    if moved.old_value == new_parent {
                        Self::untrack_move(&db, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_edit(
        db: &Db,
        id: Uuid,
        old_version: &Document,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError> {
        if old_content_checksum == new_content_checksum {
            return Ok(());
        }

        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: None,
                    moved: None,
                    new: false,
                    content_edited: Some(Edited {
                        old_value: old_version.clone(),
                        access_info: access_info_for_old_version.clone(),
                        old_content_checksum,
                    }),
                    deleted: false,
                };
                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(mut change) => match change.content_edited {
                None => {
                    change.content_edited = Some(Edited {
                        old_value: old_version.clone(),
                        access_info: access_info_for_old_version.clone(),
                        old_content_checksum,
                    });
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
                Some(edited) => {
                    if edited.old_content_checksum == new_content_checksum {
                        Self::untrack_edit(&db, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_delete(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: None,
                    moved: None,
                    new: false,
                    content_edited: None,
                    deleted: true,
                };
                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(change) => {
                if change.deleted {
                    Ok(())
                } else if change.new {
                    Self::delete_if_exists(&db, id)
                } else {
                    let delete_tracked = LocalChange {
                        id,
                        renamed: None,
                        moved: None,
                        new: false,
                        content_edited: None,
                        deleted: true,
                    };
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&delete_tracked).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
            }
        }
    }

    fn untrack_new_file(db: &Db, id: Uuid) -> Result<(), DbError> {
        Self::delete_if_exists(&db, id)
    }

    fn untrack_rename(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.renamed = None;

                if edit.ready_to_be_deleted() {
                    Self::delete_if_exists(&db, edit.id)?
                } else {
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_move(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.moved = None;

                if edit.ready_to_be_deleted() {
                    Self::delete_if_exists(&db, edit.id)?
                } else {
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_edit(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.content_edited = None;

                if edit.ready_to_be_deleted() {
                    Self::delete_if_exists(&db, edit.id)?
                } else {
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                }

                Ok(())
            }
        }
    }

    fn delete_if_exists(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => Ok(()),
            Some(_) => {
                tree.remove(id.as_bytes()).map_err(DbError::SledError)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::local_changes::{LocalChange, Moved, Renamed};
    use crate::model::state::dummy_config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::local_changes_repo::{LocalChangesRepo, LocalChangesRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    macro_rules! assert_total_local_changes (
        ($db:expr, $total:literal) => {
            assert_eq!(
                LocalChangesRepoImpl::get_all_local_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    #[test]
    fn local_changes_runthrough() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        assert_total_local_changes!(&db, 0);

        let id = Uuid::new_v4();
        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        assert_total_local_changes!(&db, 1);
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );

        let id2 = Uuid::new_v4();
        LocalChangesRepoImpl::track_rename(&db, id, "old_file", "unused_name").unwrap();
        LocalChangesRepoImpl::track_rename(&db, id2, "old_file2", "unused_name").unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id2).unwrap(),
            Some(LocalChange {
                id: id2,
                renamed: Some(Renamed::from("old_file2")),
                moved: None,
                new: false,
                content_edited: None,
                deleted: false,
            })
        );

        let id3 = Uuid::new_v4();
        LocalChangesRepoImpl::track_move(&db, id, id2, Uuid::new_v4()).unwrap();
        LocalChangesRepoImpl::track_move(&db, id3, id2, Uuid::new_v4()).unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id3).unwrap(),
            Some(LocalChange {
                id: id3,
                renamed: None,
                moved: Some(Moved::from(id2)),
                new: false,
                content_edited: None,
                deleted: false,
            })
        );

        let id4 = Uuid::new_v4();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id4).unwrap(),
            None
        );

        let id5 = Uuid::new_v4();
        LocalChangesRepoImpl::track_delete(&db, id).unwrap();
        LocalChangesRepoImpl::track_delete(&db, id5).unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: true,
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id5).unwrap(),
            Some(LocalChange {
                id: id5,
                renamed: None,
                moved: None,
                new: false,
                content_edited: None,
                deleted: true,
            })
        );
        assert_total_local_changes!(&db, 4);

        LocalChangesRepoImpl::untrack_edit(&db, id4).unwrap();
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id4).unwrap(),
            None
        );
        assert_total_local_changes!(&db, 4);

        LocalChangesRepoImpl::untrack_edit(&db, id).unwrap();
        assert_total_local_changes!(&db, 4);

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: true,
            })
        );
    }

    #[test]
    fn rename_back_to_original() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        let id = Uuid::new_v4();

        LocalChangesRepoImpl::track_rename(&db, id, "old_file", "new_name").unwrap();
        assert_total_local_changes!(&db, 1);

        LocalChangesRepoImpl::track_rename(&db, id, "garbage", "garbage2").unwrap();
        assert_total_local_changes!(&db, 1);

        LocalChangesRepoImpl::track_rename(&db, id, "garbage", "old_file").unwrap();
        assert_total_local_changes!(&db, 0);
    }

    #[test]
    fn move_back_to_original() {
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        let id = Uuid::new_v4();
        let og = Uuid::new_v4();

        LocalChangesRepoImpl::track_move(&db, id, og, Uuid::new_v4()).unwrap();
        assert_total_local_changes!(&db, 1);

        LocalChangesRepoImpl::track_move(&db, id, Uuid::new_v4(), Uuid::new_v4()).unwrap();
        assert_total_local_changes!(&db, 1);

        LocalChangesRepoImpl::track_move(&db, id, Uuid::new_v4(), og).unwrap();
        assert_total_local_changes!(&db, 0);
    }
}
