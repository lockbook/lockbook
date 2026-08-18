use crate::LocalLb;
use crate::model::api::GetPublicKeyRequest;
use crate::model::errors::{LbErr, LbResult};
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::Owner;
use crate::model::tree_like::TreeLike;
use crate::service::events::Actor;
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl LocalLb {
    // todo: this can check whether the username is known already
    #[instrument(level = "debug", skip(self))]
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        let account = self.get_account()?;
        let username = username.to_lowercase();

        let sharee = Owner(
            self.client
                .request(account, GetPublicKeyRequest { username: username.clone() })
                .await
                .map_err(LbErr::from)?
                .key,
        );

        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        db.pub_key_lookup.insert(sharee, username)?;

        tree.add_share(id, sharee, mode, &self.keychain)?;

        tx.end();

        self.events.meta_changed(Actor::User(None));

        Ok(())
    }

    /// returns pending shares -- files shared with us that we haven't accepted or rejected
    /// this function just returns the actual files that were shared -- or the roots of shared
    /// trees. For the full set of shares see [Self::get_pending_share_files]
    #[instrument(level = "debug", skip(self))]
    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let pending_roots = tree.pending_roots(&self.keychain)?.into_iter();

        tree.decrypt_all(&self.keychain, pending_roots, &db.pub_key_lookup, false)
    }

    /// returns *all* the files associated with any pending shares (the share as well as it's
    /// descendants).
    #[instrument(level = "debug", skip(self))]
    pub async fn get_pending_share_files(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let pending_files = tree.non_deleted_pending_files(&self.keychain)?.into_iter();

        tree.decrypt_all(&self.keychain, pending_files, &db.pub_key_lookup, false)
    }

    #[instrument(level = "debug", skip(self))]
    async fn delete_share(
        &self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>,
    ) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        tree.delete_share(id, maybe_encrypted_for, &self.keychain)?;

        tx.end();
        self.events.meta_changed(Actor::User(None));

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn known_usernames(&self) -> LbResult<Vec<String>> {
        let db = self.ro_tx().await;
        let db = db.db();

        Ok(db.pub_key_lookup.get().values().cloned().collect())
    }

    /// Whether `username` is a known Lockbook account (local cache, then server).
    ///
    /// Server lookup uses [`GetPublicKeyRequest`]. Auth is only a signed envelope —
    /// when signed out we probe with an ephemeral key so onboard can check
    /// availability without an account. `UserNotFound` maps to `false`.
    #[instrument(level = "debug", skip(self))]
    pub async fn username_exists(&self, username: &str) -> LbResult<bool> {
        use crate::DEFAULT_API_LOCATION;
        use crate::model::account::Account;
        use crate::model::errors::LbErrKind;

        let username = username.trim().to_lowercase();
        if username.is_empty() {
            return Ok(false);
        }
        // Signed-in only: local roster can short-circuit a hit. Empty when signed out.
        if let Ok(known) = self.known_usernames().await {
            if known.iter().any(|u| u.eq_ignore_ascii_case(&username)) {
                return Ok(true);
            }
        }

        // Prefer real account for api_url; else ephemeral signer + API_URL / default.
        let owned;
        let account = match self.get_account() {
            Ok(a) => a,
            Err(_) => {
                let api =
                    std::env::var("API_URL").unwrap_or_else(|_| DEFAULT_API_LOCATION.to_string());
                owned = Account::new(String::new(), api);
                &owned
            }
        };

        match self
            .client
            .request(account, GetPublicKeyRequest { username })
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let err = LbErr::from(e);
                match err.kind {
                    // GetPublicKey UserNotFound → AccountNonexistent in LbErr map.
                    LbErrKind::AccountNonexistent | LbErrKind::UsernameNotFound => Ok(false),
                    _ => Err(err),
                }
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        let pk = self.keychain.get_pk()?;
        self.delete_share(id, Some(pk)).await
    }
}
