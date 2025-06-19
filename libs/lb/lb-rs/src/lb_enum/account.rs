impl Lb {
    pub async fn create_account(
        &mut self,
        username: &str,
        api_url: &str,
        welcome_doc: bool,
    ) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => {
                inner.create_account(username, api_url, welcome_doc).await
            }
            Lb::Network(proxy) => {
                proxy.create_account(username, api_url, welcome_doc).await
            }
        }
    }

    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        
        match self {
            Lb::Direct(inner) => {
                inner.import_account(key, api_url).await
            }
            Lb::Network(proxy) => {
                proxy.import_account(key, api_url).await
            }
        }
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        match self {
            Lb::Direct(inner) => {
                inner.import_account_private_key_v1(account).await
            }
            Lb::Network(proxy) => {
                proxy.import_account_private_key_v1(account).await
            }
        }
    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
         match self {
            Lb::Direct(inner) => {
                inner.import_account_private_key_v2(private_key,api_url).await
            }
            Lb::Network(proxy) => {
                proxy.import_account_private_key_v2(private_key, api_url).await
            }
        }
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account>{
        match self {
            Lb::Direct(inner) => {
                inner.import_account_phrase(phrase,api_url).await
            }
            Lb::Network(proxy) => {
                proxy.import_account_phrase(phrase,api_url).await
            }
        }
    }

    pub async fn export_account_private_key(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => {
                inner.export_account_private_key()
            }
            Lb::Network(proxy) => {
                proxy.export_account_private_key().await
            }
        }
    }

    pub(crate) async fn export_account_private_key_v1(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => {
                inner.export_account_private_key_v1()
            }
            Lb::Network(proxy) => {
                proxy.export_account_private_key_v1().await
            }
        }
    }

    pub(crate) async  fn export_account_private_key_v2(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => {
                inner.export_account_private_key_v2()
            }
            Lb::Network(proxy) => {
                proxy.export_account_private_key_v2().await
            }
        }
    }

    pub async fn export_account_phrase(&self) -> LbResult<String> {
        match self {
            Lb::Direct(inner) => {
                inner.export_account_phrase()
            }
            Lb::Network(proxy) => {
                proxy.export_account_phrase().await
            }
        }
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        match self {
            Lb::Direct(inner) => {
                inner.export_account_qr()
            }
            Lb::Network(proxy) => {
                proxy.export_account_qr().await
            }
        }
    }

    pub async fn delete_account(&self) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner.delete_account().await
            }
            Lb::Network(proxy) => {
                proxy.delete_account().await
            }
        }
    }
}

use libsecp256k1::SecretKey;
use crate::{Lb, LbResult};
use crate::model::account::Account;