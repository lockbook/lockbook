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
    "canceling subscription", CancelSubscriptionError,
    "feature flag err", FeatureFlagError,
    "getting subscription info", GetSubscriptionInfoError,
    "sharing file", ShareFileError,
    "upgrading via stripe", UpgradeAccountStripeError
);

impl From<(LbError<lb::DeletePendingShareError>, Uuid)> for CliError {
    fn from(v: (LbError<lb::DeletePendingShareError>, Uuid)) -> Self {
        let (err, id) = v;
        Self(format!("deleting pending share '{}': {:?}", id, err))
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self(format!("{:?}", err))
    }
}
