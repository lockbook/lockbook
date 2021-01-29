// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html

macro_rules! underscore {
    ($t:ty) => { _ };
}

macro_rules! make_errcode_enum {
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
        }
    };
}

make_errcode_enum!(
    // Miscellaneous (3-19)
    3 => Unexpected(String),
    4 => NetworkIssue,
    5 => UpdateRequired,
    6 => UninstallRequired,
    7 => ExpectedStdin,
    8 => NoCliLocation,
    9 => PwdMissing(std::io::Error),
    10 => NoRoot,
    11 => NoRootOps(String),

    // Account (20s)
    20 => NoAccount,
    21 => AccountAlreadyExists,
    22 => AccountDoesNotExist,
    23 => AccountStringCorrupted,
    24 => UsernameTaken,
    25 => UsernameInvalid,
    26 => UsernamePkMismatch,

    // OS (30s)
    30 => OsCouldNotGetAbsPath(String, std::io::Error),
    31 => OsCouldNotCreateDir,
    32 => OsCouldNotReadChildren,
    33 => OsCouldNotReadFile(String, std::io::Error),
    34 => OsCouldNotWriteFile,
    35 => OsCouldNotDeleteFile,

    // Lockbook file ops (40s)
    40 => FileNotFound,
    41 => FileAlreadyExists(String),
    42 => FileNameEmpty,
    43 => FileNameNotAvailable,
    44 => FileNameHasSlash,
    45 => PathNoRoot,
    46 => PathContainsEmptyFile(String),
    47 => DocTreatedAsFolder(String),
    48 => CannotMoveFolderIntoItself,
    49 => CannotDeleteRoot,
);

impl ErrorKind {
    pub fn msg(&self) -> String {
        match self {
            Self::Unexpected(msg) => msg.to_string(),
            Self::NetworkIssue => "Could not reach server!".to_string(),
            Self::UpdateRequired => {
                "An update to your application is required to do this action!".to_string()
            }
            Self::UninstallRequired => {
                "Your local state cannot be migrated, please re-sync with a fresh client."
                    .to_string()
            }
            Self::ExpectedStdin => "expected stdin".to_string(),
            Self::NoCliLocation => "Could not read env var LOCKBOOK_CLI_LOCATION HOME or HOMEPATH, don't know where to place your `.lockbook` folder".to_string(),
            Self::PwdMissing(err) => format!("getting PWD from OS: {}", err),
            Self::NoRoot => "No root folder, have you synced yet?".to_string(),
            Self::NoRootOps(op) => format!("cannot {} root directory!", op),

            Self::NoAccount => "No account! Run init or import to get started!".to_string(),

            Self::OsCouldNotGetAbsPath(path, err) => format!(
                "could not get the absolute path for {}, os error: {}",
                path, err
            ),
            Self::OsCouldNotReadFile(path, err) => {
                format!("could not read file {}, os error: {}", path, err)
            }

            Self::FileAlreadyExists(path) => {
                format!("the file '{}' already exists", path)
            }
            Self::PathContainsEmptyFile(path) => {
                format!("the path '{}' contains an empty file name", path)
            }
            Self::DocTreatedAsFolder(path) => format!(
                "a file in path '{}' is a document being treated as a folder",
                path
            ),

            _ => "I heart Golang".to_string(),
        }
    }
}

pub struct CustomError {
    pub base: ErrorKind,
    pub context: Option<String>,
    pub extra: Option<String>,
}

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
                eprintln!(
                    "{}",
                    match &err.context {
                        Some(ctx) => format!("{}: {}", ctx, base_msg),
                        None => base_msg,
                    }
                );
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
    ($ekind:ident $( ( $( $args:expr ),+ ) )?, $base:literal $(, $fmtargs:expr )*) => {{
        let err = crate::error::ErrorKind::$ekind $( ( $( $args ),+ ) )?;
        eprintln!($base $(, $fmtargs )*);
        std::process::exit(err.code())
    }};
}
