use crate::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;
use lockbook_models::file_metadata::FileMetadata;
use uuid::Uuid;

const NAMESPACE_LOCAL: &str = "changed_local_metadata";
const NAMESPACE_BASE: &str = "all_base_metadata";

fn namespace(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Local => NAMESPACE_LOCAL,
        RepoSource::Base => NAMESPACE_BASE,
    }
}

pub fn insert(config: &Config, source: RepoSource, file: &FileMetadata) -> Result<(), CoreError> {
    local_storage::write(
        config,
        namespace(source),
        file.id.to_string().as_str(),
        serde_json::to_vec(&file).map_err(core_err_unexpected)?,
    )
}

pub fn get(config: &Config, source: RepoSource, id: Uuid) -> Result<FileMetadata, CoreError> {
    maybe_get(config, source, id).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get(
    config: &Config,
    source: RepoSource,
    id: Uuid,
) -> Result<Option<FileMetadata>, CoreError> {
    let maybe_bytes: Option<Vec<u8>> =
        local_storage::read(config, namespace(source), id.to_string().as_str())?;
    Ok(match maybe_bytes {
        Some(bytes) => Some(serde_json::from_slice(&bytes).map_err(core_err_unexpected)?),
        None => None,
    })
}

pub fn get_all(config: &Config, source: RepoSource) -> Result<Vec<FileMetadata>, CoreError> {
    Ok(
        local_storage::dump::<_, Vec<u8>>(config, namespace(source))?
            .into_iter()
            .map(|s| serde_json::from_slice(s.as_ref()).map_err(core_err_unexpected))
            .collect::<Result<Vec<FileMetadata>, CoreError>>()?
            .into_iter()
            .collect(),
    )
}

pub fn delete(config: &Config, source: RepoSource, id: Uuid) -> Result<(), CoreError> {
    local_storage::delete(config, namespace(source), id.to_string().as_str())
}

pub fn delete_all(config: &Config, source: RepoSource) -> Result<(), CoreError> {
    local_storage::delete_all(config, namespace(source))
}

// todo: replace
// #[cfg(test)]
// mod unit_tests {
//     use uuid::Uuid;

//     use crate::local_metadata_repo;
//     use crate::model::state::temp_config;
//     use lockbook_crypto::clock_service::Timestamp;
//     use lockbook_models::file_metadata::FileType::{Document, Folder};
//     use lockbook_models::local_changes::{LocalChange, Moved, Renamed};

//     static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(0);

//     macro_rules! assert_total_local_changes (
//         ($db:expr, $total:literal) => {
//             assert_eq!(
//                 local_changes_repo::get_all_local_changes($db)
//                     .unwrap()
//                     .len(),
//                 $total
//             );
//         }
//     );

//     #[test]
//     fn set_and_unset_fields() {
//         let cfg = &temp_config();

//         assert_total_local_changes!(cfg, 0);

//         let id = Uuid::new_v4();
//         local_metadata_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
//         local_metadata_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
//         local_metadata_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_rename(cfg, id, "old_file", "unused_name", EARLY_CLOCK).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: Some(Renamed::from("old_file")),
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );
//         assert_total_local_changes!(cfg, 1);

//         let id2 = Uuid::new_v4();
//         local_metadata_repo::track_move(cfg, id, id2, Uuid::new_v4(), EARLY_CLOCK).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: Some(Renamed::from("old_file")),
//                 moved: Some(Moved::from(id2)),
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );

//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::untrack_edit(cfg, id).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: Some(Renamed::from("old_file")),
//                 moved: Some(Moved::from(id2)),
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );

//         local_metadata_repo::untrack_rename(cfg, id).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: Some(Moved::from(id2)),
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );

//         local_metadata_repo::untrack_move(cfg, id).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );

//         local_metadata_repo::untrack_new_file(cfg, id).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             None
//         );
//         assert_total_local_changes!(cfg, 0);

//         // Deleting a file should unset it's other fields
//         local_metadata_repo::track_rename(cfg, id, "old", "new", EARLY_CLOCK).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: Some(Renamed::from("old")),
//                 moved: None,
//                 new: false,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_delete(cfg, id, Document, EARLY_CLOCK).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: false,
//                 content_edited: None,
//                 deleted: true,
//             })
//         );
//         assert_total_local_changes!(cfg, 1);
//     }

//     #[test]
//     fn new_document_deleted() {
//         let cfg = &temp_config();

//         let id = Uuid::new_v4();
//         local_metadata_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();

//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );
//         local_metadata_repo::track_delete(cfg, id, Document, EARLY_CLOCK).unwrap();

//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             None
//         );
//     }

//     #[test]
//     fn new_folder_deleted() {
//         let cfg = &temp_config();

//         let id = Uuid::new_v4();
//         local_metadata_repo::track_new_file(cfg, id, EARLY_CLOCK).unwrap();

//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: false,
//             })
//         );
//         local_metadata_repo::track_delete(cfg, id, Folder, EARLY_CLOCK).unwrap();

//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, id).unwrap(),
//             Some(LocalChange {
//                 timestamp: 0,
//                 id,
//                 renamed: None,
//                 moved: None,
//                 new: true,
//                 content_edited: None,
//                 deleted: true,
//             })
//         );
//     }

//     #[test]
//     fn track_changes_on_multiple_files() {
//         let cfg = &temp_config();

//         let id1 = Uuid::new_v4();
//         local_metadata_repo::track_new_file(cfg, id1, EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 1);

//         let id2 = Uuid::new_v4();
//         local_metadata_repo::track_rename(cfg, id2, "old", "new", EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 2);

//         let id3 = Uuid::new_v4();
//         local_metadata_repo::track_move(cfg, id3, id3, Uuid::new_v4(), EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 3);

//         let id4 = Uuid::new_v4();
//         local_metadata_repo::track_delete(cfg, id4, Document, EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 4);

//         local_metadata_repo::untrack_new_file(cfg, id1).unwrap();
//         assert_total_local_changes!(cfg, 3);

//         local_metadata_repo::untrack_rename(cfg, id2).unwrap();
//         assert_total_local_changes!(cfg, 2);

//         local_metadata_repo::untrack_move(cfg, id3).unwrap();
//         assert_total_local_changes!(cfg, 1);

//         // Untrack not supported because no one can undelete files
//     }

//     #[test]
//     fn unknown_id() {
//         let cfg = &temp_config();

//         let the_wrong_id = Uuid::new_v4();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, the_wrong_id).unwrap(),
//             None
//         );
//         local_metadata_repo::untrack_edit(cfg, the_wrong_id).unwrap();
//         assert_eq!(
//             local_metadata_repo::get_local_changes(cfg, the_wrong_id).unwrap(),
//             None
//         );
//         assert_total_local_changes!(cfg, 0);
//     }

//     #[test]
//     fn rename_back_to_original() {
//         let cfg = &temp_config();

//         let id = Uuid::new_v4();

//         local_metadata_repo::track_rename(cfg, id, "old_file", "new_name", EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_rename(cfg, id, "garbage", "garbage2", EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_rename(cfg, id, "garbage", "old_file", EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 0);
//     }

//     #[test]
//     fn move_back_to_original() {
//         let cfg = &temp_config();

//         let id = Uuid::new_v4();
//         let og = Uuid::new_v4();

//         local_metadata_repo::track_move(cfg, id, og, Uuid::new_v4(), EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_move(cfg, id, Uuid::new_v4(), Uuid::new_v4(), EARLY_CLOCK)
//             .unwrap();
//         assert_total_local_changes!(cfg, 1);

//         local_metadata_repo::track_move(cfg, id, Uuid::new_v4(), og, EARLY_CLOCK).unwrap();
//         assert_total_local_changes!(cfg, 0);
//     }
// }
