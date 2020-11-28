use std::time::SystemTimeError;

use uuid::Uuid;

use crate::model::crypto::{EncryptedDocument, UserAccessInfo};
use crate::model::file_metadata::FileType;
use crate::model::local_changes::{Edited, LocalChange, Moved, Renamed};
use crate::service::clock_service::Clock;
use crate::storage::db_provider;
use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum DbError {
    TimeError(SystemTimeError),
    BackendError(db_provider::BackendError),
    SerdeError(serde_json::Error),
}

pub trait LocalChangesRepo {
    fn get_all_local_changes(backend: &Backend) -> Result<Vec<LocalChange>, DbError>;
    fn get_local_changes(backend: &Backend, id: Uuid) -> Result<Option<LocalChange>, DbError>;
    fn track_new_file(backend: &Backend, id: Uuid) -> Result<(), DbError>;
    fn track_rename(
        backend: &Backend,
        id: Uuid,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), DbError>;
    fn track_move(
        backend: &Backend,
        id: Uuid,
        old_parent: Uuid,
        new_parent: Uuid,
    ) -> Result<(), DbError>;
    fn track_edit(
        backend: &Backend,
        id: Uuid,
        old_version: &EncryptedDocument,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError>;
    fn track_delete(backend: &Backend, id: Uuid, file_type: FileType) -> Result<(), DbError>;
    fn untrack_new_file(backend: &Backend, id: Uuid) -> Result<(), DbError>;
    fn untrack_rename(backend: &Backend, id: Uuid) -> Result<(), DbError>;
    fn untrack_move(backend: &Backend, id: Uuid) -> Result<(), DbError>;
    fn untrack_edit(backend: &Backend, id: Uuid) -> Result<(), DbError>;
    fn delete(backend: &Backend, id: Uuid) -> Result<(), DbError>;
}

pub struct LocalChangesRepoImpl<Time: Clock> {
    _clock: Time,
}

static LOCAL_CHANGES: &[u8; 13] = b"local_changes";

impl<Time: Clock> LocalChangesRepo for LocalChangesRepoImpl<Time> {
    fn get_all_local_changes(backend: &Backend) -> Result<Vec<LocalChange>, DbError> {
        let mut value = backend
            .dump::<_, Vec<u8>>(LOCAL_CHANGES)
            .map_err(DbError::BackendError)?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(DbError::SerdeError))
            .collect::<Result<Vec<LocalChange>, DbError>>()?;

        value.sort_by(|change1, change2| change1.timestamp.cmp(&change2.timestamp));

        Ok(value)
    }

    fn get_local_changes(backend: &Backend, id: Uuid) -> Result<Option<LocalChange>, DbError> {
        let maybe_value: Option<Vec<u8>> = backend
            .read(LOCAL_CHANGES, id.as_bytes())
            .map_err(DbError::BackendError)?;
        match maybe_value {
            None => Ok(None),
            Some(value) => {
                let change: LocalChange =
                    serde_json::from_slice(value.as_ref()).map_err(DbError::SerdeError)?;
                Ok(Some(change))
            }
        }
    }

    fn track_new_file(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        let new_local_change = LocalChange {
            timestamp: Time::get_time(),
            id,
            renamed: None,
            moved: None,
            new: true,
            content_edited: None,
            deleted: false,
        };

        backend
            .write(
                LOCAL_CHANGES,
                id.as_bytes(),
                serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
            )
            .map_err(DbError::BackendError)?;
        Ok(())
    }

    fn track_rename(
        backend: &Backend,
        id: Uuid,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), DbError> {
        if old_name == new_name {
            return Ok(());
        }

        match Self::get_local_changes(backend, id)? {
            None => {
                let new_local_change = LocalChange {
                    timestamp: Time::get_time(),
                    id,
                    renamed: Some(Renamed::from(old_name)),
                    moved: None,
                    new: false,
                    content_edited: None,
                    deleted: false,
                };

                backend
                    .write(
                        LOCAL_CHANGES,
                        id.as_bytes(),
                        serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => match change.renamed {
                None => {
                    change.renamed = Some(Renamed::from(old_name));
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                    Ok(())
                }
                Some(renamed) => {
                    if new_name == renamed.old_value {
                        Self::untrack_rename(backend, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_move(
        backend: &Backend,
        id: Uuid,
        old_parent: Uuid,
        new_parent: Uuid,
    ) -> Result<(), DbError> {
        if old_parent == new_parent {
            return Ok(());
        }

        match Self::get_local_changes(backend, id)? {
            None => {
                let new_local_change = LocalChange {
                    timestamp: Time::get_time(),
                    id,
                    renamed: None,
                    moved: Some(Moved::from(old_parent)),
                    new: false,
                    content_edited: None,
                    deleted: false,
                };

                backend
                    .write(
                        LOCAL_CHANGES,
                        id.as_bytes(),
                        serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => match change.moved {
                None => {
                    change.moved = Some(Moved::from(old_parent));
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                    Ok(())
                }
                Some(moved) => {
                    if moved.old_value == new_parent {
                        Self::untrack_move(backend, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_edit(
        backend: &Backend,
        id: Uuid,
        old_version: &EncryptedDocument,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError> {
        if old_content_checksum == new_content_checksum {
            return Ok(());
        }

        match Self::get_local_changes(backend, id)? {
            None => {
                let new_local_change = LocalChange {
                    timestamp: Time::get_time(),
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
                backend
                    .write(
                        LOCAL_CHANGES,
                        id.as_bytes(),
                        serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => match change.content_edited {
                None => {
                    change.content_edited = Some(Edited {
                        old_value: old_version.clone(),
                        access_info: access_info_for_old_version.clone(),
                        old_content_checksum,
                    });
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                    Ok(())
                }
                Some(edited) => {
                    if edited.old_content_checksum == new_content_checksum {
                        Self::untrack_edit(backend, id)
                    } else {
                        Ok(())
                    }
                }
            },
        }
    }

    fn track_delete(backend: &Backend, id: Uuid, file_type: FileType) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => {
                let new_local_change = LocalChange {
                    timestamp: Time::get_time(),
                    id,
                    renamed: None,
                    moved: None,
                    new: false,
                    content_edited: None,
                    deleted: true,
                };
                backend
                    .write(
                        LOCAL_CHANGES,
                        id.as_bytes(),
                        serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => {
                if change.deleted {
                    Ok(())
                } else if file_type == FileType::Document {
                    if change.new {
                        // If a document was created and deleted, just forget about it
                        Self::delete(backend, id)
                    } else {
                        // If a document was deleted, don't bother pushing it's rename / move
                        let delete_tracked = LocalChange {
                            timestamp: Time::get_time(),
                            id,
                            renamed: None,
                            moved: None,
                            new: false,
                            content_edited: None,
                            deleted: true,
                        };
                        backend
                            .write(
                                LOCAL_CHANGES,
                                id.as_bytes(),
                                serde_json::to_vec(&delete_tracked).map_err(DbError::SerdeError)?,
                            )
                            .map_err(DbError::BackendError)?;
                        Ok(())
                    }
                } else {
                    change.deleted = true;
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                    Ok(())
                }
            }
        }
    }

    fn untrack_new_file(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut new) => {
                new.new = false;

                if !new.deleted {
                    Self::delete(backend, new.id)?
                } else {
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&new).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_rename(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.renamed = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                }
                Ok(())
            }
        }
    }

    fn untrack_move(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.moved = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_edit(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.content_edited = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    backend
                        .write(
                            LOCAL_CHANGES,
                            id.as_bytes(),
                            serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn delete(backend: &Backend, id: Uuid) -> Result<(), DbError> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(_) => {
                backend
                    .delete(LOCAL_CHANGES, id.as_bytes())
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::file_metadata::FileType::{Document, Folder};
    use crate::model::local_changes::{LocalChange, Moved, Renamed};
    use crate::model::state::temp_config;
    use crate::repo::local_changes_repo::{LocalChangesRepo, LocalChangesRepoImpl};
    use crate::service::clock_service::Clock;
    use crate::storage::db_provider::{to_backend, DbProvider, DiskBackedDB};

    type DefaultDbProvider = DiskBackedDB;

    pub struct TestClock;

    impl Clock for TestClock {
        fn get_time() -> i64 {
            0
        }
    }

    pub type TestLocalChangesRepo = LocalChangesRepoImpl<TestClock>;

    macro_rules! assert_total_local_changes (
        ($db:expr, $total:literal) => {
            assert_eq!(
                TestLocalChangesRepo::get_all_local_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    #[test]
    fn set_and_unset_fields() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        assert_total_local_changes!(backend, 0);

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(backend, id).unwrap();
        TestLocalChangesRepo::track_new_file(backend, id).unwrap();
        TestLocalChangesRepo::track_new_file(backend, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_rename(backend, id, "old_file", "unused_name").unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        assert_total_local_changes!(backend, 1);

        let id2 = Uuid::new_v4();
        TestLocalChangesRepo::track_move(backend, id, id2, Uuid::new_v4()).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: false,
            })
        );

        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::untrack_edit(backend, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: Some(Renamed::from("old_file")),
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: false,
            })
        );

        TestLocalChangesRepo::untrack_rename(backend, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: Some(Moved::from(id2)),
                new: true,
                content_edited: None,
                deleted: false,
            })
        );

        TestLocalChangesRepo::untrack_move(backend, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );

        TestLocalChangesRepo::untrack_new_file(backend, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            None
        );
        assert_total_local_changes!(backend, 0);

        // Deleting a file should unset it's other fields
        TestLocalChangesRepo::track_rename(backend, id, "old", "new").unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: Some(Renamed::from("old")),
                moved: None,
                new: false,
                content_edited: None,
                deleted: false,
            })
        );
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_delete(backend, id, Document).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: false,
                content_edited: None,
                deleted: true,
            })
        );
        assert_total_local_changes!(backend, 1);
    }

    #[test]
    fn new_document_deleted() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(backend, id).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        TestLocalChangesRepo::track_delete(backend, id, Document).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            None
        );
    }

    #[test]
    fn new_folder_deleted() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(backend, id).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: false,
            })
        );
        TestLocalChangesRepo::track_delete(backend, id, Folder).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, id).unwrap(),
            Some(LocalChange {
                timestamp: 0,
                id,
                renamed: None,
                moved: None,
                new: true,
                content_edited: None,
                deleted: true,
            })
        );
    }

    #[test]
    fn track_changes_on_multiple_files() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let id1 = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(backend, id1).unwrap();
        assert_total_local_changes!(backend, 1);

        let id2 = Uuid::new_v4();
        TestLocalChangesRepo::track_rename(backend, id2, "old", "new").unwrap();
        assert_total_local_changes!(backend, 2);

        let id3 = Uuid::new_v4();
        TestLocalChangesRepo::track_move(backend, id3, id3, Uuid::new_v4()).unwrap();
        assert_total_local_changes!(backend, 3);

        let id4 = Uuid::new_v4();
        TestLocalChangesRepo::track_delete(backend, id4, Document).unwrap();
        assert_total_local_changes!(backend, 4);

        TestLocalChangesRepo::untrack_new_file(backend, id1).unwrap();
        assert_total_local_changes!(backend, 3);

        TestLocalChangesRepo::untrack_rename(backend, id2).unwrap();
        assert_total_local_changes!(backend, 2);

        TestLocalChangesRepo::untrack_move(backend, id3).unwrap();
        assert_total_local_changes!(backend, 1);

        // Untrack not supported because no one can undelete files
    }

    #[test]
    fn unknown_id() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let the_wrong_id = Uuid::new_v4();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, the_wrong_id).unwrap(),
            None
        );
        TestLocalChangesRepo::untrack_edit(backend, the_wrong_id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(backend, the_wrong_id).unwrap(),
            None
        );
        assert_total_local_changes!(backend, 0);
    }

    #[test]
    fn rename_back_to_original() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let id = Uuid::new_v4();

        TestLocalChangesRepo::track_rename(backend, id, "old_file", "new_name").unwrap();
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_rename(backend, id, "garbage", "garbage2").unwrap();
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_rename(backend, id, "garbage", "old_file").unwrap();
        assert_total_local_changes!(backend, 0);
    }

    #[test]
    fn move_back_to_original() {
        let db = &DefaultDbProvider::connect_to_db(&temp_config()).unwrap();
        let backend = &to_backend(db);

        let id = Uuid::new_v4();
        let og = Uuid::new_v4();

        TestLocalChangesRepo::track_move(backend, id, og, Uuid::new_v4()).unwrap();
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_move(backend, id, Uuid::new_v4(), Uuid::new_v4()).unwrap();
        assert_total_local_changes!(backend, 1);

        TestLocalChangesRepo::track_move(backend, id, Uuid::new_v4(), og).unwrap();
        assert_total_local_changes!(backend, 0);
    }
}
