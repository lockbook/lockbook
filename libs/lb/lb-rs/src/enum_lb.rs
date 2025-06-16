pub enum Lb {
    ActualLb(InnerLb),
    ExposedLb(ProxyLb)
}

impl Lb {
    pub async fn init(config: Config) -> LbResult<Self> {
        let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

        match TcpListener::bind(socket).await {
            Ok(listener) => {
                let inner_lb = InnerLb::init(config).await?;
                Ok(Lb::ActualLb(inner_lb))
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                Ok(Lb::ExposedLb(ProxyLb{addr: socket}))
            }
            Err(error) => Err(LbErrKind::Unexpected(format!("Failed to bind: {error}")).into())
        }
    }
}

impl Lb {

}

use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::TcpListener;    
use crate::model::core_config::Config;
use crate::{LbErrKind, LbResult};
use crate::inner_lb::InnerLb;
use crate::proxy_lb::ProxyLb;