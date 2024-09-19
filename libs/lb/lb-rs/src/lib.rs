#[macro_use]
extern crate tracing;

pub mod model;
pub mod service;
pub mod shared;
pub mod text;

mod repo;

pub use base64;
pub use basic_human_duration::ChronoHumanDuration;
pub use libsecp256k1::PublicKey;
use service::search_service::{SearchRequest, SearchResult, SearchType};
pub use shared::document_repo::{DocumentService, OnDiskDocuments};
pub use shared::file_metadata::DocumentHmac;
pub use time::Duration;
pub use uuid::Uuid;

pub use shared::account::Account;
pub use shared::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, AppStoreAccountState,
    GooglePlayAccountState, PaymentMethod, PaymentPlatform, ServerIndex, StripeAccountState,
    StripeAccountTier, SubscriptionInfo, UnixTimeMillis,
};
pub use shared::clock;
pub use shared::core_config::Config;
pub use shared::crypto::DecryptedDocument;
pub use shared::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
pub use shared::file::{File, Share, ShareMode};
pub use shared::file_like::FileLike;
pub use shared::file_metadata::{FileType, Owner};
pub use shared::filename::NameComponents;
pub use shared::lazy::LazyTree;
pub use shared::path_ops::Filter;
pub use shared::server_file::ServerFile;
pub use shared::tree_like::{TreeLike, TreeLikeMut};
pub use shared::usage::bytes_to_human;
pub use shared::work_unit::WorkUnit;

pub use crate::model::errors::{
    CoreError, LbError, LbResult, TestRepoError, UnexpectedError, Warning,
};
pub use crate::service::activity_service::RankingWeights;
pub use crate::service::import_export_service::{ExportFileInfo, ImportStatus};
pub use crate::service::search_service::{SearchResultItem, StartSearchInfo};
pub use crate::service::sync_service::{SyncProgress, SyncStatus};
pub use crate::service::usage_service::{UsageItemMetric, UsageMetrics};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam::channel::{self};
use db_rs::Db;
use shared::account::Username;
use shared::api::{
    AccountInfo, AdminFileInfoResponse, AdminValidateAccount, AdminValidateServer, GetUsageRequest,
};

use crate::repo::CoreDb;
use crate::service::api_service::{Network, Requester};
use crate::service::log_service;
use crate::service::sync_service::SyncContext;

pub type Core = CoreLib<Network, OnDiskDocuments>;

#[derive(Clone)]
#[repr(C)]
pub struct CoreLib<Client: Requester, Docs: DocumentService> {
    pub inner: Arc<Mutex<CoreState<Client, Docs>>>,
}

#[repr(C)]
pub struct CoreState<Client: Requester, Docs: DocumentService> {
    pub config: Config,
    pub public_key: Option<PublicKey>,
    pub db: CoreDb,
    pub docs: Docs,
    pub client: Client,
    pub syncing: bool,
}

impl Core {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        log_service::init(config)?;
        let db = CoreDb::init(db_rs::Config::in_folder(&config.writeable_path))
            .map_err(|err| unexpected_only!("{:#?}", err))?;

        let config = config.clone();
        let client = Network::default();
        let docs = OnDiskDocuments::from(&config);
        let syncing = false;

        let state = CoreState { config, public_key: None, db, client, docs, syncing };
        let inner = Arc::new(Mutex::new(state));

        Ok(Self { inner })
    }
}

trait LbResultExt {
    fn expected_errs(self, kinds: &[CoreError]) -> Self;
}

impl<T> LbResultExt for LbResult<T> {
    fn expected_errs(self, kinds: &[CoreError]) -> Self {
        self.map_err(|err| {
            let LbError { kind, mut backtrace } = err;
            for k in kinds {
                if *k == kind {
                    backtrace = None;
                    break;
                }
            }
            LbError { kind, backtrace }
        })
    }
}

impl<Client: Requester, Docs: DocumentService> CoreLib<Client, Docs> {
    pub fn in_tx<F, Out>(&self, f: F) -> LbResult<Out>
    where
        F: FnOnce(&mut CoreState<Client, Docs>) -> LbResult<Out>,
    {
        let mut inner = self.inner.lock()?;
        let tx = inner.db.begin_transaction()?;
        let val = f(&mut inner);
        tx.drop_safely()?;
        val
    }

    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        let account = self
            .in_tx(|s| s.create_account(username, api_url, welcome_doc))
            .expected_errs(&[
                CoreError::AccountExists,
                CoreError::UsernameTaken,
                CoreError::UsernameInvalid,
                CoreError::ServerDisabled,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])?;

        if welcome_doc {
            self.sync(None)?;
        }

        Ok(account)
    }

    /// This function is used to log out and delete the user's data from the local filesystem.
    /// Don't call it without warning the user to back up their private key.
    pub fn logout(self) {
        let inner = self.inner.lock().unwrap();
        let path = &inner.config.writeable_path;
        std::fs::remove_dir_all(path).unwrap();
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn import_account(&self, key: &str, api_url: Option<&str>) -> LbResult<Account> {
        self.in_tx(|s| s.import_account(key, api_url))
            .expected_errs(&[
                CoreError::AccountExists,
                CoreError::AccountNonexistent,
                CoreError::AccountStringCorrupted,
                CoreError::KeyPhraseInvalid,
                CoreError::UsernamePublicKeyMismatch,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account_private_key(&self) -> Result<String, LbError> {
        self.in_tx(|s| s.export_account_private_key_v1())
            .expected_errs(&[CoreError::AccountNonexistent])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account_phrase(&self) -> Result<String, LbError> {
        self.in_tx(|s| s.export_account_phrase())
            .expected_errs(&[CoreError::AccountNonexistent])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account_qr(&self) -> Result<Vec<u8>, LbError> {
        self.in_tx(|s| s.export_account_qr())
            .expected_errs(&[CoreError::AccountNonexistent])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_account(&self) -> Result<Account, LbError> {
        self.in_tx(|s| s.get_account().cloned())
            .expected_errs(&[CoreError::AccountNonexistent])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_config(&self) -> Result<Config, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.config.clone()))?)
    }

    #[instrument(level = "debug", skip(self, name), err(Debug))]
    pub fn create_file(
        &self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<File, LbError> {
        self.in_tx(|s| s.create_file(name, &parent, file_type))
            .expected_errs(&[
                CoreError::FileNameContainsSlash,
                CoreError::FileNameEmpty,
                CoreError::FileNameTooLong,
                CoreError::FileNonexistent,
                CoreError::FileNotFolder,
                CoreError::FileParentNonexistent,
                CoreError::LinkInSharedFolder,
                CoreError::LinkTargetIsOwned,
                CoreError::LinkTargetNonexistent,
                CoreError::InsufficientPermission,
                CoreError::MultipleLinksToSameFile,
                CoreError::PathTaken,
            ])
    }

    // todo this should take ownership of it's vec what the hell
    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub fn write_document(&self, id: Uuid, content: &[u8]) -> Result<(), LbError> {
        self.in_tx(|s| s.write_document(id, content))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::FileNotDocument,
                CoreError::InsufficientPermission,
            ])
    }

    pub fn safe_write(
        &self, id: Uuid, old_hmac: Option<DocumentHmac>, content: Vec<u8>,
    ) -> LbResult<DocumentHmac> {
        self.in_tx(|s| s.safe_write(id, old_hmac, content))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::FileNotDocument,
                CoreError::InsufficientPermission,
                CoreError::ReReadRequired,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_root(&self) -> Result<File, LbError> {
        self.in_tx(|s| s.root())
            .expected_errs(&[CoreError::RootNonexistent])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_children(&self, id: Uuid) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_children(&id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_and_get_children_recursively(&self, id: Uuid) -> Result<Vec<File>, LbError> {
        self.in_tx(|s| s.get_and_get_children_recursively(&id))
            .expected_errs(&[CoreError::FileNonexistent, CoreError::FileNotFolder])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_file_by_id(&self, id: Uuid) -> Result<File, LbError> {
        self.in_tx(|s| s.get_file_by_id(&id))
            .expected_errs(&[CoreError::FileNonexistent])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_file(&self, id: Uuid) -> Result<(), LbError> {
        self.in_tx(|s| s.delete(&id)).expected_errs(&[
            CoreError::RootModificationInvalid,
            CoreError::FileNonexistent,
            CoreError::InsufficientPermission,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn read_document(&self, id: Uuid) -> Result<DecryptedDocument, LbError> {
        self.in_tx(|s| s.read_document(id))
            .expected_errs(&[CoreError::FileNotDocument, CoreError::FileNonexistent])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn read_document_with_hmac(
        &self, id: Uuid,
    ) -> Result<(Option<DocumentHmac>, DecryptedDocument), LbError> {
        self.in_tx(|s| s.read_document_with_hmac(id))
            .expected_errs(&[CoreError::FileNotDocument, CoreError::FileNonexistent])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_metadatas(&self) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.list_metadatas())?)
    }

    #[instrument(level = "debug", skip(self, new_name), err(Debug))]
    pub fn rename_file(&self, id: Uuid, new_name: &str) -> Result<(), LbError> {
        self.in_tx(|s| s.rename_file(&id, new_name))
            .expected_errs(&[
                CoreError::FileNameContainsSlash,
                CoreError::FileNameEmpty,
                CoreError::FileNameTooLong,
                CoreError::FileNonexistent,
                CoreError::InsufficientPermission,
                CoreError::PathTaken,
                CoreError::RootModificationInvalid,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn move_file(&self, id: Uuid, new_parent: Uuid) -> Result<(), LbError> {
        self.in_tx(|s| s.move_file(&id, &new_parent))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::FileNotFolder,
                CoreError::FileParentNonexistent,
                CoreError::FolderMovedIntoSelf,
                CoreError::InsufficientPermission,
                CoreError::LinkInSharedFolder,
                CoreError::PathTaken,
                CoreError::RootModificationInvalid,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> Result<(), LbError> {
        self.in_tx(|s| s.share_file(id, username, mode))
            .expected_errs(&[
                CoreError::RootModificationInvalid,
                CoreError::FileNonexistent,
                CoreError::ShareAlreadyExists,
                CoreError::LinkInSharedFolder,
                CoreError::InsufficientPermission,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_pending_shares(&self) -> Result<Vec<File>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_pending_shares())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_pending_share(&self, id: Uuid) -> Result<(), LbError> {
        self.in_tx(|s| {
            let pk = s.get_public_key()?;
            s.delete_share(&id, Some(pk))
        })
        .expected_errs(&[CoreError::FileNonexistent, CoreError::ShareNonexistent])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_link_at_path(
        &self, path_and_name: &str, target_id: Uuid,
    ) -> Result<File, LbError> {
        self.in_tx(|s| s.create_link_at_path(path_and_name, target_id))
            .expected_errs(&[
                CoreError::FileNotFolder,
                CoreError::PathContainsEmptyFileName,
                CoreError::PathTaken,
                CoreError::FileNameTooLong,
                CoreError::LinkInSharedFolder,
                CoreError::LinkTargetIsOwned,
                CoreError::LinkTargetNonexistent,
                CoreError::MultipleLinksToSameFile,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn create_at_path(&self, path_and_name: &str) -> Result<File, LbError> {
        self.in_tx(|s| s.create_at_path(path_and_name))
            .expected_errs(&[
                CoreError::FileNotFolder,
                CoreError::InsufficientPermission,
                CoreError::PathContainsEmptyFileName,
                CoreError::FileNameTooLong,
                CoreError::PathTaken,
                CoreError::RootNonexistent,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn get_by_path(&self, path: &str) -> Result<File, LbError> {
        self.in_tx(|s| s.get_by_path(path))
            .expected_errs(&[CoreError::FileNonexistent])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_path_by_id(&self, id: Uuid) -> Result<String, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_path_by_id(id))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn list_paths(&self, filter: Option<Filter>) -> Result<Vec<String>, UnexpectedError> {
        Ok(self.in_tx(|s| s.list_paths(filter))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_local_changes(&self) -> Result<Vec<Uuid>, UnexpectedError> {
        Ok(self.in_tx(|s| {
            Ok(s.db
                .local_metadata
                .get()
                .keys()
                .copied()
                .collect::<Vec<Uuid>>())
        })?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn calculate_work(&self) -> Result<SyncStatus, LbError> {
        self.in_tx(|s| s.calculate_work())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    // todo: expose work calculated (return value)
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<SyncStatus, LbError> {
        SyncContext::sync(self, f).expected_errs(&[
            CoreError::ServerUnreachable, // todo already syncing?
            CoreError::ClientUpdateRequired,
            CoreError::UsageIsOverDataCap,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.db.last_synced.get().copied().unwrap_or(0)))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced_human_string(&self) -> Result<String, UnexpectedError> {
        let last_synced = self.get_last_synced()?;

        Ok(if last_synced != 0 {
            Duration::milliseconds(clock::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn suggested_docs(&self, settings: RankingWeights) -> Result<Vec<Uuid>, UnexpectedError> {
        Ok(self.in_tx(|s| s.suggested_docs(settings))?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_usage(&self) -> Result<UsageMetrics, LbError> {
        let acc = self.get_account()?;
        let s = self.inner.lock().unwrap();
        let client = s.client.clone();
        drop(s);
        let usage = client.request(&acc, GetUsageRequest {})?;
        self.in_tx(|s| s.get_usage(usage))
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage_breakdown(
        &self,
    ) -> Result<HashMap<Uuid, usize>, UnexpectedError> {
        Ok(self.in_tx(|s| s.get_uncompressed_usage_breakdown())?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, LbError> {
        // todo the errors here are wrong this doesn't talk to the server
        self.in_tx(|s| s.get_uncompressed_usage())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.import_files(sources, dest, update_status))
            .expected_errs(&[
                CoreError::DiskPathInvalid,
                CoreError::FileNonexistent,
                CoreError::FileNotFolder,
                CoreError::FileNameTooLong,
            ])
    }

    #[instrument(level = "debug", skip(self, export_progress), err(Debug))]
    pub fn export_file(
        &self, id: Uuid, destination: PathBuf, edit: bool,
        export_progress: Option<Box<dyn Fn(ExportFileInfo)>>,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.export_file(id, destination, edit, export_progress))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::DiskPathInvalid,
                CoreError::DiskPathTaken,
            ])
    }

    #[instrument(level = "debug", skip(self, input), err(Debug))]
    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, UnexpectedError> {
        Ok(self.in_tx(|s| s.search_file_paths(input))?)
    }

    #[instrument(level = "debug", skip(self, search_type))]
    pub fn start_search(&self, search_type: SearchType) -> StartSearchInfo {
        let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
        let (results_tx, results_rx) = channel::unbounded::<SearchResult>();

        let core = self.clone();

        let results_tx_c = results_tx.clone();

        thread::spawn(move || {
            if let Err(err) = core.in_tx(|s| s.start_search(search_type, results_tx, search_rx)) {
                let _ = results_tx_c.send(SearchResult::Error(err.into()));
            }
        });

        StartSearchInfo { search_tx, results_rx }
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn validate(&self) -> Result<Vec<Warning>, TestRepoError> {
        self.in_tx(|s| Ok(s.test_repo_integrity()))
            .map_err(TestRepoError::Core)?
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> Result<(), LbError> {
        self.in_tx(|s| s.upgrade_account_stripe(account_tier))
            .expected_errs(&[
                CoreError::OldCardDoesNotExist,
                CoreError::CardInvalidNumber,
                CoreError::CardInvalidExpYear,
                CoreError::CardInvalidExpMonth,
                CoreError::CardInvalidCvc,
                CoreError::AlreadyPremium,
                CoreError::ServerUnreachable,
                CoreError::CardDecline,
                CoreError::CardInsufficientFunds,
                CoreError::TryAgain,
                CoreError::CardNotSupported,
                CoreError::CardExpired,
                CoreError::CurrentUsageIsMoreThanNewTier,
                CoreError::ExistingRequestPending,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip(self, purchase_token), err(Debug))]
    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.upgrade_account_google_play(purchase_token, account_id))
            .expected_errs(&[
                CoreError::AlreadyPremium,
                CoreError::InvalidAuthDetails,
                CoreError::ExistingRequestPending,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::AppStoreAccountAlreadyLinked,
            ])
    }

    #[instrument(
        level = "debug",
        skip(self, original_transaction_id, app_account_token),
        err(Debug)
    )]
    pub fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.upgrade_account_app_store(original_transaction_id, app_account_token))
            .expected_errs(&[
                CoreError::AlreadyPremium,
                CoreError::InvalidPurchaseToken,
                CoreError::ExistingRequestPending,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::AppStoreAccountAlreadyLinked,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn cancel_subscription(&self) -> Result<(), LbError> {
        self.in_tx(|s| s.cancel_subscription()).expected_errs(&[
            CoreError::NotPremium,
            CoreError::AlreadyCanceled,
            CoreError::UsageIsOverFreeTierDataCap,
            CoreError::ExistingRequestPending,
            CoreError::CannotCancelSubscriptionForAppStore,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_subscription_info(&self) -> Result<Option<SubscriptionInfo>, LbError> {
        self.in_tx(|s| s.get_subscription_info())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn delete_account(&self) -> Result<(), LbError> {
        self.in_tx(|s| s.delete_account())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self, username), err(Debug))]
    pub fn admin_disappear_account(&self, username: &str) -> Result<(), LbError> {
        self.in_tx(|s| s.disappear_account(username))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_disappear_file(&self, id: Uuid) -> Result<(), LbError> {
        self.in_tx(|s| s.disappear_file(id)).expected_errs(&[
            CoreError::FileNonexistent,
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self, filter), err(Debug))]
    pub fn admin_list_users(
        &self, filter: Option<AccountFilter>,
    ) -> Result<Vec<Username>, LbError> {
        self.in_tx(|s| s.list_users(filter)).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self, identifier), err(Debug))]
    pub fn admin_get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> Result<AccountInfo, LbError> {
        self.in_tx(|s| s.get_account_info(identifier))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_account(&self, username: &str) -> Result<AdminValidateAccount, LbError> {
        self.in_tx(|s| s.validate_account(username))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_validate_server(&self) -> Result<AdminValidateServer, LbError> {
        self.in_tx(|s| s.validate_server()).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_file_info(&self, id: Uuid) -> Result<AdminFileInfoResponse, LbError> {
        self.in_tx(|s| s.file_info(id)).expected_errs(&[
            CoreError::FileNonexistent,
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn admin_rebuild_index(&self, index: ServerIndex) -> Result<(), LbError> {
        self.in_tx(|s| s.rebuild_index(index)).expected_errs(&[
            CoreError::InsufficientPermission,
            CoreError::ServerUnreachable,
            CoreError::ClientUpdateRequired,
        ])
    }

    #[instrument(level = "debug", skip(self, info), err(Debug))]
    pub fn admin_set_user_tier(
        &self, username: &str, info: AdminSetUserTierInfo,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.set_user_tier(username, info))
            .expected_errs(&[
                CoreError::UsernameNotFound,
                CoreError::InsufficientPermission,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
                CoreError::ExistingRequestPending,
            ])
    }

    #[instrument(level = "debug", skip(self))]
    pub fn debug_info(&self, os_info: String) -> String {
        match self.in_tx(|s| s.debug_info(os_info)) {
            Ok(debug_info) => debug_info,
            Err(e) => format!("failed to produce debug info: {:?}", e.to_string()),
        }
    }
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
