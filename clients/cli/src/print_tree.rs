use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::{get_file_by_path, read_document, Error as CoreError, GetFileByPathError};
use std::io;

pub fn print(file_name: &str) -> CliResult<()> {
    account()?;
    let cfg = config()?;

    let file_metadata = get_file_by_path(&cfg, file_name).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(file_name.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    // Temp copy and paste solution. To be reworked
    impl TreeItem for EncryptedFileMetadata {
        type Child = Self;
        fn write_self<W: io::Write>(&self, f: &mut W, style: &Style) -> io::Result<()> {
            write!(self.name())
        }
        fn children(&self) -> Cow<[Self::Child]> {
            Cow::from(vec![])
        }
    }

    print_tree(&file_metadata)?;
}