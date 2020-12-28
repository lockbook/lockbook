use std::env;
use std::path::Path;
use std::sync::RwLock;

use glib::Sender as GlibSender;
use qrcode_generator::QrCodeEcc;
use uuid::Uuid;

use lockbook_core::model::account::Account;
use lockbook_core::model::crypto::DecryptedDocument;
use lockbook_core::model::file_metadata::FileMetadata;
use lockbook_core::model::state::Config;
use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::db_state_service::State as DbState;
use lockbook_core::service::sync_service::WorkCalculated;
use lockbook_core::{
    calculate_work, create_account, create_file_at_path, delete_file, execute_work, export_account,
    get_account, get_and_get_children_recursively, get_children, get_db_state, get_file_by_id,
    get_file_by_path, get_last_synced, get_root, get_usage, import_account, list_paths, migrate_db,
    read_document, rename_file, set_last_synced, write_document,
};

use crate::error::{LbError, LbResult};
use crate::util::KILOBYTE;

macro_rules! core_to_lb_err {
    ($enum:ident, $( $( $variants:ident )* $(=> $matches:expr )* ),+) => {
        |err| match err {
            lockbook_core::Error::UiError(e) => match e {
                $( $( lockbook_core::$enum::$variants )* $( => $matches )* ),+
            },
            lockbook_core::Error::Unexpected(msg) => LbError::Program(msg),
        }
    };
}

macro_rules! match_on_core_err {
    ($err:expr, $enum:ident, $( $( $variants:ident )* $(=> $matches:expr )* ),+) => {
        match $err {
            $( $( lockbook_core::Error::UiError(lockbook_core::$enum::$variants) )* $( => $matches )* ),+
            lockbook_core::Error::Unexpected(msg) => return Err(LbError::Program(msg)),
        }
    };
}

macro_rules! uerr {
    ($base:literal $(, $args:tt )*) => {
        LbError::User(format!($base $(, $args )*))
    };
}

macro_rules! lock {
    ($lock:expr, $r_or_w:ident) => {
        $lock
            .$r_or_w()
            .map_err(|e| LbError::Program(format!("{:?}", e)))
    };
}

macro_rules! account_must {
    ($guard:expr) => {
        $guard.as_ref().ok_or(uerr!("No account found."))
    };
}

fn api_url() -> String {
    env::var("LOCKBOOK_API_URL").unwrap_or_else(|_| "http://qa.lockbook.app:8000".to_string())
}

pub enum LbSyncMsg {
    Doing(WorkUnit, String, usize, usize),
    Error(LbError),
    Done,
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

        match get_db_state(&config).map_err(core_to_lb_err!(GetStateError,
            Stub => panic!("impossible"),
        ))? {
            DbState::ReadyToUse | DbState::Empty => {}
            DbState::StateRequiresClearing => return Err(uerr!("{}", STATE_REQ_CLEAN_MSG)),
            DbState::MigrationRequired => {
                println!("Local state requires migration! Performing migration now...");
                migrate_db(&config).map_err(core_to_lb_err!(MigrationError,
                    StateRequiresCleaning => uerr!("{}", STATE_REQ_CLEAN_MSG),
                ))?;
            }
        }

        let account = RwLock::new(match get_account(&config) {
            Ok(acct) => Some(acct),
            Err(err) => match_on_core_err!(err, GetAccountError,
                NoAccount => None,
            ),
        });

        Ok(Self { config, account })
    }

    pub fn create_account(&self, uname: &str) -> LbResult<()> {
        let api_url = api_url();
        let new_acct = create_account(&self.config, &uname, &api_url).map_err(core_to_lb_err!(
            CreateAccountError,
            UsernameTaken => uerr!("The username '{}' is already taken.", uname),
            InvalidUsername => uerr!("Invalid username '{}' ({}).", uname, UNAME_REQS),
            AccountExistsAlready => uerr!("An account already exists."),
            CouldNotReachServer => uerr!("Unable to connect to the server."),
            ClientUpdateRequired => uerr!("Client upgrade required."),
        ))?;

        self.set_account(new_acct)
    }

    pub fn import_account(&self, privkey: &str) -> LbResult<()> {
        let new_acct = import_account(&self.config, privkey).map_err(core_to_lb_err!(
            ImportError,
            AccountStringCorrupted => uerr!("Your account's private key is corrupted."),
            AccountExistsAlready => uerr!("An account already exists."),
            AccountDoesNotExist => uerr!("The account you tried to import does not exist."),
            UsernamePKMismatch => uerr!("The account private key does not match username."),
            CouldNotReachServer => uerr!("Unable to connect to the server."),
            ClientUpdateRequired => uerr!("Client upgrade required."),
        ))?;

        self.set_account(new_acct)
    }

    pub fn export_account(&self) -> LbResult<String> {
        export_account(&self.config).map_err(core_to_lb_err!(AccountExportError,
            NoAccount => uerr!("No account found."),
        ))
    }

    pub fn create_file_at_path(&self, path: &str) -> LbResult<FileMetadata> {
        let prefixed = format!("{}/{}", self.root()?.name, path);

        create_file_at_path(&self.config, &prefixed).map_err(core_to_lb_err!(CreateFileAtPathError,
            FileAlreadyExists => uerr!("That file already exists!"),
            NoAccount => uerr!("No account found."),
            NoRoot => uerr!("No root folder found."),
            PathDoesntStartWithRoot => uerr!("The path '{}' doesn't start with root.", path),
            PathContainsEmptyFile => uerr!("The path '{}' contains an empty file.", path),
            DocumentTreatedAsFolder => uerr!("A document is being treated as folder."),
        ))
    }

    pub fn save(&self, id: Uuid, content: String) -> LbResult<()> {
        let bytes = content.as_bytes();

        write_document(&self.config, id, bytes).map_err(core_to_lb_err!(WriteToDocumentError,
            NoAccount => uerr!("No account found."),
            FileDoesNotExist => uerr!("The file with id '{}' does not exist.", id),
            FolderTreatedAsDocument => uerr!(""),
        ))
    }

    pub fn root(&self) -> LbResult<FileMetadata> {
        get_root(&self.config).map_err(core_to_lb_err!(GetRootError,
            NoRoot => uerr!("No root folder found."),
        ))
    }

    pub fn children(&self, parent: &FileMetadata) -> LbResult<Vec<FileMetadata>> {
        get_children(&self.config, parent.id).map_err(core_to_lb_err!(GetChildrenError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn get_children_recursively(&self, id: Uuid) -> LbResult<Vec<FileMetadata>> {
        get_and_get_children_recursively(&self.config, id).map_err(core_to_lb_err!(
            GetAndGetChildrenError,
            FileDoesNotExist => uerr!("File with id '{}' does not exist.", id),
            DocumentTreatedAsFolder => uerr!("A document is being treated as folder."),
        ))
    }

    pub fn file_by_id(&self, id: Uuid) -> LbResult<FileMetadata> {
        get_file_by_id(&self.config, id).map_err(core_to_lb_err!(GetFileByIdError,
            NoFileWithThatId => uerr!("No file found with ID '{}'.", id),
        ))
    }

    pub fn file_by_path(&self, path: &str) -> LbResult<FileMetadata> {
        let acct_lock = lock!(self.account, read)?;
        let acct = account_must!(acct_lock)?;
        let p = format!("{}/{}", acct.username, path);

        get_file_by_path(&self.config, &p).map_err(core_to_lb_err!(GetFileByPathError,
            NoFileAtThatPath => uerr!("No file at path '{}'.", p),
        ))
    }

    pub fn delete(&self, id: &Uuid) -> LbResult<()> {
        delete_file(&self.config, *id).map_err(core_to_lb_err!(FileDeleteError,
            CannotDeleteRoot => uerr!("Deleting the root folder is not permitted."),
            FileDoesNotExist => uerr!("File with id '{}' does not exist.", id),
        ))
    }

    pub fn read(&self, id: Uuid) -> LbResult<DecryptedDocument> {
        read_document(&self.config, id).map_err(core_to_lb_err!(ReadDocumentError,
            TreatedFolderAsDocument => uerr!("There is a folder treated as a document."),
            NoAccount => uerr!("No account found."),
            FileDoesNotExist => uerr!("File with id '{}' does not exist.", id),
        ))
    }

    pub fn list_paths(&self) -> LbResult<Vec<String>> {
        list_paths(&self.config, None).map_err(core_to_lb_err!(ListPathsError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn rename(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        rename_file(&self.config, *id, new_name).map_err(core_to_lb_err!(RenameFileError,
            CannotRenameRoot => uerr!("The root folder cannot be renamed."),
            FileDoesNotExist => uerr!("The file you are trying to rename does not exist."),
            FileNameNotAvailable => uerr!("The new file name is not available."),
            NewNameContainsSlash => uerr!("File names cannot contain slashes."),
            NewNameEmpty => uerr!("File names cannot be blank."),
        ))
    }

    pub fn sync(&self, chan: &GlibSender<LbSyncMsg>) -> LbResult<()> {
        let acct_lock = lock!(self.account, read)?;
        let acct = account_must!(acct_lock)?;

        let mut work: WorkCalculated;
        while {
            work = self.calculate_work()?;
            !work.work_units.is_empty()
        } {
            let total = work.work_units.len();

            for (i, wu) in work.work_units.iter().enumerate() {
                let path = self.full_path_for(&wu.get_metadata());
                chan.send(LbSyncMsg::Doing(wu.clone(), path, i + 1, total))
                    .unwrap();

                self.do_work(&acct, wu)?;
            }

            if let Err(err) = self.set_last_synced(work.most_recent_update_from_server) {
                chan.send(LbSyncMsg::Error(err)).unwrap();
            }
        }

        chan.send(LbSyncMsg::Done).unwrap();
        Ok(())
    }

    pub fn calculate_work(&self) -> LbResult<WorkCalculated> {
        calculate_work(&self.config).map_err(core_to_lb_err!(CalculateWorkError,
            CouldNotReachServer => uerr!("Unable to connect to the server."),
            ClientUpdateRequired => uerr!("Client upgrade required."),
            NoAccount => uerr!("No account found."),
        ))
    }

    fn do_work(&self, a: &Account, wu: &WorkUnit) -> LbResult<()> {
        execute_work(&self.config, &a, wu.clone()).map_err(core_to_lb_err!(ExecuteWorkError,
            CouldNotReachServer => uerr!("Unable to connect to the server."),
            ClientUpdateRequired => uerr!("Client upgrade required."),
            BadAccount => uerr!("wut"),
        ))
    }

    fn set_last_synced(&self, last_sync: u64) -> LbResult<()> {
        set_last_synced(&self.config, last_sync).map_err(core_to_lb_err!(SetLastSyncedError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn get_last_synced(&self) -> LbResult<i64> {
        get_last_synced(&self.config).map_err(core_to_lb_err!(GetLastSyncedError,
            Stub => panic!("impossible"),
        ))
    }

    pub fn usage(&self) -> LbResult<(u64, f64)> {
        let u = get_usage(&self.config).map_err(core_to_lb_err!(GetUsageError,
            NoAccount => uerr!("No account found."),
            CouldNotReachServer => uerr!("Unable to connect to the server."),
            ClientUpdateRequired => uerr!("Client upgrade required."),
        ))?;
        let total = u.into_iter().map(|usage| usage.byte_secs).sum();
        Ok((total, FAKE_LIMIT))
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

    pub fn account_qrcode(&self, chan: &GlibSender<LbResult<String>>) {
        match self.export_account() {
            Ok(privkey) => {
                let path = format!("{}/account-qr.png", self.config.writeable_path);
                if !Path::new(&path).exists() {
                    let bytes = privkey.as_bytes();
                    qrcode_generator::to_png_to_file(bytes, QrCodeEcc::Low, 400, &path).unwrap();
                }
                chan.send(Ok(path)).unwrap();
            }
            err => chan.send(err).unwrap(),
        }
    }

    pub fn list_paths_without_root(&self) -> LbResult<Vec<String>> {
        let paths = self.list_paths()?;
        let acct_lock = lock!(self.account, read)?;
        let acct = account_must!(acct_lock)?;
        let root = &acct.username;
        Ok(paths.iter().map(|p| p.replacen(root, "", 1)).collect())
    }

    pub fn full_path_for(&self, f: &FileMetadata) -> String {
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

    pub fn open(&self, id: &Uuid) -> LbResult<(FileMetadata, String)> {
        let meta = self.file_by_id(*id)?;
        let decrypted = self.read(meta.id)?;
        Ok((meta, String::from_utf8_lossy(&decrypted).to_string()))
    }
}

const UNAME_REQS: &str = "letters and numbers only";
const FAKE_LIMIT: f64 = KILOBYTE as f64 * 20.0;
const STATE_REQ_CLEAN_MSG: &str =
    "Your local state cannot be migrated, please re-sync with a fresh client.";
