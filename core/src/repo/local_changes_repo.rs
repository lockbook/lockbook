use std::time::SystemTimeError;

use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::local_storage;
use lockbook_crypto::clock_service::TimeGetter;
use lockbook_models::crypto::{EncryptedDocument, UserAccessInfo};
use lockbook_models::file_metadata::FileType;
use lockbook_models::local_changes::{Edited, LocalChange, Moved, Renamed};
use std::{thread, time};

#[derive(Debug)]
pub enum DbError {
    TimeError(SystemTimeError),
    BackendError(std::io::Error),
    SerdeError(serde_json::Error),
}

pub static LOCAL_CHANGES: &[u8; 13] = b"local_changes";

pub fn get_all_local_changes(config: &Config) -> Result<Vec<LocalChange>, DbError> {
    let mut value = local_storage::dump::<_, Vec<u8>>(config, LOCAL_CHANGES)
        .map_err(DbError::BackendError)?
        .into_iter()
        .map(|s| serde_json::from_slice(s.as_ref()).map_err(DbError::SerdeError))
        .collect::<Result<Vec<LocalChange>, DbError>>()?;

    value.sort_by(|change1, change2| change1.timestamp.cmp(&change2.timestamp));

    Ok(value)
}

pub fn get_local_changes(config: &Config, id: Uuid) -> Result<Option<LocalChange>, DbError> {
    let maybe_value: Option<Vec<u8>> =
        local_storage::read(config, LOCAL_CHANGES, id.to_string().as_str())
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

pub fn track_new_file(config: &Config, id: Uuid, now: TimeGetter) -> Result<(), DbError> {
    let new_local_change = LocalChange {
        timestamp: now().0,
        id,
        renamed: None,
        moved: None,
        new: true,
        content_edited: None,
        deleted: false,
    };

    local_storage::write(
        config,
        LOCAL_CHANGES,
        id.to_string().as_str(),
        serde_json::to_vec(&new_local_change).map_err(DbError::SerdeError)?,
    )
    .map_err(DbError::BackendError)?;
    Ok(())
}

pub fn track_rename(
    config: &Config,
    id: Uuid,
    old_name: &str,
    new_name: &str,
    now: TimeGetter,
) -> Result<(), DbError> {
    if old_name == new_name {
        return Ok(());
    }

    match get_local_changes(config, id)? {
        None => {
            let new_local_change = LocalChange {
                timestamp: now().0,
                id,
                renamed: Some(Renamed::from(old_name)),
                moved: None,
                new: false,
                content_edited: None,
                deleted: false,
            };

            local_storage::write(
                config,
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
                local_storage::write(
                    config,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
                    serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(renamed) => {
                if new_name == renamed.old_value {
                    untrack_rename(config, id)
                } else {
                    Ok(())
                }
            }
        },
    }
}

pub fn track_move(
    config: &Config,
    id: Uuid,
    old_parent: Uuid,
    new_parent: Uuid,
    now: TimeGetter,
) -> Result<(), DbError> {
    if old_parent == new_parent {
        return Ok(());
    }

    match get_local_changes(config, id)? {
        None => {
            let new_local_change = LocalChange {
                timestamp: now().0,
                id,
                renamed: None,
                moved: Some(Moved::from(old_parent)),
                new: false,
                content_edited: None,
                deleted: false,
            };

            local_storage::write(
                config,
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
                local_storage::write(
                    config,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
                    serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(moved) => {
                if moved.old_value == new_parent {
                    untrack_move(config, id)
                } else {
                    Ok(())
                }
            }
        },
    }
}

pub fn track_edit(
    config: &Config,
    id: Uuid,
    old_version: &EncryptedDocument,
    access_info_for_old_version: &UserAccessInfo,
    old_content_checksum: Vec<u8>,
    new_content_checksum: Vec<u8>,
    now: TimeGetter,
) -> Result<(), DbError> {
    if old_content_checksum == new_content_checksum {
        return Ok(());
    }

    match get_local_changes(config, id)? {
        None => {
            let new_local_change = LocalChange {
                timestamp: now().0,
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
            local_storage::write(
                config,
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
                local_storage::write(
                    config,
                    LOCAL_CHANGES,
                    id.to_string().as_str(),
                    serde_json::to_vec(&change).map_err(DbError::SerdeError)?,
                )
                .map_err(DbError::BackendError)?;
                Ok(())
            }
            Some(edited) => {
                if edited.old_content_checksum == new_content_checksum {
                    untrack_edit(config, id)
                } else {
                    Ok(())
                }
            }
        },
    }
}

pub fn track_delete(
    config: &Config,
    id: Uuid,
    file_type: FileType,
    now: TimeGetter,
) -> Result<(), DbError> {
    // Added to ensure that a prior move is at least 1ms older than this delete
    thread::sleep(time::Duration::from_millis(1));

    match get_local_changes(config, id)? {
        None => {
            let new_local_change = LocalChange {
                timestamp: now().0,
                id,
                renamed: None,
                moved: None,
                new: false,
                content_edited: None,
                deleted: true,
            };
            local_storage::write(
                config,
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
                    delete(config, id)
                } else {
                    // If a document was deleted, don't bother pushing it's rename / move
                    let delete_tracked = LocalChange {
                        timestamp: now().0,
                        id,
                        renamed: None,
                        moved: None,
                        new: false,
                        content_edited: None,
                        deleted: true,
                    };
                    local_storage::write(
                        config,
                        LOCAL_CHANGES,
                        id.to_string().as_str(),
                        serde_json::to_vec(&delete_tracked).map_err(DbError::SerdeError)?,
                    )
                    .map_err(DbError::BackendError)?;
                    Ok(())
                }
            } else {
                change.deleted = true;
                local_storage::write(
                    config,
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

pub fn untrack_new_file(config: &Config, id: Uuid) -> Result<(), DbError> {
    match get_local_changes(config, id)? {
        None => Ok(()),
        Some(mut new) => {
            new.new = false;

            if !new.deleted {
                delete(config, new.id)?
            } else {
                local_storage::write(
                    config,
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

pub fn untrack_rename(config: &Config, id: Uuid) -> Result<(), DbError> {
    match get_local_changes(config, id)? {
        None => Ok(()),
        Some(mut edit) => {
            edit.renamed = None;

            if edit.ready_to_be_deleted() {
                delete(config, edit.id)?
            } else {
                local_storage::write(
                    config,
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

pub fn untrack_move(config: &Config, id: Uuid) -> Result<(), DbError> {
    match get_local_changes(config, id)? {
        None => Ok(()),
        Some(mut edit) => {
            edit.moved = None;

            if edit.ready_to_be_deleted() {
                delete(config, edit.id)?
            } else {
                local_storage::write(
                    config,
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

pub fn untrack_edit(config: &Config, id: Uuid) -> Result<(), DbError> {
    match get_local_changes(config, id)? {
        None => Ok(()),
        Some(mut edit) => {
            edit.content_edited = None;

            if edit.ready_to_be_deleted() {
                delete(config, edit.id)?
            } else {
                local_storage::write(
                    config,
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

pub fn delete(config: &Config, id: Uuid) -> Result<(), DbError> {
    match get_local_changes(config, id)? {
        None => Ok(()),
        Some(_) => {
            local_storage::delete(config, LOCAL_CHANGES, id.to_string().as_str())
                .map_err(DbError::BackendError)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::local_changes_repo;
    use crate::model::state::temp_config;
    use lockbook_crypto::clock_service::Timestamp;
    use lockbook_models::file_metadata::FileType::{Document, Folder};
    use lockbook_models::local_changes::{LocalChange, Moved, Renamed};

    static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(0);

    macro_rules! assert_total_local_changes (
        ($db:expr, $total:literal) => {
            assert_eq!(
                local_changes_repo::get_all_local_changes($db)
                    .unwrap()
                    .len(),
                $total
            );
        }
    );

    #[test]
    fn set_and_unset_fields() {
        let cfg = &temp_config();

        assert_total_local_changes!(cfg, 0);

        let id = Uuid::new_v4();
        local_changes_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
        local_changes_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
        local_changes_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_rename(cfg, id, "old_file", "unused_name", EARLY_CLOCK).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        assert_total_local_changes!(cfg, 1);

        let id2 = Uuid::new_v4();
        local_changes_repo::track_move(cfg, id, id2, Uuid::new_v4(), EARLY_CLOCK).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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

        assert_total_local_changes!(cfg, 1);

        local_changes_repo::untrack_edit(cfg, id).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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

        local_changes_repo::untrack_rename(cfg, id).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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

        local_changes_repo::untrack_move(cfg, id).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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

        local_changes_repo::untrack_new_file(cfg, id).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
            None
        );
        assert_total_local_changes!(cfg, 0);

        // Deleting a file should unset it's other fields
        local_changes_repo::track_rename(cfg, id, "old", "new", EARLY_CLOCK).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_delete(cfg, id, Document, EARLY_CLOCK).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        assert_total_local_changes!(cfg, 1);
    }

    #[test]
    fn new_document_deleted() {
        let cfg = &temp_config();

        let id = Uuid::new_v4();
        local_changes_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();

        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        local_changes_repo::track_delete(cfg, id, Document, EARLY_CLOCK).unwrap();

        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
            None
        );
    }

    #[test]
    fn new_folder_deleted() {
        let cfg = &temp_config();

        let id = Uuid::new_v4();
        local_changes_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();

        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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
        local_changes_repo::track_delete(cfg, id, Folder, EARLY_CLOCK).unwrap();

        assert_eq!(
            local_changes_repo::get_local_changes(cfg, id).unwrap(),
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

        let id1 = Uuid::new_v4();
        local_changes_repo::track_new_file(cfg, id1, EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 1);

        let id2 = Uuid::new_v4();
        local_changes_repo::track_rename(cfg, id2, "old", "new", EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 2);

        let id3 = Uuid::new_v4();
        local_changes_repo::track_move(cfg, id3, id3, Uuid::new_v4(), EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 3);

        let id4 = Uuid::new_v4();
        local_changes_repo::track_delete(cfg, id4, Document, EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 4);

        local_changes_repo::untrack_new_file(cfg, id1).unwrap();
        assert_total_local_changes!(cfg, 3);

        local_changes_repo::untrack_rename(cfg, id2).unwrap();
        assert_total_local_changes!(cfg, 2);

        local_changes_repo::untrack_move(cfg, id3).unwrap();
        assert_total_local_changes!(cfg, 1);

        // Untrack not supported because no one can undelete files
    }

    #[test]
    fn unknown_id() {
        let cfg = &temp_config();

        let the_wrong_id = Uuid::new_v4();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, the_wrong_id).unwrap(),
            None
        );
        local_changes_repo::untrack_edit(cfg, the_wrong_id).unwrap();
        assert_eq!(
            local_changes_repo::get_local_changes(cfg, the_wrong_id).unwrap(),
            None
        );
        assert_total_local_changes!(cfg, 0);
    }

    #[test]
    fn rename_back_to_original() {
        let cfg = &temp_config();

        let id = Uuid::new_v4();

        local_changes_repo::track_rename(cfg, id, "old_file", "new_name", EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_rename(cfg, id, "garbage", "garbage2", EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_rename(cfg, id, "garbage", "old_file", EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 0);
    }

    #[test]
    fn move_back_to_original() {
        let cfg = &temp_config();

        let id = Uuid::new_v4();
        let og = Uuid::new_v4();

        local_changes_repo::track_move(cfg, id, og, Uuid::new_v4(), EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_move(cfg, id, Uuid::new_v4(), Uuid::new_v4(), EARLY_CLOCK)
            .unwrap();
        assert_total_local_changes!(cfg, 1);

        local_changes_repo::track_move(cfg, id, Uuid::new_v4(), og, EARLY_CLOCK).unwrap();
        assert_total_local_changes!(cfg, 0);
    }
}
