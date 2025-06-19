impl LbClient {
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

        call_rpc(&mut stream, "create_account", Some(args)).await
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&(key.to_string(),api_url.map(|s| s.to_string()))).map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "import_account", Some(args)).await
    }
    
    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let args = bincode::serialize(&account).map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "import_account_private_key_v1", Some(args)).await

    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let private_key_bytes=  private_key.serialize();
        let args = bincode::serialize(&(private_key_bytes, api_url.to_string()))
            .map_err(core_err_unexpected)?;

        call_rpc(&mut stream, "import_account_private_key_v2", Some(args)).await
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;

        let phrase_vec: Vec<String> = phrase.iter().map(|&s| s.to_string()).collect();
        let args = bincode::serialize(&(phrase_vec, api_url.to_string()))
        .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "import_account_phrase", Some(args)).await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn export_account_private_key(&self) -> LbResult<String> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "export_account_private_key", None).await
    }

    pub(crate) async  fn export_account_private_key_v1(&self) -> LbResult<String> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "export_account_private_key_v1", None).await
    }

    pub(crate) async fn export_account_private_key_v2(&self) -> LbResult<String> {
       let mut stream = TcpStream::connect(&self.addr).await
            .map_err(core_err_unexpected)?;
       call_rpc(&mut stream, "export_account_private_key_v2", None).await
    }

    pub async fn export_account_phrase(&self) -> LbResult<String> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "export_account_phrase", None).await
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "export_account_qr", None).await
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        let mut stream = TcpStream::connect(&self.addr)
            .await
            .map_err(core_err_unexpected)?;
        call_rpc(&mut stream, "delete_account", None).await
        
    }
}

use crate::lb_client::LbClient;
use crate::{model::errors::core_err_unexpected, LbResult};
use libsecp256k1::SecretKey;
use tokio::net::TcpStream;
use crate::rpc::call_rpc;
use crate::model::account::Account;