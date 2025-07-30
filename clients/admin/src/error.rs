use std::backtrace::Backtrace;
use std::fmt::Debug;

pub struct Error;

impl<T: Debug> From<T> for Error {
    fn from(err: T) -> Self {
        eprintln!("error: {err:?}");
        eprintln!("{:?}", Backtrace::force_capture());
        Error
    }
}
