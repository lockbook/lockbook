use std::backtrace::Backtrace;
use std::fmt::{Debug, Write};
use std::{env, panic};

use serde::Serialize;

use tokio::runtime::Handle;

use sha2::{Digest, Sha256};

use tracing::field::{Field, Visit};
use tracing::metadata::LevelFilter;
use tracing::{Event, Subscriber};
use tracing_appender::rolling::RollingFileAppender;
use tracing_gcp::GcpLayer;
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, filter, fmt};

use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::CARGO_PKG_VERSION;
use crate::config::Config;

static LOG_FILE: &str = "lockbook_server.log";

pub fn init(config: &Config) {
    let log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| s.as_str().parse().ok())
        .unwrap_or(LevelFilter::DEBUG);
    let subscriber = tracing_subscriber::Registry::default()
        // Logger for stdout (local development)
        .with(
            fmt::Layer::new()
                .pretty()
                .with_target(false)
                .with_filter(log_level)
                .with_filter(server_logs()),
        )
        // Writes to the specified file in a format that gcp understands
        .with(
            GcpLayer::init_with_writer(file_logger(config))
                .with_filter(LevelFilter::DEBUG)
                .with_filter(server_logs()),
        )
        // Logger for disaster response (any error logs sent to pagerduty)
        .with(
            PDLogger::new(config)
                .with_filter(LevelFilter::ERROR)
                .with_filter(server_logs()),
        );

    tracing::subscriber::set_global_default(subscriber).unwrap();
    panic_hook();
}

fn file_logger(config: &Config) -> RollingFileAppender {
    tracing_appender::rolling::never(&config.server.log_path, LOG_FILE)
}

fn server_logs() -> FilterFn {
    filter::filter_fn(|metadata| {
        metadata.target().starts_with("lockbook")
            || metadata.target().starts_with("dbrs")
            || metadata.target().starts_with("lb_rs")
    })
}

struct PDLogger {
    config: Config,
    handle: Handle,
}

impl<S: Subscriber> Layer<S> for PDLogger {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        self.page(AlertDetails::new(event));
    }
}

impl PDLogger {
    fn new(config: &Config) -> Self {
        let handle = Handle::current();
        let config = config.clone();
        Self { config, handle }
    }

    fn page(&self, details: AlertDetails) {
        let env = self.config.server.env.to_string();
        match &self.config.server.pd_api_key {
            Some(api_key) => send_to_pagerduty(&self.handle, env, api_key, details),
            None => eprintln!("WOULD PAGE: {}", details.message),
        }
    }
}

const PAGERDUTY_EVENTS: &str = "https://events.pagerduty.com/v2/enqueue";

fn now() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap()
}

async fn post_pagerduty(url: &str, body: serde_json::Value) {
    let result = reqwest::Client::new()
        .post(url)
        .header(reqwest::header::CONTENT_ENCODING, "identity")
        .header(reqwest::header::USER_AGENT, "lockbook-server")
        .json(&body)
        .send()
        .await;
    match result {
        Ok(response) if response.status() == reqwest::StatusCode::ACCEPTED => {}
        Ok(response) => {
            eprintln!("Failed reporting event to PagerDuty! status {}", response.status())
        }
        Err(err) => eprintln!("Failed reporting event to PagerDuty! {err}"),
    }
}

fn send_to_pagerduty(handle: &Handle, env: String, api_key: &str, alert: AlertDetails) {
    let message = alert.message.clone();
    let body = json!({
        "routing_key": api_key,
        "event_action": "trigger",
        "dedup_key": dedup_key(&message),
        "payload": {
            "summary": message,
            "source": env,
            "severity": "error",
            "timestamp": now(),
            "custom_details": alert,
        },
    });

    // https://github.com/neonphog/tokio_safe_block_on/blob/074d40929ccab649b0dcc83a4ebdbdcb70b317fb/src/lib.rs#L72-L86
    tokio::task::block_in_place(move || {
        futures::executor::block_on(async {
            handle
                .spawn(post_pagerduty(PAGERDUTY_EVENTS, body))
                .await
                .err()
                .map(|err| eprintln!("Failed spawning task in Tokio runtime! {err}"))
        })
    });
}

pub fn log_startup(config: &Config, url: &str) {
    tracing::warn!("{}", startup_message(config, url));
}

fn startup_message(config: &Config, url: &str) -> String {
    let commit = lb_rs::get_code_commit().unwrap_or("unknown");
    let server = &config.server;
    format!(
        "lockbook-server {CARGO_PKG_VERSION} ({commit}) started on {url}, env: {}, min core version: {}, max auth delay: {}ms, db: {}, files: {}, logs: {}, metrics: {}, admins: {}, features: {:?}",
        server.env,
        server.min_core_version,
        server.max_auth_delay,
        config.index_db.db_location,
        config.files.path.display(),
        server.log_path,
        config.metrics.addr,
        config.admin.admins.len(),
        config.features,
    )
}

#[derive(Serialize, Default, Clone)]
struct AlertDetails {
    message: String,
    logger: String,
    file: Option<String>,
    line: Option<String>,
    build: String,
}

impl AlertDetails {
    fn new(event: &Event) -> Self {
        let mut details = Self::default();
        let record = event.metadata();

        // Populate the message field
        event.record(&mut details);

        // Populate the other fields
        details.logger = record.target().to_string();
        details.file = record.file().map(|file| file.to_string());
        details.line = record.line().map(|line| line.to_string());
        details.build = CARGO_PKG_VERSION.to_string();

        details
    }
}

impl Visit for AlertDetails {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            write!(self.message, "{value:?}").unwrap();
        }
    }
}

fn dedup_key(record: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(record);
    let result = hasher.finalize();
    base64::encode(result)
}

fn panic_hook() {
    panic::set_hook(Box::new(move |panic_info| {
        let bt = Backtrace::force_capture();
        tracing::error!("panic detected: {panic_info} {}", bt);
    }));
}
