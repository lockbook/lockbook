//! Process-entry logging for the desktop host.

pub fn install() -> Option<TraceSession> {
    let config = lb::model::core_config::Config::ui_config("egui");
    lb::service::logging::install_default(&config).expect("install lockbook logging");
    None
}

pub struct TraceSession;
