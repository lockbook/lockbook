pub struct ProxyLb {
    pub addr: SocketAddrV4
}

impl ProxyLb {
    pub async fn create_account(
        &self,
        username: &str,
        api_url: &str,
        welcome_doc: bool,
    ) -> LbResult<Account> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(username.to_string(), api_url.to_string(), welcome_doc))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "create_account", args).await
    }

    /* #[instrument(level = "debug", skip(self, key), err(Debug))]
    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        
    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
        
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_account_private_key(&self) -> LbResult<String> {
      
    }

    pub(crate) fn export_account_private_key_v1(&self) -> LbResult<String> {
       
    }

    #[allow(dead_code)]
    pub(crate) fn export_account_private_key_v2(&self) -> LbResult<String> {
       
    }

    pub fn export_account_phrase(&self) -> LbResult<String> {
    }

    pub fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete_account(&self) -> LbResult<()> {
        
    }

    fn welcome_message(username: &str) -> Vec<u8> {
       unimplemented!()
    } */
}

use crate::{model::errors::core_err_unexpected, LbResult};
use std::net::{SocketAddrV4};
use tokio::net::TcpStream;
use crate::rpc::call_rpc;
use crate::model::account::Account;