impl Lb {
    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.create_file(name,parent,file_type).await
            }
            Lb::Network(proxy) => {
                proxy.create_file(name,parent,file_type).await
            }
        }
    }
    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.rename_file(id,new_name).await
            }
            Lb::Network(proxy) => {
                proxy.rename_file(id,new_name).await
            }
        }
    }
    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.move_file(id,new_parent).await
            }
            Lb::Network(proxy) => {
                proxy.move_file(id,new_parent).await
            }
        }
    }
    pub async fn delete(&self, id: &Uuid) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.delete(id).await
            }
            Lb::Network(proxy) => {
                proxy.delete(id).await
            }
        }
    }
    pub async fn root(&self) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.root().await
            }
            Lb::Network(proxy) => {
                proxy.root().await
            }
        }
    }
    pub async fn list_metadatas(&self) -> LbResult<Vec<File>>{
        match self {
            Lb::Direct(inner) => {
                inner.list_metadatas().await
            }
            Lb::Network(proxy) => {
                proxy.list_metadatas().await
            }
        }
    }
    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>>{
        match self {
            Lb::Direct(inner) => {
                inner.get_children(id).await
            }
            Lb::Network(proxy) => {
                proxy.get_children(id).await
            }
        }
    }
    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>>{
        match self {
            Lb::Direct(inner) => {
                inner.get_and_get_children_recursively(id).await
            }
            Lb::Network(proxy) => {
                proxy.get_and_get_children_recursively(id).await
            }
        }
    }
    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File>{
        match self {
            Lb::Direct(inner) => {
                inner.get_file_by_id(id).await
            }
            Lb::Network(proxy) => {
                proxy.get_file_by_id(id).await
            }
        }
    }
    pub async fn local_changes(&self) -> Vec<Uuid> {
        match self {
            Lb::Direct(inner) => {
                inner.local_changes().await
            }
            Lb::Network(proxy) => {
                proxy.local_changes().await
            }
        }
    }
}

use uuid::Uuid;
use crate::{model::{file::File, file_metadata::FileType}, Lb, LbResult};