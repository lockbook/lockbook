use tracing_subscriber::{filter, fmt, prelude::*, Layer};

pub fn init() {
    let subscriber = tracing_subscriber::Registry::default()
        // Logger for stdout (local development)
        .with(
            fmt::Layer::new()
                .pretty()
                .with_target(false)
                .with_filter(filter::filter_fn(|metadata| metadata.target().starts_with("drive"))),
        );
    // Logger for disaster response (any error logs sent to pagerduty)

    tracing::subscriber::set_global_default(subscriber).unwrap();
}
