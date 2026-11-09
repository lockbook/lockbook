pub use ::test_utils::{byte_source, prop, shrink};
pub mod files;
#[cfg(not(target_family = "wasm"))]
pub mod ws;

/// A core in memory, on no server. Each starts its own runtime, so property
/// tests should build one and reuse it.
pub fn in_memory_core() -> lb_rs::blocking::Lb {
    let config = lb_rs::model::core_config::Config {
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
        client_name: "tests".into(),
        build_info: None,
        client_type: lb_rs::model::core_config::ClientType::Unknown,
        ..Default::default()
    };
    let network =
        lb_rs::io::network::Network { client_type: config.client_type, ..Default::default() };
    lb_rs::blocking::Lb::in_memory(config, network).unwrap()
}
