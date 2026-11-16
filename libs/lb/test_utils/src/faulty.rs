//! A client's server whose requests a test drops, the way a network does.

use std::sync::{Arc, Mutex};

use lb_rs::io::network::ApiError;
use lb_rs::io::server::{ApiResult, Server};
use lb_rs::model::account::Account;
use lb_rs::model::api::*;
use lb_rs::model::file_like::FileLike;
use uuid::Uuid;

/// Where a dropped request is lost: on its way to the server, which never sees it, or on the
/// answer's way back, after the server applied it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lost {
    Request,
    Answer,
}

/// Which request is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The request after the next `n` the client sends one at a time: every request but
    /// [`ChangeDocRequestV2`] and [`GetDocRequest`], which a sync sends in batches whose order
    /// varies by machine.
    Nth(usize),
    /// The upload of the document with this id.
    Upload(Uuid),
}

/// `S` dropping the request a test names. Each clone shares the fault, so a world gives each
/// device its own.
#[derive(Clone)]
pub struct Faulty<S> {
    pub server: S,
    fault: Arc<Mutex<Fault>>,
}

#[derive(Default)]
struct Fault {
    /// What to drop, until it's dropped.
    armed: Option<(Target, Lost)>,
    /// The endpoint whose request was dropped since the fault was armed.
    dropped: Option<&'static str>,
}

impl<S> Faulty<S> {
    pub fn new(server: S) -> Self {
        Self { server, fault: Default::default() }
    }

    /// Drops `target` once it comes, where `lost` says.
    pub fn fail(&self, target: Target, lost: Lost) {
        *self.fault.lock().unwrap() = Fault { armed: Some((target, lost)), dropped: None };
    }

    /// Drops nothing, returning the endpoint whose request was dropped, if one was.
    pub fn heal(&self) -> Option<&'static str> {
        std::mem::take(&mut *self.fault.lock().unwrap()).dropped
    }

    /// Where `endpoint`'s request is lost, if it is: `upload` names the document a request
    /// uploads, and `counted` says whether the request counts toward [`Target::Nth`].
    fn lost(&self, endpoint: &'static str, upload: Option<Uuid>, counted: bool) -> Option<Lost> {
        let mut fault = self.fault.lock().unwrap();
        let lost = match fault.armed.as_mut()? {
            (Target::Nth(0), lost) if counted => *lost,
            (Target::Nth(n), _) if counted => {
                *n -= 1;
                return None;
            }
            (Target::Upload(id), lost) if upload == Some(*id) => *lost,
            _ => return None,
        };
        *fault = Fault { armed: None, dropped: Some(endpoint) };
        Some(lost)
    }
}

macro_rules! drop_request {
    ($lost:expr, $answer:expr) => {
        match $lost {
            None => $answer.await,
            Some(Lost::Request) => Err(ApiError::SendFailed("lost on the way".into())),
            Some(Lost::Answer) => {
                let _ = $answer.await;
                Err(ApiError::ReceiveFailed("lost on the way back".into()))
            }
        }
    };
}

macro_rules! faulty {
    ($($endpoint:ident: $request:ty),* $(,)?) => {
        $(
            async fn $endpoint(&self, account: &Account, request: $request) -> ApiResult<$request> {
                drop_request!(
                    self.lost(stringify!($endpoint), None, true),
                    self.server.$endpoint(account, request)
                )
            }
        )*
    };
}

impl<S: Server> Server for Faulty<S> {
    async fn change_doc_v2(
        &self, account: &Account, request: ChangeDocRequestV2,
    ) -> ApiResult<ChangeDocRequestV2> {
        let lost = self.lost("change_doc_v2", Some(*request.diff.new.id()), false);
        drop_request!(lost, self.server.change_doc_v2(account, request))
    }

    async fn get_document(
        &self, account: &Account, request: GetDocRequest,
    ) -> ApiResult<GetDocRequest> {
        drop_request!(
            self.lost("get_document", None, false),
            self.server.get_document(account, request)
        )
    }

    faulty! {
        new_account_v2: NewAccountRequestV2,
        upsert_file_metadata_v2: UpsertRequestV2,
        get_public_key: GetPublicKeyRequest,
        get_username: GetUsernameRequest,
        get_usage: GetUsageRequest,
        get_file_ids: GetFileIdsRequest,
        get_updates_v2: GetUpdatesRequestV2,
        get_build_info: GetBuildInfoRequest,
        delete_account: DeleteAccountRequest,
        upgrade_account_stripe: UpgradeAccountStripeRequest,
        upgrade_account_google_play: UpgradeAccountGooglePlayRequest,
        upgrade_account_app_store: UpgradeAccountAppStoreRequest,
        cancel_subscription: CancelSubscriptionRequest,
        get_subscription_info: GetSubscriptionInfoRequest,
        get_subscription_info_v2: GetSubscriptionInfoRequestV2,
        upsert_debug_info: UpsertDebugInfoRequest,
        admin_disappear_account: AdminDisappearAccountRequest,
        admin_disappear_file: AdminDisappearFileRequest,
        admin_validate_account: AdminValidateAccountRequest,
        admin_validate_server: AdminValidateServerRequest,
        admin_list_users: AdminListUsersRequest,
        admin_get_account_info: AdminGetAccountInfoRequest,
        admin_file_info: AdminFileInfoRequest,
        admin_set_user_tier: AdminSetUserTierRequest,
        admin_rebuild_index: AdminRebuildIndexRequest,
    }
}
