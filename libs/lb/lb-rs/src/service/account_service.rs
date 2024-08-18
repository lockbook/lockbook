use crate::model::errors::core_err_unexpected;
use crate::service::api_service::ApiError;
use crate::shared::account::{Account, MAX_USERNAME_LENGTH};
use crate::shared::api::{DeleteAccountRequest, GetPublicKeyRequest, NewAccountRequest};
use crate::shared::document_repo::DocumentService;
use crate::shared::file_like::FileLike;
use crate::shared::file_metadata::{FileMetadata, FileType};
use crate::{CoreError, CoreState, LbResult, Requester};
use libsecp256k1::PublicKey;
use qrcode_generator::QrCodeEcc;

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn create_account(
        &mut self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let username = String::from(username).to_lowercase();

        if username.len() > MAX_USERNAME_LENGTH {
            return Err(CoreError::UsernameInvalid.into());
        }

        if self.db.account.get().is_some() {
            return Err(CoreError::AccountExists.into());
        }

        let account = Account::new(username.clone(), api_url.to_string());
        self.public_key = Some(account.public_key());

        let root = FileMetadata::create_root(&account)?.sign(&account)?;
        let root_id = *root.id();

        let last_synced = self
            .client
            .request(&account, NewAccountRequest::new(&account, &root))?
            .last_synced;

        self.db.account.insert(account.clone())?;
        self.db.base_metadata.insert(root_id, root)?;
        self.db.last_synced.insert(last_synced as i64)?;
        self.db.root.insert(root_id)?;

        if welcome_doc {
            let welcome_doc = self.create_file("welcome.md", &root_id, FileType::Document)?;
            self.write_document(welcome_doc.id, &Self::welcome_message(&username))?;
        }

        Ok(account)
    }

    pub(crate) fn import_account(&mut self, account_string: &str) -> LbResult<Account> {
        if self.db.account.get().is_some() {
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
            .request(&account, GetPublicKeyRequest { username: account.username.clone() })?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(CoreError::UsernamePublicKeyMismatch.into());
        }

        self.public_key = Some(account_public_key);
        self.db.account.insert(account.clone())?;

        Ok(account)
    }

    pub(crate) fn export_account(&self) -> LbResult<String> {
        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;
        let encoded: Vec<u8> = bincode::serialize(&account).map_err(core_err_unexpected)?;
        Ok(base64::encode(encoded))
    }

    pub(crate) fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let acct_secret = self.export_account()?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(|err| core_err_unexpected(err).into())
    }

    pub(crate) fn get_account(&self) -> LbResult<&Account> {
        self.db
            .account
            .get()
            .ok_or_else(|| CoreError::AccountNonexistent.into())
    }

    pub(crate) fn get_public_key(&mut self) -> LbResult<PublicKey> {
        match self.public_key {
            Some(pk) => Ok(pk),
            None => {
                let account = self.get_account()?;
                let pk = account.public_key();
                self.public_key = Some(pk);
                Ok(pk)
            }
        }
    }

    pub(crate) fn delete_account(&mut self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, DeleteAccountRequest {})
            .map_err(|err| match err {
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        self.db.account.clear()?;
        self.db.last_synced.clear()?;
        self.db.base_metadata.clear()?;
        self.db.root.clear()?;
        self.db.local_metadata.clear()?;
        self.db.pub_key_lookup.clear()?;

        self.public_key = None;

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
