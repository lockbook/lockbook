use sled::Db;
use uuid::Uuid;

use crate::model::local_changes::{LocalChange, Moved, Renamed};

#[derive(Debug)]
pub enum DbError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

pub trait LocalChangesRepo {
    fn get_all_local_changes(db: &Db) -> Result<Vec<LocalChange>, DbError>;
    fn get_local_changes(db: &Db, id: Uuid) -> Result<Option<LocalChange>, DbError>;
    fn track_new_file(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn track_rename(db: &Db, id: Uuid, old_name: String) -> Result<(), DbError>;
    fn track_move(db: &Db, id: Uuid, old_parent: Uuid) -> Result<(), DbError>;
    fn track_edit(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn track_delete(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_new_file(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_rename(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_move(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_edit(db: &Db, id: Uuid) -> Result<(), DbError>;
    fn untrack_delete(db: &Db, id: Uuid) -> Result<(), DbError>;
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
            content_edited: false,
            deleted: false,
        };

        tree.insert(
            id.as_bytes(),
            serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::SledError)?;
        Ok(())
    }

    fn track_rename(db: &Db, id: Uuid, old_name: String) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: Some(Renamed::from(old_name)),
                    moved: None,
                    new: false,
                    content_edited: false,
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
                Some(_) => Ok(()),
            },
        }
    }

    fn track_move(db: &Db, id: Uuid, old_parent: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: None,
                    moved: Some(Moved::from(old_parent)),
                    new: false,
                    content_edited: false,
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
                Some(_) => Ok(()),
            },
        }
    }

    fn track_edit(db: &Db, id: Uuid) -> Result<(), DbError> {
        let tree = db.open_tree(LOCAL_CHANGES).map_err(DbError::SledError)?;

        match Self::get_local_changes(&db, id)? {
            None => {
                let new_local_change = LocalChange {
                    id,
                    renamed: None,
                    moved: None,
                    new: false,
                    content_edited: true,
                    deleted: false,
                };
                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(mut change) => {
                if change.content_edited {
                    Ok(())
                } else {
                    change.content_edited = true;
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
            }
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
                    content_edited: false,
                    deleted: true,
                };
                tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::SledError)?;
                Ok(())
            }
            Some(mut change) => {
                if change.deleted {
                    Ok(())
                } else {
                    change.deleted = true;
                    tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::SledError)?;
                    Ok(())
                }
            }
        }
    }

    fn untrack_new_file(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
    }

    fn untrack_rename(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
    }

    fn untrack_move(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
    }

    fn untrack_edit(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
    }

    fn untrack_delete(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
    }

    fn delete_if_exists(_db: &Db, _id: Uuid) -> Result<(), DbError> {
        unimplemented!()
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

    #[test]
    fn local_changes_runthrough() {
        let id = Uuid::new_v4();
        let db = DefaultDbProvider::connect_to_db(&dummy_config()).unwrap();
        assert_eq!(
            LocalChangesRepoImpl::get_all_local_changes(&db)
                .unwrap()
                .len(),
            0
        );

        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        LocalChangesRepoImpl::track_new_file(&db, id).unwrap();
        assert_eq!(
            LocalChangesRepoImpl::get_all_local_changes(&db)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: false,
                deleted: false
            })
        );

        let id2 = Uuid::new_v4();
        LocalChangesRepoImpl::track_rename(&db, id, String::from("old_file")).unwrap();
        LocalChangesRepoImpl::track_rename(&db, id2, String::from("old_file2")).unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: None,
                new: true,
                content_edited: false,
                deleted: false
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id2).unwrap(),
            Some(LocalChange {
                id: id2,
                renamed: Some(Renamed::from("old_file2")),
                moved: None,
                new: false,
                content_edited: false,
                deleted: false
            })
        );

        let id3 = Uuid::new_v4();
        LocalChangesRepoImpl::track_move(&db, id, id2).unwrap();
        LocalChangesRepoImpl::track_move(&db, id3, id2).unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: false,
                deleted: false
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id3).unwrap(),
            Some(LocalChange {
                id: id3,
                renamed: None,
                moved: Some(Moved::from(id2)),
                new: false,
                content_edited: false,
                deleted: false
            })
        );

        let id4 = Uuid::new_v4();
        LocalChangesRepoImpl::track_edit(&db, id).unwrap();
        LocalChangesRepoImpl::track_edit(&db, id4).unwrap();

        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id).unwrap(),
            Some(LocalChange {
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: true,
                deleted: false
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id4).unwrap(),
            Some(LocalChange {
                id: id4,
                renamed: None,
                moved: None,
                new: false,
                content_edited: true,
                deleted: false
            })
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
                content_edited: true,
                deleted: true
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_local_changes(&db, id5).unwrap(),
            Some(LocalChange {
                id: id5,
                renamed: None,
                moved: None,
                new: false,
                content_edited: false,
                deleted: true
            })
        );
        assert_eq!(
            LocalChangesRepoImpl::get_all_local_changes(&db)
                .unwrap()
                .len(),
            5
        );
    }
}
