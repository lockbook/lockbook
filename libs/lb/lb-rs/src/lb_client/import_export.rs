impl LbClient {
    pub async fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> LbResult<()>{
        
    }
    pub async fn export_file<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, dest: PathBuf, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
        
    }
    pub async fn export_file_recursively<F: Fn(ExportFileInfo)>(
        &self, id: Uuid, disk_path: &Path, edit: bool, update_status: &Option<F>,
    ) -> LbResult<()>{
       
    }
}

use crate::lb_client::LbClient;
use std::path::{PathBuf,Path};
use uuid::Uuid;
use crate::{service::import_export::{ExportFileInfo, ImportStatus}, LbResult};