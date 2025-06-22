pub mod account;
pub mod activity;
pub mod admin;
pub struct LbClient {
    pub addr: SocketAddrV4
}

use std::net::SocketAddrV4;
