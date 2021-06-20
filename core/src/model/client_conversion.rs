use crate::model::state::Config;
use crate::service::file_encryption_service::get_name;
use crate::service::sync_service::WorkCalculated;
use crate::CoreError;
use lockbook_models::account::Username;
use lockbook_models::file_metadata::{FileMetadata, FileType};
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
    pub work_units: Vec<ClientWorkUnit>,
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
    meta: &FileMetadata,
) -> Result<ClientFileMetadata, CoreError> {
    let name = get_name(config, meta)?;

    Ok(ClientFileMetadata {
        id: meta.id,
        file_type: meta.file_type,
        parent: meta.parent,
        name,
        metadata_version: meta.metadata_version,
        owner: meta.owner.clone(),
        content_version: meta.content_version,
        deleted: meta.deleted,
        users_with_access: meta
            .user_access_keys
            .iter()
            .map(|(username, _access_info)| username.clone())
            .collect(),
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
            Err(_) => ClientWorkUnit::ServerUnknownName(metadata.id),
        },
    })
}

pub fn generate_client_work_calculated(
    config: &Config,
    work_calculated: &WorkCalculated,
) -> Result<ClientWorkCalculated, CoreError> {
    let mut client_work_units = vec![];

    for work_unit in work_calculated.work_units.iter() {
        client_work_units.push(generate_client_work_unit(config, &work_unit)?)
    }

    Ok(ClientWorkCalculated {
        work_units: client_work_units,
        most_recent_update_from_server: work_calculated.most_recent_update_from_server,
    })
}
