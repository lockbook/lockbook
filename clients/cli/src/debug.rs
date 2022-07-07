use lockbook_core::Core;
use lockbook_core::{FileMetaMapExt, FileMetaVecExt};
use lockbook_core::{TestRepoError, Warning};

use crate::error::CliError;
use crate::selector::select_meta;
use crate::{error, Debug, Uuid};

pub fn debug(core: &Core, debug: Debug) -> Result<(), CliError> {
    use Debug::*;
    match debug {
        Info { path, id } => info(core, path, id),
        Errors => error::print_err_table(),
        WhoAmI => whoami(core),
        WhereAmI => whereami(core),
        Validate => validate(core),
        Tree => tree(core),
    }
}

fn info(core: &Core, path: Option<String>, id: Option<Uuid>) -> Result<(), CliError> {
    let meta = select_meta(core, path, id, None, None)?;
    println!("{:#?}", meta);
    Ok(())
}

fn whoami(core: &Core) -> Result<(), CliError> {
    println!("{}", core.get_account()?.username);
    Ok(())
}

fn whereami(core: &Core) -> Result<(), CliError> {
    let account = core.get_account()?;
    let config = &core.config;
    println!("Server: {}", account.api_url);
    println!("Core: {}", config.writeable_path);
    Ok(())
}

fn validate(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let err = match core.validate() {
        Ok(warnings) => {
            if warnings.is_empty() {
                return Ok(());
            };

            for w in &warnings {
                match w {
                    Warning::EmptyFile(id) => {
                        let path = core.get_path_by_id(*id)?;
                        eprintln!("File at path {} is empty.", path);
                    }
                    Warning::InvalidUTF8(id) => {
                        let path = core.get_path_by_id(*id)?;
                        eprintln!("File at path {} contains invalid UTF8.", path);
                    }
                    Warning::UnreadableDrawing(id) => {
                        let path = core.get_path_by_id(*id)?;
                        eprintln!("Drawing at path {} is unreadable.", path);
                    }
                }
            }

            CliError::validate_warnings_found(warnings.len())
        }
        Err(err) => match err {
            TestRepoError::NoAccount => CliError::no_account(),
            TestRepoError::NoRootFolder => CliError::no_root(),
            TestRepoError::DocumentTreatedAsFolder(id) => {
                CliError::doc_treated_as_dir(core.get_path_by_id(id)?)
            }
            TestRepoError::FileOrphaned(id) => CliError::file_orphaned(core.get_path_by_id(id)?),
            TestRepoError::CycleDetected(_) => CliError::cycle_detected(),
            TestRepoError::FileNameEmpty(_) => CliError::file_name_empty(),
            TestRepoError::FileNameContainsSlash(id) => {
                CliError::file_name_has_slash(core.get_path_by_id(id)?)
            }
            TestRepoError::NameConflictDetected(id) => {
                CliError::name_conflict_detected(core.get_path_by_id(id)?)
            }
            TestRepoError::DocumentReadError(id, err) => {
                CliError::validate_doc_read(core.get_path_by_id(id)?, format!("{:#?}", err))
            }
            TestRepoError::Core(err) => {
                CliError::unexpected(format!("unexpected error: {:#?}", err))
            }
            TestRepoError::Tree(err) => {
                CliError::unexpected(format!("unexpected error: {:#?}", err))
            }
        },
    };

    Err(err)
}

fn tree(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let files = core
        .list_metadatas()
        .map_err(|err| CliError::unexpected(format!("{}", err)))?;

    println!("{}", files.to_map().pretty_print());

    Ok(())
}
