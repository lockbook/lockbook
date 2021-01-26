// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
#[derive(Clone, Copy)]
pub enum ErrCode {
    Success = 0,

    Unexpected = 5,
    NetworkIssue = 4,
    UpdateRequired = 25,
    UninstallRequired = 26,
    ExpectedStdin = 6,
    NoCliLocation = 24,
    PwdMissing = 30,
    NoRootOps = 29,

    NoAccount = 8,
    AccountAlreadyExists = 21,
    AccountDoesNotExist = 22,
    AccountStringCorrupted = 7,
    UsernameTaken = 1,
    UsernameInvalid = 3,
    UsernamePkMismatch = 23,

    CouldNotReadOsFile = 15,
    CouldNotGetOsAbsPath = 16,
    CouldNotWriteToOsFile = 18,
    CouldNotDeleteOsFile = 180,
    CouldNotCreateOsDirectory = 31,
    CouldNotReadOsChildren = 34,

    FileNotFound = 17,
    FileAlreadyExists = 9,
    NameContainsSlash = 19,
    FileNameNotAvailable = 20,
    NoRoot = 10,
    PathNoRoot = 11,
    DocTreatedAsFolder = 12,
    CannotMoveFolderIntoItself = 32,
    CannotDeleteRoot = 33,
    PathContainsEmptyFile = 27,
    NameEmpty = 28,
}

#[macro_export]
macro_rules! exitlb {
    ($code:ident, $base:literal $(, $args:expr )*) => {{
        eprintln!($base $(, $args )*);
        std::process::exit(crate::error::ErrCode::$code as i32)
    }};
}
