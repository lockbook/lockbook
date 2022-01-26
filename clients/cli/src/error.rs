use crate::err_unexpected;
use lockbook_core::UnexpectedError;

pub type CliResult<T> = Result<T, Error>;

pub enum Error {
    Simple(ErrorKind),
    Custom(CustomError),
}

impl Error {
    pub fn print(&self) {
        match self {
            Self::Simple(base) => eprintln!("error: {}", base.msg()),
            Self::Custom(err) => {
                let base_msg = err.base.msg();
                let with_ctx = match &err.context {
                    Some(ctx) => format!("{}: {}", ctx, base_msg),
                    None => base_msg,
                };

                eprintln!("error: {}", with_ctx);
                if let Some(extra) = &err.extra {
                    eprintln!("{}", extra);
                }
            }
        }
    }

    pub fn exit(&self) -> ! {
        self.print();
        std::process::exit(match self {
            Self::Simple(base) => base.code(),
            Self::Custom(err) => err.base.code(),
        })
    }
}

impl From<UnexpectedError> for Error {
    fn from(e: UnexpectedError) -> Self {
        err_unexpected!("unexpected error: {}", e)
    }
}

macro_rules! underscore {
    ($t:ty) => {
        _
    };
}

macro_rules! make_errkind_enum {
    ($( $codes:literal => $variants:ident $( ( $( $types:ty ),* ) )? ,)*) => {
        pub enum ErrorKind {
            $( $variants $( ( $( $types ),* ) )?, )*
        }

        impl ErrorKind {
            pub fn code(&self) -> i32 {
                match self {
                    $( Self::$variants $( ( $( underscore!($types) ),* ) )? => $codes ,)*
                }
            }

            pub fn print_table() -> CliResult<()> {
                $( println!("{:>6}  {}", $codes, stringify!($variants)); )*
                Ok(())
            }
        }
    };
}

type IoError = std::io::Error;

// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
make_errkind_enum!(
    // Miscellaneous (3-19)
    3 => Unexpected(String),
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
    24 => UsernameTaken(String),
    25 => UsernameInvalid(String),
    26 => UsernamePkMismatch,

    // OS (30s)
    30 => OsPwdMissing(IoError),
    31 => OsCouldNotCreateDir(String, IoError),
    32 => OsCouldNotCreateFile(String, IoError),
    33 => OsCouldNotWriteFile(String, IoError),
    34 => OsCouldNotDeleteFile(String, IoError),
    35 => OsInvalidPath(String),
    36 => OsFileCollision(String),

    // Lockbook file ops (40-52)
    40 => FileNotFound(String),
    41 => FileAlreadyExists(String),
    42 => FileNameEmpty,
    43 => FileNameNotAvailable(String),
    44 => FileNameHasSlash(String),
    45 => PathNoRoot(String),
    46 => PathContainsEmptyFile(String),
    47 => DocTreatedAsFolder(String),
    48 => CannotMoveFolderIntoItself,
    49 => NoRootOps(&'static str),
    50 => InvalidDrawing(String),
    51 => FolderTreatedAsDoc(String),

    // Validation errors (53 - 57)
    53 => FileOrphaned(String),
    54 => CycleDetected,
    55 => NameConflictDetected(String),
    56 => DocumentReadError(String, String),
    57 => WarningsFound(i32),
);

impl ErrorKind {
    pub fn msg(&self) -> String {
        match self {
            Self::Unexpected(msg) => msg.to_string(),
            Self::NetworkIssue => "Could not reach server!".to_string(),
            Self::UpdateRequired => "An update to your application is required to do this action!".to_string(),
            Self::UninstallRequired => "Your local state cannot be migrated, please re-sync with a fresh client.".to_string(),
            Self::ExpectedStdin => "expected stdin".to_string(),
            Self::NoCliLocation => "Could not read env var LOCKBOOK_CLI_LOCATION HOME or HOMEPATH, don't know where to place your `.lockbook` folder".to_string(),
            Self::NoRoot => "No root folder, have you synced yet?".to_string(),
            Self::ServerDisabled => "Server has disabled this feature.".to_string(),

            Self::NoAccount => "No account! Run 'new-account' or 'import-private-key' to get started!".to_string(),
            Self::AccountAlreadyExists => "Account already exists. Run `lockbook erase-everything` to erase your local state.".to_string(),
            Self::AccountDoesNotExistOnServer => "An account with this username was not found on the server.".to_string(),
            Self::AccountStringCorrupted => "Account string corrupted, not imported".to_string(),
            Self::UsernameTaken(uname) => format!("username '{}' is already taken.", uname),
            Self::UsernameInvalid(uname) => format!("username '{}' invalid (a-z || 0-9).", uname),
            Self::UsernamePkMismatch => "The public_key in this account_string does not match what is on the server.".to_string(),

            Self::OsPwdMissing(err) => format!("getting PWD from OS: {}", err),
            Self::OsCouldNotCreateDir(path, err) => format!("could not create directory '{}': {}", path, err),
            Self::OsCouldNotCreateFile(path, err) => format!("could not create file '{}': {}", path, err),
            Self::OsCouldNotWriteFile(path, err) => format!("could not write file '{}': {}", path, err),
            Self::OsCouldNotDeleteFile(path, err) => format!("could not delete file '{}': {}", path, err),
            Self::OsInvalidPath(path) => format!("'{}' is an invalid path", path),
            Self::OsFileCollision(path) => format!("A file collision was detected in '{}'", path),

            Self::FileNotFound(path) => format!("file '{}' not found", path),
            Self::FileAlreadyExists(path) => format!("the file '{}' already exists", path),
            Self::FileNameEmpty => "The file name provided is empty!".to_string(),
            Self::FileNameNotAvailable(name) => format!("File name '{}' is not available.", name),
            Self::FileNameHasSlash(name) => format!("File name '{}' has a slash.", name),
            Self::PathNoRoot(path) => format!("Path '{}' does not start with your root folder.", path),
            Self::PathContainsEmptyFile(path) => format!("the path '{}' contains an empty file name", path),
            Self::DocTreatedAsFolder(path) => format!("a file in path '{}' is a document being treated as a folder", path),
            Self::CannotMoveFolderIntoItself => "Cannot move file into its self or children.".to_string(),
            Self::NoRootOps(op) => format!("cannot {} your root directory!", op),
            Self::InvalidDrawing(name) => format!("'{}' is an invalid drawing", name),
            Self::FolderTreatedAsDoc(path) => format!("a file in path '{}' is a folder being treated as a document", path),

            Self::FileOrphaned(path) => format!("file '{}' has no path to root", path),
            Self::CycleDetected => "A cycle was detected in the file hierarchy".to_string(),
            Self::NameConflictDetected(path) => format!("A name conflict was detected for file at path `{}`", path),
            Self::DocumentReadError(path, err) => format!("{} was not readable due to: {}", path, err),
            Self::WarningsFound(count) => format!("{} warnings found", count),
        }
    }
}

pub struct CustomError {
    pub base: ErrorKind,
    pub context: Option<String>,
    pub extra: Option<String>,
}

#[macro_export]
macro_rules! err {
    ($err:ident $( ( $( $args:expr ),+ ) )?) => {
        $crate::error::Error::Simple($crate::error::ErrorKind::$err $( ( $( $args ),+ ) )?)
    };
}

#[macro_export]
macro_rules! err_unexpected {
    ($base:literal $(, $fmtargs:expr )*) => {
        $crate::error::Error::Simple($crate::error::ErrorKind::Unexpected({
            let msg = format!($base $(, $fmtargs )*);
            format!("unexpected: {}", msg)
        }))
    };
}

#[macro_export]
macro_rules! err_extra {
    ($err:ident $( ( $( $args:expr ),+ ) )?, $base:literal $(, $fmtargs:expr )*) => {
        $crate::error::Error::Custom($crate::error::CustomError {
            base: $crate::error::ErrorKind::$err $( ( $( $args ),+ ) )?,
            context: None,
            extra: Some(format!($base $(, $fmtargs )*)),
        })
    };
}
