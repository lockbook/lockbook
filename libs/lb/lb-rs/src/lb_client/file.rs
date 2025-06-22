impl LbClient {
    pub async fn create_file(
        &self,
        name: &str,
        parent: &Uuid,
        file_type: FileType,
    ) -> LbResult<File> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&(name.to_string(), *parent, file_type))
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "create_file", Some(args)).await
    }

    pub async fn rename_file(
        &self,
        id: &Uuid,
        new_name: &str,
    ) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&(*id, new_name.to_string()))
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "rename_file", Some(args)).await
    }

    pub async fn move_file(
        &self,
        id: &Uuid,
        new_parent: &Uuid,
    ) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&(*id, *new_parent))
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "move_file", Some(args)).await
    }

    pub async fn delete(
        &self,
        id: &Uuid,
    ) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "delete", Some(args)).await
    }

    pub async fn root(&self) -> LbResult<File> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "root", None).await
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "list_metadatas", None).await
    }

    pub async fn get_children(
        &self,
        id: &Uuid,
    ) -> LbResult<Vec<File>> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_children", Some(args)).await
    }

    pub async fn get_and_get_children_recursively(
        &self,
        id: &Uuid,
    ) -> LbResult<Vec<File>> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_and_get_children_recursively", Some(args)).await
    }

    pub async fn get_file_by_id(
        &self,
        id: Uuid,
    ) -> LbResult<File> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        let args = bincode::serialize(&id)
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "get_file_by_id", Some(args)).await
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .unwrap();          
        call_rpc(&mut stream, "local_changes", None)
            .await
            .unwrap()         
    }
}

use crate::lb_client::LbClient;
use crate::model::file::File;
use crate::model::file_metadata::FileType;
use crate::{model::errors::core_err_unexpected, LbResult};
use tokio::net::TcpStream;
use uuid::Uuid;
use crate::rpc::call_rpc;