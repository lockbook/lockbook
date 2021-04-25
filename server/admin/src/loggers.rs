use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use log::{Level, Log, Metadata, Record};

// TODO add colors here?
// TODO understand stderr behavior
pub fn init() -> Dispatch {
    fern::Dispatch::new()
        .format(move |out, message, record| out.finish(message.clone()))
        .chain(std::io::stdout())
        .level(log::LevelFilter::Warn)
}
