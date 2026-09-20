use db_rs::View;
use db_rs::config::Config;
use db_rs::log::Log;
use db_rs::views::composite_view::{Composite, Schema};
use std::error::Error;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::path::Path;
use uuid::Uuid;

pub type MigrationResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Old binaries must be stopped before upgrading. Old logs are retained for recovery.
pub fn init_with_migration<S: Schema>(
    root: &Path, schema: &str, copy: impl FnOnce(&mut S) -> MigrationResult<()>,
) -> MigrationResult<Composite<S>> {
    fs::create_dir_all(root)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(format!("{schema}.migration.lock")))?;
    lock.lock()?;

    let destination = root.join(schema);
    if destination.try_exists()? {
        let config = Config::default().log_location(destination);
        if Log::find_latest(&config)?.is_none() {
            return Err("published database directory contains no log".into());
        }
        return Ok(Composite::init(&config)?);
    }

    let staging = root.join(format!("{schema}.migration-{}", Uuid::new_v4()));
    fs::create_dir(&staging)?;
    let config = Config::default().log_location(&staging);
    let mut db = Composite::init(&config)?;
    let tx = db.write_tx()?;
    copy(&mut db.schema)?;
    tx.end_tx(&mut db)?;
    drop(db);
    // Verify the encoded data can be replayed before publishing the new directory.
    drop(Composite::<S>::init(&config)?);
    #[cfg(unix)]
    File::open(&staging)?.sync_all()?;
    fs::rename(&staging, &destination)?;
    #[cfg(unix)]
    File::open(root)?.sync_all()?;
    Ok(Composite::init(&Config::default().log_location(destination))?)
}
