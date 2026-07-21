use crate::LocalLb;
use crate::model::api::GetPublicKeyRequest;
use crate::model::errors::{LbErr, LbErrKind, LbResult};
use crate::model::file::{File, ShareMode};
use crate::model::file_metadata::Owner;
use crate::model::tree_like::TreeLike;
use crate::service::events::Actor;
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl LocalLb {
    /// Resolve `username` → public key without sharing.
    ///
    /// 1. Local `pub_key_lookup` (any owner already mapped to this name)
    /// 2. Server `GetPublicKey` — successful results are cached
    ///
    /// Returns:
    /// - `Ok(Some(pk))` — account exists
    /// - `Ok(None)` — server says the user does not exist
    /// - `Err(ServerUnreachable)` — offline / request failed to send
    /// - `Err(…)` — other failures (e.g. client update required, no account)
    #[instrument(level = "debug", skip(self))]
    pub async fn get_public_key(&self, username: &str) -> LbResult<Option<PublicKey>> {
        let username = username.trim().to_lowercase();
        if username.is_empty() {
            return Ok(None);
        }

        // Cache hit: reverse-lookup by username in the local key map.
        {
            let tx = self.ro_tx().await;
            if let Some(owner) = tx.db().pub_key_lookup.get().iter().find_map(|(owner, name)| {
                if name.eq_ignore_ascii_case(&username) {
                    Some(*owner)
                } else {
                    None
                }
            }) {
                return Ok(Some(owner.0));
            }
        }

        let account = self.get_account()?;
        match self
            .client
            .request(account, GetPublicKeyRequest { username: username.clone() })
            .await
        {
            Ok(resp) => {
                let mut tx = self.begin_tx().await;
                tx.db()
                    .pub_key_lookup
                    .insert(Owner(resp.key), username)?;
                tx.end();
                Ok(Some(resp.key))
            }
            Err(e) => {
                let err = LbErr::from(e);
                match err.kind {
                    // UserNotFound maps to AccountNonexistent today — treat as
                    // a soft miss so callers can distinguish offline vs missing.
                    LbErrKind::AccountNonexistent => Ok(None),
                    _ => Err(err),
                }
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()> {
        let username = username.to_lowercase();

        let sharee = match self.get_public_key(&username).await? {
            Some(pk) => Owner(pk),
            None => return Err(LbErrKind::AccountNonexistent.into()),
        };

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

    #[instrument(level = "debug", skip(self))]
    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr> {
        let pk = self.keychain.get_pk()?;
        self.delete_share(id, Some(pk)).await
    }
}
