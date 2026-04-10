use tracing_appender::rolling::RollingFileAppender;
use tracing_appender::rolling::never;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

static LOG_FILE: &str = "lb-metrics.log";

pub fn init() {
    let file = file_logger();

    tracing_subscriber::registry()
        .with(
            fmt::Layer::new()
                .pretty()
                .with_target(false)
                .with_filter(LevelFilter::INFO),
        )
        .with(
            fmt::Layer::new()
                .with_writer(file)
                .with_ansi(false)
                .with_filter(LevelFilter::INFO),
        )
        .init();
}

fn file_logger() -> RollingFileAppender {
    never("/var/log", LOG_FILE)
}
