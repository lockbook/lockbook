// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html

macro_rules! underscore {
    ($t:ty) => { _ };
}

macro_rules! make_errcode_enum {
    ($( $variants:ident $( ( $( $types:ty ),* ) )? => $values:literal ,)*) => {
        pub enum ErrorKind {
            $( $variants $( ( $( $types ),* ) )?, )*
        }

        impl ErrorKind {
            pub fn code(&self) -> i32 {
                match self {
                    $( Self::$variants $( ( $( underscore!($types) ),* ) )? => $values ,)*
                }
            }
        }
    };
}

make_errcode_enum!(
    Success => 0,

    Unexpected(String) => 5,
    NetworkIssue => 4,
    UpdateRequired => 25,
    UninstallRequired => 26,
    ExpectedStdin => 6,
    NoCliLocation => 24,
    PwdMissing => 30,
    NoRoot => 10,
    NoRootOps => 29,

    NoAccount => 8,
    AccountAlreadyExists => 21,
    AccountDoesNotExist => 22,
    AccountStringCorrupted => 7,
    UsernameTaken => 1,
    UsernameInvalid => 3,
    UsernamePkMismatch => 23,

    OsCouldNotGetAbsPath(String, std::io::Error) => 16,
    OsCouldNotCreateDir => 31,
    OsCouldNotReadChildren => 34,
    OsCouldNotReadFile(String, std::io::Error) => 15,
    OsCouldNotWriteFile => 18,
    OsCouldNotDeleteFile => 180,

    FileNotFound => 17,
    FileAlreadyExists(String) => 9,
    FileNameEmpty => 28,
    FileNameNotAvailable => 20,
    FileNameHasSlash => 19,
    PathNoRoot => 11,
    PathContainsEmptyFile(String) => 27,
    DocTreatedAsFolder(String) => 12,
    CannotMoveFolderIntoItself => 32,
    CannotDeleteRoot => 33,
);

impl ErrorKind {
    pub fn msg(&self) -> String {
        match self {
            Self::Unexpected(msg) => msg.to_string(),
            Self::NoRoot => "No root folder, have you synced yet?".to_string(),

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
