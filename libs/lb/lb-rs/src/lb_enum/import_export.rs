impl Lb {
    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.import_files(sources,dest,update_status).await
            }
            Lb::Network(proxy) => {
                proxy.import_files(sources,dest,update_status).await
            }
        }
    }
    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.export_file(id,dest,edit,update_status).await
            }
            Lb::Network(proxy) => {
                proxy.export_file(id,dest,edit,update_status).await
            }
        }
    }
    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.export_file_recursively(id,disk_path,edit,update_status).await
            }
            Lb::Network(proxy) => {
                proxy.export_file_recursively(id,disk_path,edit,update_status).await
            }
        }
    }
}

use std::path::{PathBuf,Path};
use uuid::Uuid;
use crate::{service::import_export::{ExportFileInfo, ImportStatus}, Lb, LbResult};