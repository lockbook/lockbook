mod history;
mod queries;
mod source;
mod ui;

use db_rs::View;
use db_rs::config::Config;
use db_rs::views::option::DbOption;
use history::History;
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Error>;

const CACHE_DIR: &str = "view-v1";

fn private_directory(path: &Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

fn read_cache(directory: &Path) -> Result<Option<History>> {
    let directory = directory.join(CACHE_DIR);
    if !directory.join("db.0.log").try_exists()? {
        return Ok(None);
    }
    let mut db = DbOption::<History>::init(&Config::default().log_location(directory))?;
    Ok(db.take()?)
}

fn write_cache(directory: &Path, history: History) -> Result<History> {
    let view = directory.join(CACHE_DIR);
    std::fs::create_dir_all(&view)?;
    let staging = tempfile::tempdir_in(directory)?;
    let mut db = DbOption::init(&Config::default().log_location(staging.path().to_path_buf()))?;
    let tx = db.write_tx()?;
    db.replace(history)?;
    tx.end_tx(&mut db)?;
    let history = db
        .take()?
        .ok_or("metrics cache lost the imported history")?;
    drop(db);
    let mut published = tempfile::NamedTempFile::new_in(&view)?;
    std::io::copy(&mut std::fs::File::open(staging.path().join("db.0.log"))?, &mut published)?;
    published.as_file().sync_all()?;
    published.persist(view.join("db.0.log"))?;
    Ok(history)
}

fn format_bytes(bytes: u64) -> String {
    for (unit, name) in [(1 << 40, "TiB"), (1 << 30, "GiB"), (1 << 20, "MiB"), (1 << 10, "KiB")] {
        if bytes >= unit {
            return format!("{:.1} {name}", bytes as f64 / unit as f64);
        }
    }
    format!("{bytes} B")
}

pub fn run(logs: Option<PathBuf>, remote: String) -> Result<()> {
    let directory =
        PathBuf::from(lb::model::core_config::Config::writeable_path("admin")).join("metrics");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 780.0])
            .with_min_inner_size([560.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Lockbook Metrics",
        options,
        Box::new(move |context| {
            let mut fonts = eframe::egui::FontDefinitions::default();
            workspace_rs::register_fonts(&mut fonts);
            context.egui_ctx.set_fonts(fonts);
            workspace_rs::theme::visuals::init(&context.egui_ctx);
            Ok(Box::new(ui::Metrics::new(directory, remote, logs, &context.egui_ctx)))
        }),
    )
    .map_err(|error| error.to_string().into())
}
