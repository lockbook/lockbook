use crate::model::account::{Account, MAX_USERNAME_LENGTH};
use crate::model::api::{
    DeleteAccountRequest, GetPublicKeyRequest, GetUsernameRequest, NewAccountRequestV2,
};
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::meta::Meta;
use crate::{DEFAULT_API_LOCATION, Lb};
use libsecp256k1::SecretKey;
use qrcode_generator::QrCodeEcc;

use crate::io::network::ApiError;

impl Lb {
    /// CoreError::AccountExists,
    /// CoreError::UsernameTaken,
    /// CoreError::UsernameInvalid,
    /// CoreError::ServerDisabled,
    /// CoreError::ServerUnreachable,
    /// CoreError::ClientUpdateRequired,
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let username = String::from(username).to_lowercase();

        if username.len() > MAX_USERNAME_LENGTH {
            return Err(LbErrKind::UsernameInvalid.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if db.account.get().is_some() {
            return Err(LbErrKind::AccountExists.into());
        }

        let account = Account::new(username.clone(), api_url.to_string());

        let root = Meta::create_root(&account)?.sign_with(&account)?;
        let root_id = *root.id();

        let last_synced = self
            .client
            .request(&account, NewAccountRequestV2::new(&account, &root))
            .await?
            .last_synced;

        db.account.insert(account.clone())?;
        db.base_metadata.insert(root_id, root)?;
        db.last_synced.insert(last_synced as i64)?;
        db.root.insert(root_id)?;
        db.pub_key_lookup
            .insert(Owner(account.public_key()), account.username.clone())?;

        self.keychain.cache_account(account.clone()).await?;

        tx.end();

        self.events.meta_changed();

        if welcome_doc {
            let welcome_doc = self
                .create_file("welcome.md", &root_id, FileType::Document)
                .await?;
            self.write_document(welcome_doc.id, &Self::welcome_message(&username))
                .await?;
            self.sync(None).await?;
        }

        Ok(account)
    }

    #[instrument(level = "debug", skip(self, key), err(Debug))]
    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        if self.get_account().is_ok() {
            warn!("tried to import an account, but account exists already.");
            return Err(LbErrKind::AccountExists.into());
        }

        if let Ok(key) = base64::decode(key) {
            if let Ok(account) = bincode::deserialize(&key[..]) {
                return self.import_account_private_key_v1(account).await;
            } else if let Ok(key) = SecretKey::parse_slice(&key) {
                return self
                    .import_account_private_key_v2(key, api_url.unwrap_or(DEFAULT_API_LOCATION))
                    .await;
            }
        }

        let phrase: [&str; 24] = key
            .split([' ', ','])
            .filter(|maybe_word| !maybe_word.is_empty())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| LbErrKind::AccountStringCorrupted)?;

        self.import_account_phrase(phrase, api_url.unwrap_or(DEFAULT_API_LOCATION))
            .await
    }

    pub async fn import_account_private_key_v1(&self, account: Account) -> LbResult<Account> {
        let server_public_key = self
            .client
            .request(&account, GetPublicKeyRequest { username: account.username.clone() })
            .await?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(LbErrKind::UsernamePublicKeyMismatch.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.insert(account.clone())?;
        self.keychain.cache_account(account.clone()).await?;

        Ok(account)
    }

    pub async fn import_account_private_key_v2(
        &self, private_key: SecretKey, api_url: &str,
    ) -> LbResult<Account> {
        let mut account =
            Account { username: "".to_string(), api_url: api_url.to_string(), private_key };
        let public_key = account.public_key();

        account.username = self
            .client
            .request(&account, GetUsernameRequest { key: public_key })
            .await?
            .username;

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.insert(account.clone())?;
        self.keychain.cache_account(account.clone()).await?;

        Ok(account)
    }

    pub async fn import_account_phrase(
        &self, phrase: [&str; 24], api_url: &str,
    ) -> LbResult<Account> {
        let private_key = Account::phrase_to_private_key(phrase)?;
        self.import_account_private_key_v2(private_key, api_url)
            .await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_account_private_key(&self) -> LbResult<String> {
        self.export_account_private_key_v1()
    }

    pub(crate) fn export_account_private_key_v1(&self) -> LbResult<String> {
        let account = self.get_account()?;
        let encoded: Vec<u8> = bincode::serialize(account).map_err(core_err_unexpected)?;
        Ok(base64::encode(encoded))
    }

    #[allow(dead_code)]
    pub(crate) fn export_account_private_key_v2(&self) -> LbResult<String> {
        let account = self.get_account()?;
        Ok(base64::encode(account.private_key.serialize()))
    }

    pub fn export_account_phrase(&self) -> LbResult<String> {
        let account = self.get_account()?;
        Ok(account.get_phrase()?.join(" "))
    }

    pub fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let acct_secret = self.export_account_private_key_v1()?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(|err| core_err_unexpected(err).into())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete_account(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, DeleteAccountRequest {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
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
