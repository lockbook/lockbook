pub type CliResult = Result<(), Error>;

pub enum Error {
    Simple(ErrorKind),
    Custom(CustomError),
}

impl Error {
    pub fn print(&self) {
        match self {
            Self::Simple(base) => eprintln!("{}", base.msg()),
            Self::Custom(err) => {
                let base_msg = err.base.msg();
                let with_ctx = match &err.context {
                    Some(ctx) => format!("{}: {}", ctx, base_msg),
                    None => base_msg,
                };

                eprintln!("{}", with_ctx);
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

macro_rules! underscore {
    ($t:ty) => { _ };
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

            pub fn print_table() -> CliResult {
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
    10 => NoRootOps(String),

    // Account (20s)
    20 => NoAccount,
    21 => AccountAlreadyExists,
    22 => AccountDoesNotExist,
    23 => AccountStringCorrupted,
    24 => UsernameTaken(String),
    25 => UsernameInvalid(String),
    26 => UsernamePkMismatch,

    // OS (30s)
    30 => OsPwdMissing(IoError),
    31 => OsCouldNotGetAbsPath(String, IoError),
    32 => OsCouldNotGetFileName(String),
    33 => OsCouldNotCreateDir(String, IoError),
    34 => OsCouldNotListChildren(String, IoError),
    35 => OsCouldNotReadFile(String, IoError),
    36 => OsCouldNotCreateFile(String, IoError),
    37 => OsCouldNotWriteFile(String, IoError),
    38 => OsCouldNotDeleteFile(String, IoError),

    // Lockbook file ops (40s)
    40 => FileNotFound(String),
    41 => FileAlreadyExists(String),
    42 => FileNameEmpty,
    43 => FileNameNotAvailable(String),
    44 => FileNameHasSlash(String),
    45 => PathNoRoot(String),
    46 => PathContainsEmptyFile(String),
    47 => DocTreatedAsFolder(String),
    48 => CannotMoveFolderIntoItself,
    49 => CannotDeleteRoot(String),
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
            Self::NoRootOps(op) => format!("cannot {} root directory!", op),

            Self::NoAccount => "No account! Run init or import to get started!".to_string(),
            Self::AccountAlreadyExists => "Account already exists. Run `lockbook erase-everything` to erase your local state.".to_string(),
            Self::AccountDoesNotExist => "An account with this username was not found on the server.".to_string(),
            Self::AccountStringCorrupted => "Account string corrupted, not imported".to_string(),
            Self::UsernameTaken(uname) => format!("username '{}' is already taken.", uname),
            Self::UsernameInvalid(uname) => format!("username '{}' invalid (a-z || 0-9).", uname),
            Self::UsernamePkMismatch => "The public_key in this account_string does not match what is on the server.".to_string(),

            Self::OsPwdMissing(err) => format!("getting PWD from OS: {}", err),
            Self::OsCouldNotGetAbsPath(path, err) => format!("could not get absolute path for '{}': {}", path, err),
            Self::OsCouldNotGetFileName(path) => format!("could not get file name for '{}'", path),
            Self::OsCouldNotCreateDir(path, err) => format!("could not create directory '{}': {}", path, err),
            Self::OsCouldNotListChildren(path, err) => format!("could not list children for directory '{}': {}", path, err),
            Self::OsCouldNotReadFile(path, err) => format!("could not read file '{}': {}", path, err),
            Self::OsCouldNotCreateFile(path, err) => format!("could not create file '{}': {}", path, err),
            Self::OsCouldNotWriteFile(path, err) => format!("could not write file '{}': {}", path, err),
            Self::OsCouldNotDeleteFile(path, err) => format!("could not delete file '{}': {}", path, err),

            Self::FileNotFound(path) => format!("file '{}' not found", path),
            Self::FileAlreadyExists(path) => format!("the file '{}' already exists", path),
            Self::FileNameEmpty => "The file name provided is empty!".to_string(),
            Self::FileNameNotAvailable(name) => format!("File name '{}' is not available.", name),
            Self::FileNameHasSlash(name) => format!("File name '{}' has a slash.", name),
            Self::PathNoRoot(path) => format!("Path '{}' does not start with your root folder.", path),
            Self::PathContainsEmptyFile(path) => format!("the path '{}' contains an empty file name", path),
            Self::DocTreatedAsFolder(path) => format!("a file in path '{}' is a document being treated as a folder", path),
            Self::CannotMoveFolderIntoItself => "Cannot move file into its self or children.".to_string(),
            Self::CannotDeleteRoot(path) => format!("Cannot delete '{}' since it is the root folder.", path),
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
        crate::error::Error::Simple(crate::error::ErrorKind::$err $( ( $( $args ),+ ) )?)
    };
}

#[macro_export]
macro_rules! err_unexpected {
    ($base:literal $(, $fmtargs:expr )*) => {
        crate::error::Error::Simple(crate::error::ErrorKind::Unexpected({
            let msg = format!($base $(, $fmtargs )*);
            format!("unexpected error: {}", msg)
        }))
    };
}

#[macro_export]
macro_rules! err_extra {
    ($err:ident $( ( $( $args:expr ),+ ) )?, $base:literal $(, $fmtargs:expr )*) => {
        crate::error::Error::Custom(crate::error::CustomError {
            base: crate::error::ErrorKind::$err $( ( $( $args ),+ ) )?,
            context: None,
            extra: Some(format!($base $(, $fmtargs )*)),
        })
    };
}

#[macro_export]
macro_rules! exitlb {
    ($err:ident $( ( $( $args:expr ),+ ) )?) => {{
        let err = crate::error::ErrorKind::$err $( ( $( $args ),+ ) )?;
        eprintln!("{}", err.msg());
        std::process::exit(err.code())
    }};
}
