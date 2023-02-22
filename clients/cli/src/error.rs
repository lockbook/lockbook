use std::fmt;
use std::io;
use std::path::PathBuf;

use lb::Error as LbError;
use lb::Uuid;

pub struct CliError(pub String);

impl CliError {
    pub fn new(msg: impl ToString) -> Self {
        Self(msg.to_string())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.0)
    }
}

impl From<lb::CoreError> for CliError {
    fn from(err: lb::CoreError) -> Self {
        Self(format!("{:?}", err))
    }
}

impl From<lb::UnexpectedError> for CliError {
    fn from(err: lb::UnexpectedError) -> Self {
        Self(format!("unexpected: {:?}", err))
    }
}

macro_rules! impl_from_lb_errors_for_cli_error {
    ($( $ctx:literal, $uierr:ident ),*) => {
        $(
            impl From<LbError<lb::$uierr>> for CliError {
                fn from(err: LbError<lb::$uierr>) -> Self {
                    Self(format!("{}: {:?}", $ctx, err))
                }
            }
        )*
    };
}

#[rustfmt::skip]
impl_from_lb_errors_for_cli_error!(
    "exporting account", AccountExportError,
    "calculating work", CalculateWorkError,
    "canceling subscription", CancelSubscriptionError,
    "creating account", CreateAccountError,
    "creating file at path", CreateFileAtPathError,
    "feature flag err", FeatureFlagError,
    "getting account", GetAccountError,
    "getting root", GetRootError,
    "getting subscription info", GetSubscriptionInfoError,
    "getting usage", GetUsageError,
    "importing account", ImportError,
    "sharing file", ShareFileError,
    "syncing", SyncAllError,
    "upgrading via stripe", UpgradeAccountStripeError
);

impl From<(LbError<lb::DeletePendingShareError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::DeletePendingShareError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("deleting pending share '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::ExportDrawingError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::ExportDrawingError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("exporting drawing with id '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::ExportFileError>, PathBuf)> for CliError {
    fn from(v: (LbError<lb::ExportFileError>, PathBuf)) -> Self {
        let (err, disk_dir) = v;
        Self(format!("exporting file to {:?}: {:?}", disk_dir, err))
    }
}

impl From<(LbError<lb::FileDeleteError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::FileDeleteError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("deleting file with id '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::GetAndGetChildrenError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::GetAndGetChildrenError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("get and get children of '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::GetFileByIdError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::GetFileByIdError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("get file by id '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::GetFileByPathError>, &str)> for CliError {
    fn from(v: (LbError<lb::GetFileByPathError>, &str)) -> Self {
        let (err, path) = v;
        Self(format!("get file by path '{}': {:?}", path, err))
    }
}

impl From<(LbError<lb::ImportFileError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::ImportFileError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("importing file to '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::MoveFileError>, Uuid, Uuid)> for CliError {
    fn from(v: (LbError<lb::MoveFileError>, Uuid, Uuid)) -> Self {
        let (err, src_id, dest_id) = v;
        Self(format!("moving '{}' -> '{}': {:?}", src_id, dest_id, err))
    }
}

impl From<(LbError<lb::ReadDocumentError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::ReadDocumentError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("reading doc '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::RenameFileError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::RenameFileError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("renaming file '{}': {:?}", id, err))
    }
}

impl From<(LbError<lb::WriteToDocumentError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::WriteToDocumentError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("writing doc '{}': {:?}", id, err))
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self(format!("{:?}", err))
    }
}
