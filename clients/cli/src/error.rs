use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use lockbook_core::model::errors::GetAccountError;
use lockbook_core::Error as LbError;
use lockbook_core::UnexpectedError;

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
            ErrCode::Unexpected => eprintln!("unexpected error: {}", self.msg),
            _ => eprintln!("error: {}", self.msg),
        }
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

    pub fn uninstall_required() -> Self {
        Self::new(
            ErrCode::UninstallRequired,
            "Your local state cannot be migrated, please re-sync with a fresh client.",
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
            ErrCode::AccountDoesNotExistOnServer,
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

    pub fn path_no_root<T: fmt::Display>(path: T) -> Self {
        Self::new(
            ErrCode::PathNoRoot,
            format!("path '{}' does not begin with your root folder.", path),
        )
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

    pub fn file_exists<P: fmt::Display>(path: P) -> Self {
        Self::new(ErrCode::FileAlreadyExists, format!("the file '{}' already exists", path))
    }

    pub fn file_name_has_slash<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::FileNameHasSlash, format!("file name '{}' has a slash.", name))
    }

    pub fn file_name_taken<T: fmt::Display>(name: T) -> Self {
        Self::new(ErrCode::FileNameUnavailable, format!("file name '{}' is not available.", name))
    }

    pub fn doc_treated_as_dir<T: fmt::Display>(lb_path: T) -> Self {
        Self::new(
            ErrCode::DocTreatedAsFolder,
            format!("a file in path '{}' is a document being treated as a folder", lb_path),
        )
    }

    pub fn dir_treated_as_doc<T: fmt::Display>(lb_path: T) -> Self {
        Self::new(
            ErrCode::FolderTreatedAsDoc,
            format!("a file in path '{}' is a folder being treated as a document", lb_path),
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

    pub fn os_create_file(path: &PathBuf, err: io::Error) -> Self {
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

    pub fn name_conflict_detected<T: fmt::Display>(lb_path: T) -> Self {
        Self::new(
            ErrCode::NameConflictDetected,
            format!("A name conflict was detected for file at path `{}`", lb_path),
        )
    }

    pub fn validate_doc_read<T: fmt::Display>(lb_path: T, err: T) -> Self {
        Self::new(ErrCode::DocumentReadError, format!("{} unreadable: {}", lb_path, err))
    }
}

macro_rules! make_errkind_enum {
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
make_errkind_enum!(
    // Miscellaneous (3-19)
    3 => Unexpected,
    4 => NetworkIssue,
    5 => UpdateRequired,
    6 => UninstallRequired,
    7 => ExpectedStdin,
    8 => NoCliLocation,
    9 => NoRoot,
    10 => ServerDisabled,

    // Account (20s)
    20 => NoAccount,
    21 => AccountAlreadyExists,
    22 => AccountDoesNotExistOnServer,
    23 => AccountStringCorrupted,
    24 => UsernameTaken,
    25 => UsernameInvalid,
    26 => UsernamePkMismatch,

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
    45 => PathNoRoot,
    46 => PathContainsEmptyFile,
    47 => DocTreatedAsFolder,
    48 => CannotMoveFolderIntoItself,
    49 => NoRootOps,
    50 => InvalidDrawing,
    51 => FolderTreatedAsDoc,

    // Validation errors (53 - 57)
    53 => FileOrphaned,
    54 => CycleDetected,
    55 => NameConflictDetected,
    56 => DocumentReadError,
    57 => WarningsFound,
);

impl From<UnexpectedError> for CliError {
    fn from(e: UnexpectedError) -> Self {
        Self::unexpected(format!("unexpected error: {}", e))
    }
}

impl From<LbError<GetAccountError>> for CliError {
    fn from(e: LbError<GetAccountError>) -> Self {
        match e {
            LbError::UiError(GetAccountError::NoAccount) => CliError::no_account(),
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }
    }
}
