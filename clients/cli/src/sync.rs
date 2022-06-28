use lockbook_core::ClientWorkUnit;
use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::SyncAllError;
use lockbook_core::SyncProgress;

use crate::error::CliError;

pub fn sync(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let closure = |sync_progress: SyncProgress| {
        match sync_progress.current_work_unit {
            ClientWorkUnit::PullMetadata => println!("Pulling file tree updates"),
            ClientWorkUnit::PushMetadata => println!("Pushing file tree updates"),
            ClientWorkUnit::PullDocument(name) => println!("Pulling: {}", name),
            ClientWorkUnit::PushDocument(name) => println!("Pushing: {}", name),
        };
    };

    core.sync(Some(Box::new(closure)))
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                SyncAllError::ClientUpdateRequired => CliError::update_required(),
                SyncAllError::CouldNotReachServer => CliError::network_issue(),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    println!("Sync complete.");

    Ok(())
}
