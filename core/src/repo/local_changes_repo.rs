use std::time::SystemTimeError;

use uuid::Uuid;

use crate::storage::db_provider::Backend;
use lockbook_crypto::clock_service::Clock;
use lockbook_models::crypto::{EncryptedDocument, UserAccessInfo};
use lockbook_models::file_metadata::FileType;
use lockbook_models::local_changes::{Edited, LocalChange, Moved, Renamed};
use std::{thread, time};

#[derive(Debug)]
pub enum DbError<MyBackend: Backend> {
    TimeError(SystemTimeError),
    BackendError(MyBackend::Error),
    SerdeError(serde_json::Error),
}

pub trait LocalChangesRepo<MyBackend: Backend> {
    fn get_all_local_changes(
        backend: &MyBackend::Db,
    ) -> Result<Vec<LocalChange>, DbError<MyBackend>>;
    fn get_local_changes(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<Option<LocalChange>, DbError<MyBackend>>;
    fn track_new_file(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
    fn track_rename(
        backend: &MyBackend::Db,
        id: Uuid,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), DbError<MyBackend>>;
    fn track_move(
        backend: &MyBackend::Db,
        id: Uuid,
        old_parent: Uuid,
        new_parent: Uuid,
    ) -> Result<(), DbError<MyBackend>>;
    fn track_edit(
        backend: &MyBackend::Db,
        id: Uuid,
        old_version: &EncryptedDocument,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError<MyBackend>>;
    fn track_delete(
        backend: &MyBackend::Db,
        id: Uuid,
        file_type: FileType,
    ) -> Result<(), DbError<MyBackend>>;
    fn untrack_new_file(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
    fn untrack_rename(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
    fn untrack_move(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
    fn untrack_edit(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
    fn delete(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>>;
}

pub struct LocalChangesRepoImpl<Time: Clock, MyBackend: Backend> {
    _clock: Time,
    _backend: MyBackend,
}

static LOCAL_CHANGES: &[u8; 13] = b"local_changes";

impl<Time: Clock, MyBackend: Backend> LocalChangesRepo<MyBackend>
    for LocalChangesRepoImpl<Time, MyBackend>
{
    fn get_all_local_changes(
        backend: &MyBackend::Db,
    ) -> Result<Vec<LocalChange>, DbError<MyBackend>> {
        let mut value = MyBackend::dump::<_, Vec<u8>>(backend, LOCAL_CHANGES)
            .map_err(DbError::BackendError)?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(DbError::SerdeError))
            .collect::<Result<Vec<LocalChange>, DbError<MyBackend>>>()?;

        value.sort_by(|change1, change2| change1.timestamp.cmp(&change2.timestamp));

        Ok(value)
    }

    fn get_local_changes(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<Option<LocalChange>, DbError<MyBackend>> {
        let maybe_value: Option<Vec<u8>> =
            MyBackend::read(backend, LOCAL_CHANGES, id.to_string().as_str())
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

    fn track_new_file(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        let new_local_change = LocalChange {
            timestamp: Time::get_time(),
            id,
            renamed: None,
            moved: None,
            new: true,
            content_edited: None,
            deleted: false,
        };

        MyBackend::write(
            backend,
            LOCAL_CHANGES,
            id.to_string().as_str(),
            serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
        )
        .map_err(DbError::BackendError)?;
        Ok(())
    }

    fn track_rename(
        backend: &MyBackend::Db,
        id: Uuid,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), DbError<MyBackend>> {
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

                MyBackend::write(
                    backend,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => match change.renamed {
                None => {
                    change.renamed = Some(Renamed::from(old_name));
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
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
        backend: &MyBackend::Db,
        id: Uuid,
        old_parent: Uuid,
        new_parent: Uuid,
    ) -> Result<(), DbError<MyBackend>> {
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

                MyBackend::write(
                    backend,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
                    serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(mut change) => match change.moved {
                None => {
                    change.moved = Some(Moved::from(old_parent));
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
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
        backend: &MyBackend::Db,
        id: Uuid,
        old_version: &EncryptedDocument,
        access_info_for_old_version: &UserAccessInfo,
        old_content_checksum: Vec<u8>,
        new_content_checksum: Vec<u8>,
    ) -> Result<(), DbError<MyBackend>> {
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
                MyBackend::write(
                    backend,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
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
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
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

    fn track_delete(
        backend: &MyBackend::Db,
        id: Uuid,
        file_type: FileType,
    ) -> Result<(), DbError<MyBackend>> {
        // Added to ensure that a prior move is at least 1ms older than this delete
        thread::sleep(time::Duration::from_millis(1));

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
                MyBackend::write(
                    backend,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
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
                        MyBackend::write(
                            backend,
                            LOCAL_CHANGES,
                            id.to_string().as_str(),
                            serde_json::to_vec(&delete_tracked).map_err(DbError::SerdeError)?,
                        )
                        .map_err(DbError::BackendError)?;
                        Ok(())
                    }
                } else {
                    change.deleted = true;
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                    Ok(())
                }
            }
        }
    }

    fn untrack_new_file(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut new) => {
                new.new = false;

                if !new.deleted {
                    Self::delete(backend, new.id)?
                } else {
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&new).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_rename(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.renamed = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                }
                Ok(())
            }
        }
    }

    fn untrack_move(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.moved = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn untrack_edit(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(mut edit) => {
                edit.content_edited = None;

                if edit.ready_to_be_deleted() {
                    Self::delete(backend, edit.id)?
                } else {
                    MyBackend::write(
                        backend,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&edit).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                }

                Ok(())
            }
        }
    }

    fn delete(backend: &MyBackend::Db, id: Uuid) -> Result<(), DbError<MyBackend>> {
        match Self::get_local_changes(backend, id)? {
            None => Ok(()),
            Some(_) => {
                MyBackend::delete(backend, LOCAL_CHANGES, id.to_string().as_str())
                    .map_err(DbError::BackendError)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::local_changes_repo::{LocalChangesRepo, LocalChangesRepoImpl};
    use crate::storage::db_provider::Backend;
    use crate::DefaultBackend;
    use lockbook_crypto::clock_service::Clock;
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use lockbook_models::local_changes::{LocalChange, Moved, Renamed};

    pub struct TestClock;

    impl Clock for TestClock {
        fn get_time() -> i64 {
            0
        }
    }

    pub type TestLocalChangesRepo = LocalChangesRepoImpl<TestClock, DefaultBackend>;

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
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        assert_total_local_changes!(db, 0);

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(db, id).unwrap();
        TestLocalChangesRepo::track_new_file(db, id).unwrap();
        TestLocalChangesRepo::track_new_file(db, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_rename(db, id, "old_file", "unused_name").unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        assert_total_local_changes!(db, 1);

        let id2 = Uuid::new_v4();
        TestLocalChangesRepo::track_move(db, id, id2, Uuid::new_v4()).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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

        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::untrack_edit(db, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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

        TestLocalChangesRepo::untrack_rename(db, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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

        TestLocalChangesRepo::untrack_move(db, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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

        TestLocalChangesRepo::untrack_new_file(db, id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
            None
        );
        assert_total_local_changes!(db, 0);

        // Deleting a file should unset it's other fields
        TestLocalChangesRepo::track_rename(db, id, "old", "new").unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_delete(db, id, Document).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        assert_total_local_changes!(db, 1);
    }

    #[test]
    fn new_document_deleted() {
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(db, id).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        TestLocalChangesRepo::track_delete(db, id, Document).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
            None
        );
    }

    #[test]
    fn new_folder_deleted() {
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let id = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(db, id).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        TestLocalChangesRepo::track_delete(db, id, Folder).unwrap();

        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, id).unwrap(),
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
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let id1 = Uuid::new_v4();
        TestLocalChangesRepo::track_new_file(db, id1).unwrap();
        assert_total_local_changes!(db, 1);

        let id2 = Uuid::new_v4();
        TestLocalChangesRepo::track_rename(db, id2, "old", "new").unwrap();
        assert_total_local_changes!(db, 2);

        let id3 = Uuid::new_v4();
        TestLocalChangesRepo::track_move(db, id3, id3, Uuid::new_v4()).unwrap();
        assert_total_local_changes!(db, 3);

        let id4 = Uuid::new_v4();
        TestLocalChangesRepo::track_delete(db, id4, Document).unwrap();
        assert_total_local_changes!(db, 4);

        TestLocalChangesRepo::untrack_new_file(db, id1).unwrap();
        assert_total_local_changes!(db, 3);

        TestLocalChangesRepo::untrack_rename(db, id2).unwrap();
        assert_total_local_changes!(db, 2);

        TestLocalChangesRepo::untrack_move(db, id3).unwrap();
        assert_total_local_changes!(db, 1);

        // Untrack not supported because no one can undelete files
    }

    #[test]
    fn unknown_id() {
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let the_wrong_id = Uuid::new_v4();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, the_wrong_id).unwrap(),
            None
        );
        TestLocalChangesRepo::untrack_edit(db, the_wrong_id).unwrap();
        assert_eq!(
            TestLocalChangesRepo::get_local_changes(db, the_wrong_id).unwrap(),
            None
        );
        assert_total_local_changes!(db, 0);
    }

    #[test]
    fn rename_back_to_original() {
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let id = Uuid::new_v4();

        TestLocalChangesRepo::track_rename(db, id, "old_file", "new_name").unwrap();
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_rename(db, id, "garbage", "garbage2").unwrap();
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_rename(db, id, "garbage", "old_file").unwrap();
        assert_total_local_changes!(db, 0);
    }

    #[test]
    fn move_back_to_original() {
        let cfg = &temp_config();
        let db = &DefaultBackend::connect_to_db(cfg).unwrap();

        let id = Uuid::new_v4();
        let og = Uuid::new_v4();

        TestLocalChangesRepo::track_move(db, id, og, Uuid::new_v4()).unwrap();
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_move(db, id, Uuid::new_v4(), Uuid::new_v4()).unwrap();
        assert_total_local_changes!(db, 1);

        TestLocalChangesRepo::track_move(db, id, Uuid::new_v4(), og).unwrap();
        assert_total_local_changes!(db, 0);
    }
}
