use std::fmt;
use std::io;
use std::path::Path;

use lockbook_core::{Error as LbError, FeatureFlagError, File};
use lockbook_core::{GetAccountError, Uuid};
use lockbook_core::{GetSubscriptionInfoError, UnexpectedError};

pub struct CliError {
    pub code: ErrCode,
    pub msg: String,
}

impl CliError {
    fn new<S: ToString>(code: ErrCode, msg: S) -> Self {
        Self { msg: msg.to_string(), code }
    }

    pub fn with_extra<T: fmt::Display>(self, extra: T) -> Self {
        let code = self.code;
        let msg = format!("{}\n{}", self.msg, extra);
        Self { code, msg }
    }

    pub fn print(&self) {
        match self.code {
            ErrCode::Input => eprintln!("input error: {}", self.msg),
            ErrCode::Unexpected => eprintln!("unexpected error: {}", self.msg),
            _ => eprintln!("error: {}", self.msg),
        }
    }

    pub fn input<S: ToString>(msg: S) -> Self {
        Self { msg: msg.to_string(), code: ErrCode::Input }
    }

    pub fn unexpected<S: ToString>(msg: S) -> Self {
        Self { msg: msg.to_string(), code: ErrCode::Unexpected }
    }

    pub fn network_issue() -> Self {
        Self::new(ErrCode::NetworkIssue, "Could not reach server!")
    }

    pub fn update_required() -> Self {
        Self::new(
            ErrCode::UpdateRequired,
            "An update to your application is required to do this action!",
        )
    }

    pub fn expected_stdin() -> Self {
        Self::new(ErrCode::ExpectedStdin, "expected stdin")
    }

    pub fn no_cli_location() -> Self {
        Self::new(
            ErrCode::NoCliLocation,
            "Could not read env var LOCKBOOK_CLI_LOCATION HOME or HOMEPATH, don't know where to place your `.lockbook` folder",
        )
    }

    pub fn no_root() -> Self {
        Self::new(ErrCode::NoRoot, "No root folder, have you synced yet?")
    }

    pub fn server_disabled() -> Self {
        Self::new(
            ErrCode::ServerDisabled,
            "This server is not accepting new accounts at the moment. Please try again another time.",
        )
    }

    pub fn no_account() -> Self {
        Self::new(
            ErrCode::NoAccount,
            "No account! Run 'new-account' or 'import-private-key' to get started!",
        )
    }

    pub fn account_exists() -> Self {
        Self::new(
            ErrCode::AccountAlreadyExists,
            "Account already exists. Run `lockbook erase-everything` to erase your local state.",
        )
    }

    pub fn account_not_on_server() -> Self {
        Self::new(
            ErrCode::AccountNotOnServer,
            "An account with this username was not found on the server.",
        )
    }

    pub fn account_string_corrupted() -> Self {
        Self::new(ErrCode::AccountStringCorrupted, "Account string corrupted, not imported")
    }

    pub fn username_pk_mismatch() -> Self {
        Self::new(
            ErrCode::UsernamePkMismatch,
            "The public_key in this account_string does not match what is on the server.",
        )
    }

    pub fn file_name_empty() -> Self {
        Self::new(ErrCode::FileNameEmpty, "file name provided is empty!")
    }

    pub fn moving_folder_into_itself() -> Self {
        Self::new(
            ErrCode::CannotMoveFolderIntoItself,
            "Cannot move file into itself or its children.",
        )
    }

    pub fn cycle_detected() -> Self {
        Self::new(ErrCode::CycleDetected, "A cycle was detected in the file hierarchy")
    }

    pub fn username_taken(uname: &str) -> Self {
        Self::new(ErrCode::UsernameTaken, format!("username '{}' is already taken.", uname))
    }

    pub fn username_invalid(uname: &str) -> Self {
        Self::new(ErrCode::UsernameInvalid, format!("username '{}' invalid (a-z || 0-9).", uname))
    }

    pub fn path_has_empty_file<T: fmt::Display>(path: T) -> Self {
        Self::new(
            ErrCode::PathContainsEmptyFile,
            format!("path '{}' contains an empty file name", path),
        )
    }

    pub fn file_not_found<T: fmt::Display>(path: T) -> Self {
        Self::new(ErrCode::FileNotFound, format!("file '{}' not found", path))
    }

    pub fn file_id_not_found(id: Uuid) -> Self {
        Self::new(ErrCode::FileNotFound, format!("file-id '{id}' not found"))
    }

    pub fn file_exists<P: fmt::Display>(path: P) -> Self {
        Self::new(ErrCode::FileAlreadyExists, format!("the file '{}' already exists", path))
    }

    pub fn file_name_has_slash<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::FileNameHasSlash, format!("file name '{}' has a slash.", name))
    }

    pub fn file_name_taken<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::FileNameUnavailable, format!("file name '{}' is not available.", name))
    }

    pub fn file_name_too_long<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::FileNameTooLong, format!("file name '{}' is too long", name))
    }

    pub fn doc_treated_as_dir<T: fmt::Display>(lb_path: T) -> Self {
        Self::new(
            ErrCode::DocTreatedAsFolder,
            format!("a file in path '{}' is a document being treated as a folder", lb_path),
        )
    }

    pub fn dir_treated_as_doc(f: &File) -> Self {
        Self::new(
            ErrCode::FolderTreatedAsDoc,
            format!("a file named '{}' is a folder being treated as a document", f.name),
        )
    }

    pub fn invalid_drawing<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::InvalidDrawing, format!("'{}' is an invalid drawing", name))
    }

    pub fn no_root_ops(op: &'static str) -> Self {
        Self::new(ErrCode::NoRootOps, format!("cannot {} root folder!", op))
    }

    pub fn os_current_dir(err: io::Error) -> Self {
        Self::new(ErrCode::OsCwdMissing, format!("getting cwd: {}", err))
    }

    pub fn os_mkdir<P: AsRef<Path>>(path: P, err: io::Error) -> Self {
        Self::new(
            ErrCode::OsCouldNotCreateDir,
            format!("could not create directory '{}': {}", path.as_ref().display(), err),
        )
    }

    pub fn os_create_file(path: &Path, err: io::Error) -> Self {
        Self::new(
            ErrCode::OsCouldNotCreateFile,
            format!("could not create file {:?}: {}", path, err),
        )
    }

    pub fn os_write_file<P: AsRef<Path>>(path: P, err: io::Error) -> Self {
        Self::new(
            ErrCode::OsCouldNotWriteFile,
            format!("could not write file '{}': {}", path.as_ref().display(), err),
        )
    }

    pub fn os_delete_file<P: AsRef<Path>>(path: P, err: io::Error) -> Self {
        Self::new(
            ErrCode::OsCouldNotDeleteFile,
            format!("could not delete file '{}': {}", path.as_ref().display(), err),
        )
    }

    pub fn os_invalid_path<P: AsRef<Path>>(path: P) -> Self {
        Self::new(
            ErrCode::OsInvalidPath,
            format!("'{}' is an invalid path", path.as_ref().display()),
        )
    }

    pub fn os_file_collision<P: AsRef<Path>>(path: P) -> Self {
        Self::new(
            ErrCode::OsFileCollision,
            format!("A file collision was detected in '{}'", path.as_ref().display()),
        )
    }

    pub fn validate_warnings_found(n: usize) -> Self {
        Self::new(ErrCode::WarningsFound, format!("{} warnings found", n))
    }

    pub fn file_orphaned<T: fmt::Display>(lb_path: T) -> Self {
        Self::new(ErrCode::FileOrphaned, format!("file '{}' has no path to root", lb_path))
    }

    pub fn name_conflict_detected<T: fmt::Display>(ids: T) -> Self {
        Self::new(
            ErrCode::NameConflictDetected,
            format!("A name conflict was detected for these files: `{}`", ids),
        )
    }

    pub fn validate_doc_read<T: fmt::Display>(lb_path: T, err: T) -> Self {
        Self::new(ErrCode::DocumentReadError, format!("{} unreadable: {}", lb_path, err))
    }

    pub fn billing<T: fmt::Display>(msg: T) -> Self {
        Self::new(ErrCode::Billing, msg)
    }

    pub fn insufficient_permission() -> Self {
        Self::new(ErrCode::InsufficientPermission, "your account is unauthorized.".to_string())
    }

    pub fn link_in_shared<T: fmt::Display>(name: T) -> Self {
        Self::new(
            ErrCode::SharingError,
            format!("{} is a link and cannot be moved into a shared folder.", name),
        )
    }

    pub fn no_write_permission<T: fmt::Display>(name: T) -> Self {
        Self::new(
            ErrCode::InsufficientPermission,
            format!("You don't have permission to modify {}", name),
        )
    }
}

macro_rules! make_errcode_enum {
    ($( $codes:literal => $variants:ident $( ( $( $types:ty ),* ) )? ,)*) => {
        pub enum ErrCode {
            $( $variants = $codes , )*
        }

        pub fn print_err_table() -> Result<(), CliError> {
            $( println!("{:>6}  {}", $codes, stringify!($variants)); )*
            Ok(())
        }
    };
}

// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
make_errcode_enum!(
    // Miscellaneous (1-19)
    1 => Input,
    3 => Unexpected,
    4 => NetworkIssue,
    5 => UpdateRequired,
    6 => ExpectedStdin,
    7 => NoCliLocation,
    8 => NoRoot,
    9 => ServerDisabled,

    // Account (20s)
    20 => NoAccount,
    21 => AccountAlreadyExists,
    22 => AccountNotOnServer,
    23 => AccountStringCorrupted,
    24 => UsernameTaken,
    25 => UsernameInvalid,
    26 => UsernamePkMismatch,
    27 => Billing,
    28 => InsufficientPermission,

    // OS (30s)
    30 => OsCwdMissing,
    31 => OsCouldNotCreateDir,
    32 => OsCouldNotCreateFile,
    33 => OsCouldNotWriteFile,
    34 => OsCouldNotDeleteFile,
    35 => OsInvalidPath,
    36 => OsFileCollision,

    // Lockbook file ops (40-52)
    40 => FileNotFound,
    41 => FileAlreadyExists,
    42 => FileNameEmpty,
    43 => FileNameUnavailable,
    44 => FileNameHasSlash,
    46 => PathContainsEmptyFile,
    47 => DocTreatedAsFolder,
    48 => CannotMoveFolderIntoItself,
    49 => NoRootOps,
    50 => InvalidDrawing,
    51 => FolderTreatedAsDoc,
    52 => FileNameTooLong,

    // Validation errors (53 - 57)
    53 => FileOrphaned,
    54 => CycleDetected,
    55 => NameConflictDetected,
    56 => DocumentReadError,
    57 => WarningsFound,

    // Sharing errors (60s)
    60 => SharingError,
);

impl From<UnexpectedError> for CliError {
    fn from(e: UnexpectedError) -> Self {
        Self::unexpected(format!("unexpected error: {}", e))
    }
}

impl From<LbError<GetAccountError>> for CliError {
    fn from(e: LbError<GetAccountError>) -> Self {
        match e {
            LbError::UiError(GetAccountError::NoAccount) => Self::no_account(),
            LbError::Unexpected(msg) => Self::unexpected(msg),
        }
    }
}

impl From<LbError<GetSubscriptionInfoError>> for CliError {
    fn from(err: LbError<GetSubscriptionInfoError>) -> Self {
        match err {
            LbError::UiError(GetSubscriptionInfoError::CouldNotReachServer) => {
                CliError::network_issue()
            }
            LbError::UiError(GetSubscriptionInfoError::ClientUpdateRequired) => {
                CliError::update_required()
            }
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }
    }
}

impl From<LbError<FeatureFlagError>> for CliError {
    fn from(err: LbError<FeatureFlagError>) -> Self {
        match err {
            LbError::UiError(FeatureFlagError::CouldNotReachServer) => CliError::network_issue(),
            LbError::UiError(FeatureFlagError::ClientUpdateRequired) => CliError::update_required(),
            LbError::UiError(FeatureFlagError::InsufficientPermission) => {
                CliError::insufficient_permission()
            }
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self::unexpected(err.to_string())
    }
}
