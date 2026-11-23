use crate::experiments::{WelcomeDoc, cohort};
use crate::io::docs::LocalDocuments;
use crate::io::server::Server;
use crate::model::account::{Account, MAX_USERNAME_LENGTH};
use crate::model::api::{
    DeleteAccountRequest, GetPublicKeyRequest, GetUsernameRequest, NewAccountRequestV2,
};
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::meta::Meta;
use crate::service::events::Actor;
use crate::{DEFAULT_API_LOCATION, Lb};
use libsecp256k1::SecretKey;
use qrcode_generator::QrCodeEcc;

use crate::io::network::ApiError;

/// A short introduction to writing in Markdown: the note one welcome cohort
/// starts with, and the sample note a client can offer in an empty vault.
pub const MARKDOWN_WELCOME: &str = include_str!("samples/markdown_welcome.md");

impl<S: Server, D: LocalDocuments> Lb<S, D> {
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
        self.create_account_with_key(username, api_url, self.entropy.secret_key(), welcome_doc)
            .await
    }

    /// [Self::create_account] with the caller's key.
    #[instrument(level = "debug", skip(self, key), err(Debug))]
    pub async fn create_account_with_key(
        &self, username: &str, api_url: &str, key: SecretKey, welcome_doc: bool,
    ) -> LbResult<Account> {
        let username = String::from(username).to_lowercase();

        if username.len() > MAX_USERNAME_LENGTH {
            return Err(LbErrKind::UsernameInvalid.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if db.account.as_ref().is_some() {
            return Err(LbErrKind::AccountExists.into());
        }

        let account =
            Account { username: username.clone(), api_url: api_url.to_string(), private_key: key };

        let root = Meta::create_root(&account, &self.entropy)?.sign_with(&account)?;
        let root_id = *root.id();

        let last_synced = self
            .client
            .new_account_v2(&account, NewAccountRequestV2::new(&account, &root))
            .await?
            .last_synced;

        db.account.replace(account.clone())?;
        db.base_metadata.insert(root_id, root)?;
        db.last_synced.replace(last_synced as i64)?;
        db.root.replace(root_id)?;
        db.pub_key_lookup
            .insert(Owner(account.public_key()), account.username.clone())?;

        self.keychain.cache_account(account.clone()).await?;

        tx.end();

        if welcome_doc {
            let cohort: WelcomeDoc = cohort(&account.username);
            match cohort {
                WelcomeDoc::NoWelcomeDoc => {}
                WelcomeDoc::OldWelcomeDoc => {
                    let welcome_doc = self
                        .create_file("welcome.md", &root_id, FileType::Document)
                        .await?;
                    self.write_document(welcome_doc.id, MARKDOWN_WELCOME.as_bytes())
                        .await?;
                    self.sync().await?;
                }
                WelcomeDoc::FramgentedArchetypes => {
                    let samples = self
                        .create_file("samples", &root_id, FileType::Folder)
                        .await?;

                    let todo = self
                        .create_file("todo.md", &samples.id, FileType::Document)
                        .await?;
                    self.write_document(todo.id, include_bytes!("samples/todo.md"))
                        .await?;

                    let journal = self
                        .create_file("journal.md", &samples.id, FileType::Document)
                        .await?;
                    let journal_content = include_str!("samples/journal.md")
                        .replace("{date}", &chrono::Local::now().format("%y-%m-%d").to_string());
                    self.write_document(journal.id, journal_content.as_bytes())
                        .await?;

                    let pdf = self
                        .create_file("bitcoin.pdf", &samples.id, FileType::Document)
                        .await?;
                    self.write_document(pdf.id, include_bytes!("samples/bitcoin.pdf"))
                        .await?;

                    let drawing = self
                        .create_file("drawing.svg", &samples.id, FileType::Document)
                        .await?;
                    self.write_document(drawing.id, include_bytes!("samples/drawing.svg"))
                        .await?;

                    let welcome = self
                        .create_file("welcome.md", &root_id, FileType::Document)
                        .await?;
                    self.write_document(welcome.id, include_bytes!("samples/welcome.md"))
                        .await?;

                    self.sync().await?;
                }
                WelcomeDoc::HypeWelcomeDoc => {
                    let welcome_doc = self
                        .create_file("welcome.md", &root_id, FileType::Document)
                        .await?;
                    self.write_document(welcome_doc.id, include_bytes!("samples/hype_welcome.md"))
                        .await?;
                    self.sync().await?;
                }
            };
        }

        self.events.meta_changed(Actor::User(None));
        self.events.signed_in();

        Ok(account)
    }

    #[instrument(level = "debug", skip(self, key), err(Debug))]
    pub async fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        if self.get_account().is_ok() {
            warn!("tried to import an account, but account exists already.");
            return Err(LbErrKind::AccountExists.into());
        }

        let key = key.trim();

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
            .get_public_key(&account, GetPublicKeyRequest { username: account.username.clone() })
            .await?
            .key;

        let account_public_key = account.public_key();

        if account_public_key != server_public_key {
            return Err(LbErrKind::UsernamePublicKeyMismatch.into());
        }

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.replace(account.clone())?;
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
            .get_username(&account, GetUsernameRequest { key: public_key })
            .await?
            .username;

        let mut tx = self.begin_tx().await;
        let db = tx.db();
        db.account.replace(account.clone())?;
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
        let account = self.get_account()?;
        Ok(base64::encode(account.private_key.serialize()))
    }

    pub fn export_account_phrase(&self) -> LbResult<String> {
        let account = self.get_account()?;
        Ok(account.get_phrase()?.join(" "))
    }

    pub fn export_account_qr(&self) -> LbResult<Vec<u8>> {
        let acct_secret = self.export_account_private_key()?;
        qrcode_generator::to_png_to_vec(acct_secret, QrCodeEcc::Low, 1024)
            .map_err(|err| core_err_unexpected(err).into())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete_account(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .delete_account(account, DeleteAccountRequest {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        db.account.take()?;
        db.last_synced.take()?;
        db.base_metadata.clear()?;
        db.root.take()?;
        db.local_metadata.clear()?;
        db.pub_key_lookup.clear()?;

        // todo: clear cache?

        Ok(())
    }
}
