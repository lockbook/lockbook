#[macro_use]
extern crate tracing;

pub mod model;
pub mod service;

mod repo;

pub use base64;
pub use basic_human_duration::ChronoHumanDuration;
pub use libsecp256k1::PublicKey;
pub use time::Duration;
pub use uuid::Uuid;

pub use lockbook_shared::account::Account;
pub use lockbook_shared::api::{
    AccountFilter, AccountIdentifier, AdminSetUserTierInfo, AppStoreAccountState,
    GooglePlayAccountState, PaymentMethod, PaymentPlatform, ServerIndex, StripeAccountState,
    StripeAccountTier, SubscriptionInfo, UnixTimeMillis,
};
pub use lockbook_shared::clock;
pub use lockbook_shared::core_config::Config;
pub use lockbook_shared::crypto::DecryptedDocument;
pub use lockbook_shared::document_repo::RankingWeights;
pub use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
pub use lockbook_shared::file::{File, Share, ShareMode};
pub use lockbook_shared::file_like::FileLike;
pub use lockbook_shared::file_metadata::{FileType, Owner};
pub use lockbook_shared::filename::NameComponents;
pub use lockbook_shared::lazy::LazyTree;
pub use lockbook_shared::path_ops::Filter;
pub use lockbook_shared::server_file::ServerFile;
pub use lockbook_shared::tree_like::{TreeLike, TreeLikeMut};
pub use lockbook_shared::usage::bytes_to_human;
pub use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};

pub use crate::model::drawing::SupportedImageFormats;
pub use crate::model::errors::{
    CoreError, LbError, LbResult, TestRepoError, UnexpectedError, Warning,
};
pub use crate::service::import_export_service::{ExportFileInfo, ImportStatus};
pub use crate::service::search_service::{SearchResultItem, StartSearchInfo};
pub use crate::service::sync_service::{SyncProgress, WorkCalculated};
pub use crate::service::usage_service::{UsageItemMetric, UsageMetrics};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use db_rs::Db;
use lockbook_shared::account::Username;
use lockbook_shared::api::{
    AccountInfo, AdminFileInfoResponse, AdminValidateAccount, AdminValidateServer,
};

use crate::repo::CoreDb;
use crate::service::api_service::{Network, Requester};
use crate::service::log_service;

pub type Core = CoreLib<Network>;

#[derive(Clone)]
pub struct CoreLib<Client: Requester> {
    inner: Arc<Mutex<CoreState<Client>>>,
}

pub struct CoreState<Client: Requester> {
    pub config: Config,
    pub public_key: Option<PublicKey>,
    pub db: CoreDb,
    pub client: Client,
}

impl Core {
    #[instrument(level = "info", skip_all, err(Debug))]
    pub fn init(config: &Config) -> Result<Self, UnexpectedError> {
        log_service::init(config)?;
        let db = CoreDb::init(db_rs::Config::in_folder(&config.writeable_path))
            .map_err(|err| unexpected_only!("{:#?}", err))?;

        let config = config.clone();
        let client = Network::default();

        let state = CoreState { config, public_key: None, db, client };
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

impl<Client: Requester> CoreLib<Client> {
    pub fn in_tx<F, Out>(&self, f: F) -> LbResult<Out>
    where
        F: FnOnce(&mut CoreState<Client>) -> LbResult<Out>,
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
        self.in_tx(|s| s.create_account(username, api_url, welcome_doc))
            .expected_errs(&[
                CoreError::AccountExists,
                CoreError::UsernameTaken,
                CoreError::UsernameInvalid,
                CoreError::ServerDisabled,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn import_account(&self, account_string: &str) -> LbResult<Account> {
        self.in_tx(|s| s.import_account(account_string))
            .expected_errs(&[
                CoreError::AccountExists,
                CoreError::AccountNonexistent,
                CoreError::AccountStringCorrupted,
                CoreError::UsernamePublicKeyMismatch,
                CoreError::ServerUnreachable,
                CoreError::ClientUpdateRequired,
            ])
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn export_account(&self) -> Result<String, LbError> {
        self.in_tx(|s| s.export_account())
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

    #[instrument(level = "debug", skip(self, content), err(Debug))]
    pub fn write_document(&self, id: Uuid, content: &[u8]) -> Result<(), LbError> {
        self.in_tx(|s| s.write_document(id, content))
            .expected_errs(&[
                CoreError::FileNonexistent,
                CoreError::FileNotDocument,
                CoreError::InsufficientPermission,
            ])?;
        self.in_tx(|s| s.cleanup())
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
                .data()
                .keys()
                .copied()
                .collect::<Vec<Uuid>>())
        })?)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn calculate_work(&self) -> Result<WorkCalculated, LbError> {
        self.in_tx(|s| s.calculate_work())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    // todo: expose work calculated (return value)
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync(&self, f: Option<Box<dyn Fn(SyncProgress)>>) -> Result<WorkCalculated, LbError> {
        self.in_tx(|s| {
            let wc = s.sync(f)?;
            s.cleanup()?;
            Ok(wc)
        })
        .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_last_synced(&self) -> Result<i64, UnexpectedError> {
        Ok(self.in_tx(|s| Ok(s.db.last_synced.data().copied().unwrap_or(0)))?)
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
        self.in_tx(|s| s.get_usage())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_uncompressed_usage(&self) -> Result<UsageItemMetric, LbError> {
        self.in_tx(|s| s.get_uncompressed_usage())
            .expected_errs(&[CoreError::ServerUnreachable, CoreError::ClientUpdateRequired])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn get_drawing(&self, id: Uuid) -> Result<Drawing, LbError> {
        self.in_tx(|s| s.get_drawing(id)).expected_errs(&[
            CoreError::DrawingInvalid,
            CoreError::FileNotDocument,
            CoreError::FileNonexistent,
        ])
    }

    #[instrument(level = "debug", skip(self, d), err(Debug))]
    pub fn save_drawing(&self, id: Uuid, d: &Drawing) -> Result<(), LbError> {
        self.in_tx(|s| {
            s.save_drawing(id, d)?;
            s.cleanup()
        })
        .expected_errs(&[
            CoreError::DrawingInvalid,
            CoreError::FileNonexistent,
            CoreError::FileNotDocument,
        ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, LbError> {
        self.in_tx(|s| s.export_drawing(id, format, render_theme))
            .expected_errs(&[
                CoreError::DrawingInvalid,
                CoreError::FileNonexistent,
                CoreError::FileNotDocument,
            ])
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn export_drawing_to_disk(
        &self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), LbError> {
        self.in_tx(|s| s.export_drawing_to_disk(id, format, render_theme, location))
            .expected_errs(&[
                CoreError::DrawingInvalid,
                CoreError::FileNonexistent,
                CoreError::FileNotDocument,
                CoreError::DiskPathInvalid,
                CoreError::DiskPathTaken,
            ])
    }

    #[instrument(level = "debug", skip(self, update_status), err(Debug))]
    pub fn import_files<F: Fn(ImportStatus)>(
        &self, sources: &[PathBuf], dest: Uuid, update_status: &F,
    ) -> Result<(), LbError> {
        self.in_tx(|s| {
            s.import_files(sources, dest, update_status)?;
            s.cleanup()
        })
        .expected_errs(&[
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

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub fn start_search(&self) -> Result<StartSearchInfo, UnexpectedError> {
        Ok(self.in_tx(|s| s.start_search())?)
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
}

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub static DEFAULT_API_LOCATION: &str = "https://api.prod.lockbook.net";
pub static CORE_CODE_VERSION: &str = env!("CARGO_PKG_VERSION");
