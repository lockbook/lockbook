// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html

macro_rules! underscore {
    ($t:ty) => { _ };
}

macro_rules! make_errcode_enum {
    ($( $variants:ident $( ( $( $types:ty ),* ) )? => $values:literal ,)*) => {
        pub enum ErrCode {
            $( $variants $( ( $( $types ),* ) )?, )*
        }

        impl ErrCode {
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

    Unexpected => 5,
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

    OsCouldNotGetAbsPath => 16,
    OsCouldNotCreateDir => 31,
    OsCouldNotReadChildren => 34,
    OsCouldNotReadFile(String, std::io::Error) => 15,
    OsCouldNotWriteFile => 18,
    OsCouldNotDeleteFile => 180,

    FileNotFound => 17,
    FileAlreadyExists => 9,
    FileNameEmpty => 28,
    FileNameNotAvailable => 20,
    FileNameHasSlash => 19,
    PathNoRoot => 11,
    PathContainsEmptyFile => 27,
    DocTreatedAsFolder => 12,
    CannotMoveFolderIntoItself => 32,
    CannotDeleteRoot => 33,
);

impl ErrCode {
    pub fn msg(&self) -> String {
        match self {
            Self::NoRoot => "No root folder, have you synced yet?".to_string(),

            Self::OsCouldNotReadFile(path, err) => {
                format!("could not read file {}, os error: {}", path, err)
            }

            _ => "I heart Golang".to_string(),
        }
    }
}

#[macro_export]
macro_rules! exitlb {
    ($err:ident) => {{
        let err = crate::error::ErrCode::$err;
        eprintln!("{}", err.msg());
        std::process::exit(err.code())
    }};
    ($code:ident, $base:literal $(, $args:expr )*) => {{
        eprintln!($base $(, $args )*);
        std::process::exit(crate::error::ErrCode::$code.code())
    }};
}
