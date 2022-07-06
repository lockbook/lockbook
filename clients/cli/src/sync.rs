use indicatif::{ProgressBar, ProgressStyle};
use lockbook_core::ClientWorkUnit;
use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::SyncAllError;
use lockbook_core::SyncProgress;
use std::rc::Rc;

use crate::error::CliError;

pub fn sync(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let pb = setup_progress();
    core.sync(progress_closure(pb.clone()))
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                SyncAllError::ClientUpdateRequired => CliError::update_required(),
                SyncAllError::CouldNotReachServer => CliError::network_issue(),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    pb.finish_with_message("Sync complete.");

    Ok(())
}

fn setup_progress() -> Rc<ProgressBar> {
    let pb = Rc::new(ProgressBar::new(5));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed}] [{bar:.green/green}] {pos}/{len} ({eta}) {msg}")
            .progress_chars("#>-"),
    );
    pb
}

fn progress_closure(pb: Rc<ProgressBar>) -> Option<Box<dyn Fn(SyncProgress)>> {
    let closure = move |sync_progress: SyncProgress| {
        pb.set_length(sync_progress.total as u64);
        pb.set_position(sync_progress.progress as u64);
        match sync_progress.current_work_unit {
            ClientWorkUnit::PullMetadata => pb.set_message("Pulling file tree updates"),
            ClientWorkUnit::PushMetadata => pb.set_message("Pushing file tree updates"),
            ClientWorkUnit::PullDocument(name) => pb.set_message(format!("Pulling: {}", name)),
            ClientWorkUnit::PushDocument(name) => pb.set_message(format!("Pushing: {}", name)),
        };
    };

    Some(Box::new(closure))
}
