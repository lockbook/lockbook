impl Lb {
    pub async fn create_link_at_path(&self, path: &str, target_id: Uuid) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.create_link_at_path(path,target_id).await
            }
            Lb::Network(proxy) => {
                proxy.create_link_at_path(path,target_id).await
            }
        }
    }
    pub async fn create_at_path(&self, path: &str) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.create_at_path(path).await
            }
            Lb::Network(proxy) => {
                proxy.create_at_path(path).await
            }
        }
    }
    pub async fn get_by_path(&self, path: &str) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.get_by_path(path).await
            }
            Lb::Network(proxy) => {
                proxy.get_by_path(path).await
            }
        }
    }
    pub async fn get_path_by_id(&self, id: Uuid) -> LbResult<String>{
        match self {
            Lb::Direct(inner) => {
                inner.get_path_by_id(id).await
            }
            Lb::Network(proxy) => {
                proxy.get_path_by_id(id).await
            }
        }
    }
    pub async fn list_paths(&self, filter: Option<Filter>) -> LbResult<Vec<String>>{
        match self {
            Lb::Direct(inner) => {
                inner.list_paths(filter).await
            }
            Lb::Network(proxy) => {
                proxy.list_paths(filter).await
            }
        }
    }
    pub async fn list_paths_with_ids(
        &self, filter: Option<Filter>,
    ) -> LbResult<Vec<(Uuid, String)>>{
        match self {
            Lb::Direct(inner) => {
                inner.list_paths_with_ids(filter).await
            }
            Lb::Network(proxy) => {
                proxy.list_paths_with_ids(filter).await
            }
        }
    }
}

use uuid::Uuid;
use crate::{model::{file::File, path_ops::Filter}, Lb, LbResult};