use lazy_static::lazy_static;
use prometheus::{IntGaugeVec, register_int_gauge_vec};

lazy_static! {
    pub static ref INSTALLS: IntGaugeVec = register_int_gauge_vec!(
        "installs",
        "Install/download counts by platform",
        &["platform", "product", "country"]
    )
    .unwrap();
}
