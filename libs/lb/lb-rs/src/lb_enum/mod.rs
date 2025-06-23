pub mod account;
pub mod activity;
pub mod admin;
pub mod billing;
pub mod debug;
pub mod documents;
pub mod events;
pub mod file;
pub mod import_export;
pub mod integrity;
pub mod keychain;
pub mod path;
pub mod share;

pub enum Lb {
    Direct(LbServer),
    Network(LbClient)
}

impl Lb {
    pub async fn init(config: Config) -> LbResult<Self> {
        let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

        match TcpListener::bind(socket).await {
            Ok(listener) => {
                let inner_lb = LbServer::init(config).await?;
                Ok(Lb::Direct(inner_lb))
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                Ok(Lb::Network(LbClient{addr: socket}))
            }
            Err(error) => Err(LbErrKind::Unexpected(format!("Failed to bind: {error}")).into())
        }
    }
}

use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::TcpListener;    
use crate::model::core_config::Config;
use crate::{LbErrKind, LbResult};
use crate::lb_server::LbServer;
use crate::lb_client::LbClient;