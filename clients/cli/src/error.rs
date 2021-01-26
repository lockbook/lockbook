// Error Codes, respect: http://www.tldp.org/LDP/abs/html/exitcodes.html
#[derive(Clone, Copy)]
pub enum ErrCode {
    Success = 0,
    UsernameTaken = 1,
    UsernameInvalid = 3,
    NetworkIssue = 4,
    ExpectedStdin = 6,
    AccountStringCorrupted = 7,
    NoAccount = 8,
    FileAlreadyExists = 9,
    NoRoot = 10,
    PathNoRoot = 11,
    DocTreatedAsFolder = 12,
    CouldNotReadOsFile = 15,
    CouldNotGetOsAbsPath = 16,
    FileNotFound = 17,
    CouldNotWriteToOsFile = 18,
    CouldNotDeleteOsFile = 180,
    NameContainsSlash = 19,
    FileNameNotAvailable = 20,
    AccountAlreadyExists = 21,
    AccountDoesNotExist = 22,
    UsernamePkMismatch = 23,
    NoCliLocation = 24,
    UpdateRequired = 25,
    UninstallRequired = 26,
    PathContainsEmptyFile = 27,
    NameEmpty = 28,
    NoRootOps = 29,
    PwdMissing = 30,
    CouldNotCreateOsDirectory = 31,
    CannotMoveFolderIntoItself = 32,
    CannotDeleteRoot = 33,
    CouldNotReadOsChildren = 34,
    Unexpected = 5,
}

#[macro_export]
macro_rules! exitlb {
    ($code:ident, $base:literal $(, $args:expr )*) => {{
        eprintln!($base $(, $args )*);
        std::process::exit(crate::error::ErrCode::$code as i32)
    }};
}
