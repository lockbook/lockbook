use std::path::Path;

use db_rs::View;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task;

use crate::model::errors::Unexpected;
use crate::{Lb, LbResult};

impl Lb {
    pub(crate) async fn setup_ipc(&self) -> LbResult<()> {
        if !self.config.background_work {
            return Ok(());
        }

        let directory = self.db.read().await.log().directory.canonicalize()?;
        let watched_directory = directory.clone();
        let (wake, mut wakes) = mpsc::channel(1);
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let changed = match event {
                Ok(event) => is_log_event(&event, &watched_directory),
                Err(error) => {
                    warn!(?error, "database watcher failed; requesting catch-up");
                    true
                }
            };
            if changed {
                let _ = wake.try_send(());
            }
        })
        .map_unexpected()?;
        watcher
            .watch(&directory, RecursiveMode::NonRecursive)
            .map_unexpected()?;

        // Register first so a commit between initialization and watching isn't missed.
        self.begin_tx().await.end();

        let core = self.clone();
        let runtime = Handle::current();
        tokio::spawn(async move {
            let _watcher = watcher;
            while wakes.recv().await.is_some() {
                let core = core.clone();
                let runtime = runtime.clone();
                // Waiting for another process's database lock must not block a Tokio worker.
                let catch_up = task::spawn_blocking(move || {
                    runtime.block_on(async { core.begin_tx().await.end() });
                });
                if let Err(error) = catch_up.await {
                    error!(?error, "database catch-up worker failed");
                    break;
                }
            }
        });

        Ok(())
    }
}

fn is_log_event(event: &Event, directory: &Path) -> bool {
    event.need_rescan()
        || (!matches!(
            event.kind,
            EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
        ) && (event.paths.is_empty()
            || event.paths.iter().any(|path| {
                path == directory || path.extension().is_some_and(|extension| extension == "log")
            })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, Flag, MetadataKind, RenameMode};

    #[test]
    fn ignores_reads_and_lock_files() {
        let directory = Path::new("CoreV5");
        let log = directory.join("db.0.log");
        for event in [
            Event::new(EventKind::Access(AccessKind::Any)).add_path(log.clone()),
            Event::new(EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)))
                .add_path(log),
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any)))
                .add_path(directory.join("db.lock")),
        ] {
            assert!(!is_log_event(&event, directory));
        }
    }

    #[test]
    fn detects_appends_snapshots_and_rescans() {
        let directory = Path::new("CoreV5");
        let log = directory.join("db.1.log");
        for event in [
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any))).add_path(log.clone()),
            Event::new(EventKind::Create(CreateKind::File)).add_path(log.clone()),
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                .add_path(directory.join("db.1.tmp"))
                .add_path(log),
            Event::new(EventKind::Other).set_flag(Flag::Rescan),
        ] {
            assert!(is_log_event(&event, directory));
        }
    }
}
