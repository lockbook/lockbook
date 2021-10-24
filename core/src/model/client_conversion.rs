use crate::model::state::Config;
use crate::repo::account_repo;
use crate::service::sync_service::WorkCalculated;
use crate::CoreError;
use lockbook_models::account::Username;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use lockbook_models::work_unit::WorkUnit;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ClientFileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: String,
    pub owner: String,
    pub metadata_version: u64,
    pub content_version: u64,
    pub deleted: bool,
    pub users_with_access: Vec<Username>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ClientWorkCalculated {
    pub local_files: Vec<ClientFileMetadata>,
    pub server_files: Vec<ClientFileMetadata>,
    pub server_unknown_name_count: usize,
    pub most_recent_update_from_server: u64,
}

#[derive(Debug, Serialize, Clone)]
pub enum ClientWorkUnit {
    ServerUnknownName(Uuid),
    Server(ClientFileMetadata),
    Local(ClientFileMetadata),
}

pub fn generate_client_file_metadata(
    config: &Config,
    meta: &DecryptedFileMetadata,
) -> Result<ClientFileMetadata, CoreError> {
    Ok(ClientFileMetadata {
        id: meta.id,
        file_type: meta.file_type,
        parent: meta.parent,
        name: meta.decrypted_name.clone(),
        metadata_version: meta.metadata_version,
        owner: meta.owner.clone(),
        content_version: meta.content_version,
        deleted: meta.deleted,
        users_with_access: vec![account_repo::get(config)?.username], // todo: fix
    })
}

pub fn generate_client_work_unit(
    config: &Config,
    work_unit: &WorkUnit,
) -> Result<ClientWorkUnit, CoreError> {
    let maybe_file_metadata = generate_client_file_metadata(config, &work_unit.get_metadata());

    Ok(match work_unit {
        WorkUnit::LocalChange { .. } => ClientWorkUnit::Local(maybe_file_metadata?),
        WorkUnit::ServerChange { metadata } => match maybe_file_metadata {
            Ok(file_metadata) => ClientWorkUnit::Server(file_metadata),
            Err(_) => ClientWorkUnit::ServerUnknownName(metadata.id), // todo: this can be triggered by unexpected errors; what's this supposed to mean, anyway?
        },
    })
}

pub fn generate_client_work_calculated(
    config: &Config,
    work_calculated: &WorkCalculated,
) -> Result<ClientWorkCalculated, CoreError> {
    let mut local_files = vec![];
    let mut server_files = vec![];
    let mut new_files_count = 0;

    for work_unit in work_calculated.work_units.iter() {
        let maybe_file_metadata = generate_client_file_metadata(config, &work_unit.get_metadata());

        match work_unit {
            WorkUnit::LocalChange { .. } => local_files.push(maybe_file_metadata?),
            WorkUnit::ServerChange { .. } => match maybe_file_metadata {
                Ok(file_metadata) => server_files.push(file_metadata),
                Err(_) => new_files_count += 1,
            },
        }
    }

    Ok(ClientWorkCalculated {
        local_files,
        server_files,
        server_unknown_name_count: new_files_count,
        most_recent_update_from_server: work_calculated.most_recent_update_from_server,
    })
}
