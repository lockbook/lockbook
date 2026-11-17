use crate::Lb;
use crate::io::server::Server;
use crate::model::account::{Account, MAX_USERNAME_LENGTH, SUB_ACCOUNT_SEPARATOR, Username};
use crate::model::api::{
    DeleteSubAccountRequest, ListSubAccountsRequest, NewSubAccountRequest, SubAccountInfo,
};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::meta::Meta;

impl<S: Server> Lb<S> {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_sub_account(&self, label: &str) -> LbResult<Username> {
        let account = self.get_account()?;
        let username =
            format!("{}{SUB_ACCOUNT_SEPARATOR}{}", account.username, label.to_lowercase());
        if username.len() > MAX_USERNAME_LENGTH {
            return Err(LbErrKind::UsernameInvalid.into());
        }

        let counter = self
            .client
            .list_sub_accounts(account, ListSubAccountsRequest {})
            .await?
            .next_counter;
        let sub_account = account.sub_account(username.clone(), counter)?;
        let root_folder = Meta::create_root(&sub_account)?.sign_with(&sub_account)?;

        self.client
            .new_sub_account(
                account,
                NewSubAccountRequest {
                    username: username.clone(),
                    public_key: sub_account.public_key(),
                    root_folder,
                    counter,
                },
            )
            .await?;

        Ok(username)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_sub_accounts(&self) -> LbResult<Vec<SubAccountInfo>> {
        let account = self.get_account()?;
        Ok(self
            .client
            .list_sub_accounts(account, ListSubAccountsRequest {})
            .await?
            .sub_accounts)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete_sub_account(&self, username: &str) -> LbResult<()> {
        let account = self.get_account()?;
        self.client
            .delete_sub_account(
                account,
                DeleteSubAccountRequest { username: username.to_lowercase() },
            )
            .await?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn export_sub_account_private_key(&self, username: &str) -> LbResult<String> {
        let sub_account = self.sub_account(username).await?;
        Ok(base64::encode(sub_account.private_key.serialize()))
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn export_sub_account_phrase(&self, username: &str) -> LbResult<String> {
        let sub_account = self.sub_account(username).await?;
        Ok(sub_account.get_phrase()?.join(" "))
    }

    async fn sub_account(&self, username: &str) -> LbResult<Account> {
        let account = self.get_account()?;
        let username = username.to_lowercase();
        let info = self
            .list_sub_accounts()
            .await?
            .into_iter()
            .find(|sub_account| sub_account.username == username)
            .ok_or(LbErrKind::UsernameNotFound)?;

        let sub_account = account.sub_account(info.username, info.counter)?;
        if sub_account.public_key() != info.public_key {
            return Err(LbErrKind::UsernamePublicKeyMismatch.into());
        }

        Ok(sub_account)
    }
}
