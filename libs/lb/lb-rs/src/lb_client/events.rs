impl LbClient {
    pub async fn subscribe(&self) -> Receiver<Event> {
       
    }
}

use crate::lb_client::LbClient;
use crate::service::events::Event;
use crate::{model::errors::core_err_unexpected};
use tokio::net::TcpStream;
use tokio::sync::broadcast::Receiver;
use crate::rpc::call_rpc;