use std::env;
use std::path::Path;
use std::sync::RwLock;

use qrcode_generator::QrCodeEcc;
use uuid::Uuid;

use lockbook_core::model::state::Config;
use lockbook_core::service::db_state_service::State as DbState;
use lockbook_core::service::sync_service::SyncProgress;
use lockbook_core::{
    calculate_work, create_account, create_file, delete_file, export_account, get_account,
    get_and_get_children_recursively, get_children, get_db_state, get_file_by_id, get_file_by_path,
    get_last_synced, get_root, get_usage, import_account, list_paths, migrate_db, read_document,
    rename_file, sync_all, write_document,
};
use lockbook_models::account::Account;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::file_metadata::{FileMetadata, FileType};

use crate::error::{LbErrTarget, LbError, LbResult};
use crate::{closure, progerr, uerr, uerr_dialog, uerr_status_panel};
use lockbook_core::model::client_conversion::{
    ClientFileMetadata, ClientWorkCalculated, ClientWorkUnit,
};
use lockbook_core::service::usage_service::{bytes_to_human, UsageMetrics};

macro_rules! match_core_err {
    (
        $err:expr,
        $enum:ident,
        $( $variants:ident => $matches:expr ),+,
        @Unexpected($msg:ident) => $unexp:expr,
    ) => {
        match $err {
            $( lockbook_core::Error::UiError(lockbook_core::$enum::$variants) => $matches, )+
            lockbook_core::Error::Unexpected($msg) => $unexp,
        }
    };
}

macro_rules! map_core_err {
    ($enum:ident, $( $variants:ident => $matches:expr ,)+) => {
        |err| match_core_err!(err, $enum,
            $( $variants  => $matches ),+,
            @Unexpected(msg) => progerr!("{}", msg),
        )
    };
}

macro_rules! lock {
    ($lock:expr, $r_or_w:ident) => {
        $lock.$r_or_w().map_err(|e| progerr!("{:?}", e))
    };
}

macro_rules! account {
    ($guard:expr) => {
        $guard.as_ref().ok_or(uerr_dialog!("No account found."))
    };
}

fn api_url() -> String {
    env::var("API_URL").unwrap_or_else(|_| lockbook_core::DEFAULT_API_LOCATION.to_string())
}

pub struct LbSyncMsg {
    pub work: ClientWorkUnit,
    pub name: String,
    pub index: usize,
    pub total: usize,
}

pub struct LbCore {
    config: Config,
    account: RwLock<Option<Account>>,
}

impl LbCore {
    pub fn new(cfg_path: &str) -> LbResult<Self> {
        let config = Config {
            writeable_path: cfg_path.to_string(),
        };

        match get_db_state(&config).map_err(map_core_err!(GetStateError,
            Stub => panic!("impossible"),
        ))? {
            DbState::ReadyToUse | DbState::Empty => {}
            DbState::StateRequiresClearing => return Err(uerr_dialog!("{}", STATE_REQ_CLEAN_MSG)),
            DbState::MigrationRequired => {
                println!("Local state requires migration! Performing migration now...");
                migrate_db(&config).map_err(map_core_err!(MigrationError,
                    StateRequiresCleaning => uerr_dialog!("{}", STATE_REQ_CLEAN_MSG),
                ))?;
            }
        }

        let account = RwLock::new(match get_account(&config) {
            Ok(acct) => Some(acct),
            Err(err) => match_core_err!(err, GetAccountError,
                NoAccount => None,
                @Unexpected(msg) => return Err(progerr!("{}", msg)),
            ),
        });

        Ok(Self { config, account })
    }

    pub fn create_account(&self, uname: &str) -> LbResult<()> {
        let api_url = api_url();
        let new_acct = create_account(&self.config, &uname, &api_url).map_err(map_core_err!(
            CreateAccountError,
            UsernameTaken => uerr_dialog!("The username '{}' is already taken.", uname),
            InvalidUsername => uerr_dialog!("Invalid username '{}' ({}).", uname, UNAME_REQS),
            AccountExistsAlready => uerr_dialog!("An account already exists."),
            CouldNotReachServer => uerr_dialog!("Unable to connect to the server."),
            ClientUpdateRequired => uerr_dialog!("Client upgrade required."),
        ))?;
        self.set_account(new_acct)
    }

    pub fn import_account(&self, privkey: &str) -> LbResult<()> {
        let new_acct = import_account(&self.config, privkey).map_err(map_core_err!(
            ImportError,
            AccountStringCorrupted => uerr_dialog!("Your account's private key is corrupted."),
            AccountExistsAlready => uerr_dialog!("An account already exists."),
            AccountDoesNotExist => uerr_dialog!("The account you tried to import does not exist."),
            UsernamePKMismatch => uerr_dialog!("The account private key does not match username."),
            CouldNotReachServer => uerr_dialog!("Unable to connect to the server."),
            ClientUpdateRequired => uerr_dialog!("Client upgrade required."),
        ))?;
        self.set_account(new_acct)
    }

    pub fn export_account(&self) -> LbResult<String> {
        export_account(&self.config).map_err(map_core_err!(AccountExportError,
            NoAccount => uerr_dialog!("No account found."),
        ))
    }

    pub fn create_file(
        &self,
        name: &str,
        parent: Uuid,
        file_type: FileType,
    ) -> LbResult<ClientFileMetadata> {
        create_file(&self.config, name, parent, file_type).map_err(map_core_err!(CreateFileError,
            FileNameNotAvailable => uerr_dialog!("That file name is not available."),
            NoAccount => uerr_dialog!("No account found."),
            DocumentTreatedAsFolder => uerr_dialog!("A document is being treated as folder."),
            CouldNotFindAParent => uerr_dialog!("Could not find parent."),
            FileNameEmpty => uerr_dialog!("Cannot create file with no name."),
            FileNameContainsSlash => uerr_dialog!("The file name cannot contain a slash."),
        ))
    }

    pub fn save(&self, id: Uuid, content: String) -> LbResult<()> {
        let bytes = content.as_bytes();

        write_document(&self.config, id, bytes).map_err(map_core_err!(WriteToDocumentError,
            NoAccount => uerr_dialog!("No account found."),
            FileDoesNotExist => uerr_dialog!("The file with id '{}' does not exist.", id),
            FolderTreatedAsDocument => uerr_dialog!(""),
        ))
    }

    pub fn root(&self) -> LbResult<ClientFileMetadata> {
        get_root(&self.config).map_err(map_core_err!(GetRootError,
            NoRoot => uerr_dialog!("No root folder found."),
        ))
    }

    pub fn children(&self, parent: &ClientFileMetadata) -> LbResult<Vec<ClientFileMetadata>> {
        get_children(&self.config, parent.id).map_err(map_core_err!(GetChildrenError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn get_children_recursively(&self, id: Uuid) -> LbResult<Vec<FileMetadata>> {
        get_and_get_children_recursively(&self.config, id).map_err(map_core_err!(
            GetAndGetChildrenError,
            FileDoesNotExist => uerr_dialog!("File with id '{}' does not exist.", id),
            DocumentTreatedAsFolder => uerr_dialog!("A document is being treated as folder."),
        ))
    }

    pub fn file_by_id(&self, id: Uuid) -> LbResult<ClientFileMetadata> {
        get_file_by_id(&self.config, id).map_err(map_core_err!(GetFileByIdError,
            NoFileWithThatId => uerr_dialog!("No file found with ID '{}'.", id),
        ))
    }

    pub fn file_by_path(&self, path: &str) -> LbResult<ClientFileMetadata> {
        let acct_lock = lock!(self.account, read)?;
        let acct = account!(acct_lock)?;
        let p = format!("{}/{}", acct.username, path);

        get_file_by_path(&self.config, &p).map_err(map_core_err!(GetFileByPathError,
            NoFileAtThatPath => uerr_dialog!("No file at path '{}'.", p),
        ))
    }

    pub fn delete(&self, id: &Uuid) -> LbResult<()> {
        delete_file(&self.config, *id).map_err(map_core_err!(FileDeleteError,
            CannotDeleteRoot => uerr_dialog!("Deleting the root folder is not permitted."),
            FileDoesNotExist => uerr_dialog!("File with id '{}' does not exist.", id),
        ))
    }

    pub fn read(&self, id: Uuid) -> LbResult<DecryptedDocument> {
        read_document(&self.config, id).map_err(map_core_err!(ReadDocumentError,
            TreatedFolderAsDocument => uerr_dialog!("There is a folder treated as a document."),
            NoAccount => uerr_dialog!("No account found."),
            FileDoesNotExist => uerr_dialog!("File with id '{}' does not exist.", id),
        ))
    }

    pub fn list_paths(&self) -> LbResult<Vec<String>> {
        list_paths(&self.config, None).map_err(map_core_err!(ListPathsError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn rename(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        rename_file(&self.config, *id, new_name).map_err(map_core_err!(RenameFileError,
            CannotRenameRoot => uerr_dialog!("The root folder cannot be renamed."),
            FileDoesNotExist => uerr_dialog!("The file you are trying to rename does not exist."),
            FileNameNotAvailable => uerr_dialog!("The new file name is not available."),
            NewNameContainsSlash => uerr_dialog!("File names cannot contain slashes."),
            NewNameEmpty => uerr_dialog!("File names cannot be blank."),
        ))
    }

    pub fn sync(&self, ch: glib::Sender<Option<LbSyncMsg>>) -> LbResult<()> {
        let closure = closure!(ch => move |sync_progress: SyncProgress| {
            let wu = sync_progress.current_work_unit;

            let name = match &wu {
                ClientWorkUnit::ServerUnknownName(_) => "New file".to_string(),
                ClientWorkUnit::Server(metadata) => metadata.name.clone(),
                ClientWorkUnit::Local(metadata) => metadata.name.clone(),
            };

            let data = LbSyncMsg {
                work: wu,
                name: name,
                index: sync_progress.progress + 1,
                total: sync_progress.total,
            };

            ch.send(Some(data)).unwrap();
        });

        let sync =
            sync_all(&self.config, Some(Box::new(closure))).map_err(map_core_err!(SyncAllError,
                CouldNotReachServer => uerr_status_panel!("Offline."),
                ClientUpdateRequired => uerr_dialog!("Client upgrade required."),
                NoAccount => uerr_dialog!("No account found."),
            ));

        ch.send(None).unwrap();

        sync?;

        Ok(())
    }

    pub fn calculate_work(&self) -> LbResult<ClientWorkCalculated> {
        calculate_work(&self.config).map_err(map_core_err!(CalculateWorkError,
            CouldNotReachServer => uerr_status_panel!("Offline."),
            ClientUpdateRequired => uerr_dialog!("Client upgrade required."),
            NoAccount => uerr_dialog!("No account found."),
        ))
    }

    pub fn get_last_synced(&self) -> LbResult<i64> {
        get_last_synced(&self.config).map_err(map_core_err!(GetLastSyncedError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn get_usage(&self) -> LbResult<UsageMetrics> {
        get_usage(&self.config).map_err(map_core_err!(GetUsageError,
            NoAccount => uerr_dialog!("No account found."),
            CouldNotReachServer => uerr_status_panel!("Offline."),
            ClientUpdateRequired => uerr_dialog!("Client upgrade required."),
        ))
    }

    pub fn has_account(&self) -> LbResult<bool> {
        let acct = lock!(self.account, read)?;
        Ok(acct.is_some())
    }

    fn set_account(&self, a: Account) -> LbResult<()> {
        let mut acct = lock!(self.account, write)?;
        *acct = Some(a);
        Ok(())
    }

    pub fn account_qrcode(&self) -> LbResult<String> {
        let privkey = self.export_account()?;
        let path = format!("{}/account-qr.png", self.config.writeable_path);
        if !Path::new(&path).exists() {
            let bytes = privkey.as_bytes();
            qrcode_generator::to_png_to_file(bytes, QrCodeEcc::Low, 400, &path).unwrap();
        }
        Ok(path)
    }

    pub fn list_paths_without_root(&self) -> LbResult<Vec<String>> {
        let paths = self.list_paths()?;
        let acct_lock = lock!(self.account, read)?;
        let acct = account!(acct_lock)?;
        let root = &acct.username;
        Ok(paths.iter().map(|p| p.replacen(root, "", 1)).collect())
    }

    pub fn full_path_for(&self, f: &ClientFileMetadata) -> String {
        let root_id = match self.root() {
            Ok(root) => {
                if f.id == root.id {
                    return "/".to_string();
                }
                root.id
            }
            Err(_) => Default::default(),
        };

        let mut path = "".to_string();
        let mut ff = f.clone();
        while ff.id != root_id {
            path.insert_str(0, &format!("/{}", ff.name));
            ff = match self.file_by_id(ff.parent) {
                Ok(f) => f,
                Err(_) => break,
            }
        }

        path
    }

    pub fn open(&self, id: &Uuid) -> LbResult<(ClientFileMetadata, String)> {
        let meta = self.file_by_id(*id)?;
        let decrypted = self.read(meta.id)?;
        Ok((meta, String::from_utf8_lossy(&decrypted).to_string()))
    }

    pub fn sync_status(&self) -> LbResult<String> {
        match self.get_last_synced()? {
            0 => Ok("✘  Never synced.".to_string()),
            _ => {
                let work = self.calculate_work()?;
                let n_files = work.local_files.len()
                    + work.server_files.len()
                    + work.server_unknown_name_count;
                Ok(match n_files {
                    0 => "✔  Synced.".to_string(),
                    1 => "<b>1</b>  file not synced.".to_string(),
                    _ => format!("<b>{}</b>  files not synced.", n_files),
                })
            }
        }
    }

    pub fn usage_status(&self) -> LbResult<(Option<String>, Option<String>)> {
        let usage = self.get_usage()?;

        if usage.server_usage.exact >= usage.data_cap.exact {
            return Ok((
                Some("You're out of space!".to_string()),
                Some("You have run out of space, go to the settings to buy more!".to_string()),
            ));
        } else if usage.server_usage.exact as f32 / usage.data_cap.exact as f32
            > USAGE_WARNING_THRESHOLD
        {
            return Ok((
                Some(format!(
                    "{} of {} remaining!",
                    bytes_to_human(usage.data_cap.exact - usage.server_usage.exact),
                    usage.data_cap.readable
                )),
                Some("You are running out of space, go to the settings to buy more!".to_string()),
            ));
        }

        Ok((None, None))
    }
}

const UNAME_REQS: &str = "letters and numbers only";
const STATE_REQ_CLEAN_MSG: &str =
    "Your local state cannot be migrated, please re-sync with a fresh client.";
const USAGE_WARNING_THRESHOLD: f32 = 0.9;
