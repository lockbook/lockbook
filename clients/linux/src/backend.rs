use std::env;
use std::path::Path;

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
    read_document, rename_file, set_last_synced, write_document, Error as CoreError,
    GetAccountError,
};

use crate::error::{LbError, LbResult};
use crate::util::KILOBYTE;

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
}

impl LbCore {
    pub fn new(cfg_path: &str) -> Self {
        Self {
            config: Config {
                writeable_path: cfg_path.to_string(),
            },
        }
    }

    pub fn init_db(&self) -> Result<(), String> {
        match get_db_state(&self.config) {
            Ok(state) => match state {
                DbState::ReadyToUse => Ok(()),
                DbState::Empty => Ok(()),
                DbState::MigrationRequired => {
                    println!("Local state requires migration! Performing migration now...");
                    match migrate_db(&self.config) {
                        Ok(_) => {
                            println!("Migration Successful!");
                            Ok(())
                        }
                        Err(err) => Err(format!("{:?}", err)),
                    }
                }
                DbState::StateRequiresClearing => Err(
                    "Your local state cannot be migrated, please re-sync with a fresh client."
                        .to_string(),
                ),
            },
            Err(err) => Err(format!("{:?}", err)),
        }
    }

    pub fn create_account(&self, uname: &str) -> LbResult<Account> {
        create_account(&self.config, &uname, &api_url())
            .map_err(|err| errs::create_account::to_lb_error(err, uname))
    }

    pub fn import_account(&self, privkey: &str) -> Result<(), String> {
        match import_account(&self.config, privkey) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("error importing: {:?}", err)),
        }
    }

    pub fn export_account(&self) -> LbResult<String> {
        export_account(&self.config).map_err(errs::export_account::to_lb_error)
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

    pub fn account(&self) -> Result<Option<Account>, String> {
        match get_account(&self.config) {
            Ok(acct) => Ok(Some(acct)),
            Err(err) => match err {
                CoreError::UiError(GetAccountError::NoAccount) => Ok(None),
                CoreError::Unexpected(err) => {
                    println!("error getting account: {}", err);
                    Err("Unable to load account.".to_string())
                }
            },
        }
    }

    pub fn root(&self) -> Result<FileMetadata, String> {
        get_root(&self.config).map_err(|err| format!("{:?}", err))
    }

    pub fn children(&self, parent: &FileMetadata) -> Result<Vec<FileMetadata>, String> {
        get_children(&self.config, parent.id).map_err(|err| format!("{:?}", err))
    }

    pub fn create_file_at_path(&self, path: &str) -> Result<FileMetadata, String> {
        let prefixed = format!("{}/{}", self.root().unwrap().name, path);

        create_file_at_path(&self.config, &prefixed).map_err(|err| format!("{:?}", err))
    }

    pub fn file_by_id(&self, id: Uuid) -> Result<FileMetadata, String> {
        get_file_by_id(&self.config, id).map_err(|err| format!("{:?}", err))
    }

    pub fn file_by_path(&self, path: &str) -> Result<FileMetadata, String> {
        let acct = self.account().unwrap().unwrap();
        let p = format!("{}/{}", acct.username, path);

        get_file_by_path(&self.config, &p).map_err(|err| format!("{:?}", err))
    }

    pub fn list_paths(&self) -> Result<Vec<String>, String> {
        list_paths(&self.config, None).map_err(|err| format!("{:?}", err))
    }

    pub fn list_paths_without_root(&self) -> Result<Vec<String>, String> {
        match self.list_paths() {
            Ok(paths) => {
                let root = self.account().unwrap().unwrap().username;
                Ok(paths.iter().map(|p| p.replacen(&root, "", 1)).collect())
            }
            Err(err) => Err(err),
        }
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

    pub fn save(&self, id: Uuid, content: String) -> Result<(), String> {
        write_document(&self.config, id, content.as_bytes()).map_err(|err| format!("{:?}", err))
    }

    pub fn open(&self, id: &Uuid) -> Result<(FileMetadata, String), String> {
        match self.file_by_id(*id) {
            Ok(meta) => match self.read(meta.id) {
                Ok(decrypted) => Ok((meta, String::from_utf8_lossy(&decrypted).to_string())),
                Err(err) => Err(err),
            },
            Err(err) => Err(format!("{:?}", err)),
        }
    }

    pub fn read(&self, id: Uuid) -> Result<DecryptedDocument, String> {
        read_document(&self.config, id).map_err(|err| format!("{:?}", err))
    }

    pub fn get_children_recursively(&self, id: Uuid) -> Result<Vec<FileMetadata>, String> {
        get_and_get_children_recursively(&self.config, id).map_err(|err| format!("{:?}", err))
    }

    pub fn delete(&self, id: &Uuid) -> Result<(), String> {
        delete_file(&self.config, *id).map_err(|err| format!("{:?}", err))
    }

    pub fn rename(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        rename_file(&self.config, *id, new_name).map_err(errs::rename::to_lb_error)
    }

    pub fn sync(&self, chan: &GlibSender<LbSyncMsg>) -> LbResult<()> {
        let account = self.account().unwrap().unwrap();

        let mut work: WorkCalculated;
        while {
            work = match self.calculate_work() {
                Ok(w) => w,
                Err(err) => return Err(err),
            };
            !work.work_units.is_empty()
        } {
            let total = work.work_units.len();

            for (i, wu) in work.work_units.iter().enumerate() {
                let path = self.full_path_for(&wu.get_metadata());
                chan.send(LbSyncMsg::Doing(wu.clone(), path, i + 1, total))
                    .unwrap();

                if let Err(err) = self.do_work(&account, wu) {
                    return Err(err);
                }
            }

            if let Err(err) = self.set_last_synced(work.most_recent_update_from_server) {
                chan.send(LbSyncMsg::Error(err)).unwrap();
            }
        }

        chan.send(LbSyncMsg::Done).unwrap();
        Ok(())
    }

    pub fn calculate_work(&self) -> LbResult<WorkCalculated> {
        calculate_work(&self.config).map_err(errs::calc_work::to_lb_error)
    }

    fn do_work(&self, a: &Account, wu: &WorkUnit) -> LbResult<()> {
        execute_work(&self.config, &a, wu.clone()).map_err(errs::exec_work::to_lb_error)
    }

    fn set_last_synced(&self, last_sync: u64) -> LbResult<()> {
        set_last_synced(&self.config, last_sync).map_err(|err| match err {
            CoreError::UiError(lockbook_core::SetLastSyncedError::Stub) => panic!("impossible"),
            CoreError::Unexpected(msg) => LbError::Program(msg),
        })
    }

    pub fn get_last_synced(&self) -> Result<i64, String> {
        get_last_synced(&self.config).map_err(|err| format!("{:?}", err))
    }

    pub fn usage(&self) -> Result<(u64, f64), String> {
        let fake_limit = KILOBYTE as f64 * 20.0;
        match get_usage(&self.config) {
            Ok(u) => Ok((u.into_iter().map(|usage| usage.byte_secs).sum(), fake_limit)),
            Err(err) => Err(format!("{:?}", err)),
        }
    }
}

mod errs {
    macro_rules! imports {
        ($enum:ident $(as $rename:ident )?, $( $( $variants:ident ).+ $(as $renames:ident )?),+) => {
            use crate::error::LbError;
            use lockbook_core::{Error, Error::UiError, Error::Unexpected};
            use lockbook_core::$enum $( as $rename )?;
            $( $( use lockbook_core::$enum::$variants )+ $( as $renames )?; )*
        };
    }

    macro_rules! user {
        ($base:literal $(, $args:tt )*) => {
            LbError::User(format!($base $(, $args )*))
        };
    }

    macro_rules! prog {
        ($msg:expr) => {
            LbError::Program($msg)
        };
    }

    pub mod create_account {
        imports!(
            CreateAccountError as CreatingAcct,
            UsernameTaken as UnameTaken,
            InvalidUsername as UnameInvalid,
            AccountExistsAlready as AccountExists,
            CouldNotReachServer,
            ClientUpdateRequired
        );

        const UNAME_REQS: &str = "letters and numbers only";

        pub fn to_lb_error(e: Error<CreatingAcct>, uname: &str) -> LbError {
            match e {
                UiError(e) => match e {
                    UnameTaken => user!("The username '{}' is already taken.", uname),
                    UnameInvalid => user!("Invalid username '{}' ({}).", uname, UNAME_REQS),
                    AccountExists => user!("An account already exists."),
                    CouldNotReachServer => user!("Unable to connect to the server."),
                    ClientUpdateRequired => user!("Client upgrade required."),
                },
                Unexpected(msg) => prog!(msg),
            }
        }
    }

    pub mod export_account {
        imports!(AccountExportError as Exporting, NoAccount);

        pub fn to_lb_error(e: Error<Exporting>) -> LbError {
            match e {
                UiError(NoAccount) => user!("No account found"),
                Unexpected(msg) => prog!(msg),
            }
        }
    }

    pub mod rename {
        imports!(
            RenameFileError as Renaming,
            CannotRenameRoot as IsRoot,
            FileDoesNotExist as NotExist,
            FileNameNotAvailable as NameNotAvail,
            NewNameContainsSlash as NameHasSlash,
            NewNameEmpty as NameIsEmpty
        );

        pub fn to_lb_error(e: Error<Renaming>) -> LbError {
            match e {
                UiError(e) => match e {
                    IsRoot => user!("The root folder cannot be renamed."),
                    NotExist => user!("The file you are trying to rename does not exist."),
                    NameNotAvail => user!("The new file name is not available."),
                    NameHasSlash => user!("File names cannot contain slashes."),
                    NameIsEmpty => user!("File names cannot be blank."),
                },
                Unexpected(msg) => prog!(msg),
            }
        }
    }

    pub mod calc_work {
        imports!(
            CalculateWorkError as CalculatingWork,
            CouldNotReachServer,
            ClientUpdateRequired,
            NoAccount
        );

        pub fn to_lb_error(e: Error<CalculatingWork>) -> LbError {
            match e {
                UiError(e) => match e {
                    CouldNotReachServer => user!("Unable to connect to the server."),
                    ClientUpdateRequired => user!("Client upgrade required."),
                    NoAccount => user!("No account found."),
                },
                Unexpected(msg) => prog!(msg),
            }
        }
    }

    pub mod exec_work {
        imports!(
            ExecuteWorkError as ExecutingWork,
            CouldNotReachServer,
            ClientUpdateRequired,
            BadAccount
        );

        pub fn to_lb_error(e: Error<ExecutingWork>) -> LbError {
            match e {
                UiError(e) => match e {
                    CouldNotReachServer => user!("Unable to connect to the server."),
                    ClientUpdateRequired => user!("Client upgrade required."),
                    BadAccount => user!("wut"),
                },
                Unexpected(msg) => prog!(msg),
            }
        }
    }
}
