use crate::logic::account::{Account, MAX_USERNAME_LENGTH};
use crate::logic::api::{DeleteAccountRequest, GetPublicKeyRequest, NewAccountRequest};
use crate::logic::file_like::FileLike;
use crate::logic::file_metadata::{FileMetadata, FileType};
use crate::model::errors::{core_err_unexpected, CoreError, LbResult};
use crate::Lb;
use qrcode_generator::QrCodeEcc;

use super::network::ApiError;

impl Lb {
    /// CoreError::AccountExists,
    /// CoreError::UsernameTaken,
    /// CoreError::UsernameInvalid,
    /// CoreError::ServerDisabled,
    /// CoreError::ServerUnreachable,
    /// CoreError::ClientUpdateRequired,
    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let username = String::from(username).to_lowercase();

        if username.len() > MAX_USERNAME_LENGTH {
            return Err(CoreError::UsernameInvalid.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if db.account.get().is_some() {
            return Err(CoreError::AccountExists.into());
        }

        let account = Account::new(username.clone(), api_url.to_string());
        self.cache_account(account.clone()).await;

        let root = FileMetadata::create_root(&account)?.sign(&account)?;
        let root_id = *root.id();

        let last_synced = self
            .client
            .request(&account, NewAccountRequest::new(&account, &root))
            .await?
            .last_synced;

        db.account.insert(account.clone())?;
        db.base_metadata.insert(root_id, root)?;
        db.last_synced.insert(last_synced as i64)?;
        db.root.insert(root_id)?;
        tx.end();

        let bg_lb = self.clone();
        tokio::spawn(async move {
            if welcome_doc {
                let welcome_doc = bg_lb
                    .create_file("welcome.md", &root_id, FileType::Document)
                    .await
                    .unwrap();
                bg_lb
                    .write_document(welcome_doc.id, &Self::welcome_message(&username))
                    .await
                    .unwrap();
                //sync
            }
        });

        Ok(account)
    }

    pub async fn import_account(&mut self, account_string: &str) -> LbResult<Account> {
        if self.get_account().is_ok() {
            warn!("tried to import an account, but account exists already.");
            return Err(CoreError::AccountExists.into());
        }

        let decoded = match base64::decode(account_string) {
            Ok(d) => d,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted.into());
            }
        };

        let account: Account = match bincode::deserialize(&decoded[..]) {
            Ok(a) => a,
            Err(_) => {
                return Err(CoreError::AccountStringCorrupted.into());
            }
        };

        let server_public_key = self
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() })
            .await?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.insert(account.clone())?;
        self.cache_account(account.clone()).await;

        Ok(account)
    }

    pub async fn export_account(&self) -> LbResult<String> {
        let account = self.get_account()?;
        let encoded: Vec<u8> = bincode::serialize(account).map_err(core_err_unexpected)?;
        Ok(base64::encode(encoded))
    }

    pub async fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let acct_secret = self.export_account().await?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(|err| core_err_unexpected(err).into())
    }

    pub async fn delete_account(&mut self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, DeleteAccountRequest {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        db.account.clear()?;
        db.last_synced.clear()?;
        db.base_metadata.clear()?;
        db.root.clear()?;
        db.local_metadata.clear()?;
        db.pub_key_lookup.clear()?;

        // todo: clear cache?

        Ok(())
    }

    fn welcome_message(username: &str) -> Vec<u8> {
        format!(r#"# Hello {username}

Welcome to Lockbook! This is an example note to help you get started with our note editor. You can keep it to use as a cheat sheet or delete it anytime.

Lockbook uses Markdown, a lightweight language for formatting plain text. You can use all our supported formatting just by typing. Here’s how it works:

# This is a heading

## This is a smaller heading

### This is an even smaller heading

###### Headings have 6 levels

For italic, use single *asterisks* or _underscores_.

For bold, use double **asterisks** or __underscores__.

For inline code, use single `backticks`

For code blocks, use
```
triple
backticks
```

>For block quotes,
use a greater-than sign

Bulleted list items
* start
* with
* asterisks
- or
- hyphens
+ or
+ plus
+ signs

Numbered list items
1. start
2. with
3. numbers
4. and
5. periods

Happy note taking! You can report any issues to our [Github project](https://github.com/lockbook/lockbook/issues/new) or join our [Discord server](https://discord.gg/qv9fmAZCm6)."#).into()
    }
}
