use crate::ignore::IgnoreRules;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

pub struct FsWatcher {
    rx: mpsc::Receiver<Vec<PathBuf>>,
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

impl FsWatcher {
    /// Create a new filesystem watcher on the given root directory.
    pub fn new(root: &Path, _ignore: &IgnoreRules) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel(64);
        let root_owned = root.to_path_buf();

        // Paths we recently wrote ourselves — suppress self-triggered events
        let self_writes: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        // Clone ignore rules context for the callback
        let root_for_filter = root_owned.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                let events = match result {
                    Ok(events) => events,
                    Err(errs) => {
                        for e in errs {
                            tracing::warn!("watcher error: {e}");
                        }
                        return;
                    }
                };

                let mut changed = Vec::new();
                let self_writes_guard = self_writes.lock().unwrap();

                for event in events {
                    for path in &event.paths {
                        if self_writes_guard.contains(path) {
                            continue;
                        }
                        // We can't call ignore.is_ignored here without ownership issues,
                        // so we filter in the consumer side. Just pass all paths through.
                        if path.starts_with(&root_for_filter) {
                            changed.push(path.clone());
                        }
                    }
                }

                drop(self_writes_guard);

                if !changed.is_empty() {
                    changed.sort();
                    changed.dedup();
                    let _ = tx.blocking_send(changed);
                }
            },
        )?;

        debouncer
            .watch(root, notify::RecursiveMode::Recursive)?;

        Ok(Self { rx, _debouncer: debouncer })
    }

    /// Wait for the next batch of changed paths.
    pub async fn next_batch(&mut self) -> Option<Vec<PathBuf>> {
        self.rx.recv().await
    }
}
