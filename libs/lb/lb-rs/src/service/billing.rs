use crate::Lb;
use crate::io::network::ApiError;
use crate::io::server::Server;
use crate::model::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, GetSubscriptionInfoRequestV2,
    StripeBillingPortalError, StripeBillingPortalRequest, StripeCheckoutError,
    StripeCheckoutRequest, StripePlan, SubscriptionInfo, UpgradeAccountAppStoreError,
    UpgradeAccountAppStoreRequest, UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
};
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};

// todo: when core is responsible for syncing, these should probably trigger syncs and status updates
impl<S: Server> Lb<S> {
    /// A Stripe Checkout page where this account pays for Premium on `plan`.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn stripe_checkout_url(&self, plan: StripePlan) -> LbResult<String> {
        let account = self.get_account()?;

        let response = self
            .client
            .stripe_checkout(account, StripeCheckoutRequest { plan })
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    StripeCheckoutError::AlreadyPremium => LbErrKind::AlreadyPremium,
                    StripeCheckoutError::ExistingRequestPending => {
                        LbErrKind::ExistingRequestPending
                    }
                    StripeCheckoutError::UserNotFound => LbErrKind::AccountNonexistent,
                },
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(response.url)
    }

    /// A Stripe billing page where this account changes its card, gets receipts, or cancels.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn stripe_billing_portal_url(&self) -> LbResult<String> {
        let account = self.get_account()?;

        let response = self
            .client
            .stripe_billing_portal(account, StripeBillingPortalRequest {})
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(StripeBillingPortalError::UserNotFound) => {
                    LbErrKind::AccountNonexistent
                }
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(response.url)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .upgrade_account_google_play(
                account,
                UpgradeAccountGooglePlayRequest {
                    purchase_token: purchase_token.to_string(),
                    account_id: account_id.to_string(),
                },
            )
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountGooglePlayError::AlreadyPremium => LbErrKind::AlreadyPremium,
                    UpgradeAccountGooglePlayError::InvalidPurchaseToken => {
                        LbErrKind::InvalidPurchaseToken
                    }
                    UpgradeAccountGooglePlayError::ExistingRequestPending => {
                        LbErrKind::ExistingRequestPending
                    }
                    UpgradeAccountGooglePlayError::UserNotFound => core_err_unexpected(err),
                },
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .upgrade_account_app_store(
                account,
                UpgradeAccountAppStoreRequest { original_transaction_id, app_account_token },
            )
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountAppStoreError::AlreadyPremium => LbErrKind::AlreadyPremium,
                    UpgradeAccountAppStoreError::InvalidAuthDetails => {
                        LbErrKind::InvalidAuthDetails
                    }
                    UpgradeAccountAppStoreError::ExistingRequestPending => {
                        LbErrKind::ExistingRequestPending
                    }
                    UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked => {
                        LbErrKind::AppStoreAccountAlreadyLinked
                    }
                    UpgradeAccountAppStoreError::UserNotFound => core_err_unexpected(err),
                },
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn cancel_subscription(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .cancel_subscription(account, CancelSubscriptionRequest {})
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(CancelSubscriptionError::NotPremium) => LbErrKind::NotPremium,
                ApiError::Endpoint(CancelSubscriptionError::AlreadyCanceled) => {
                    LbErrKind::AlreadyCanceled
                }
                ApiError::Endpoint(CancelSubscriptionError::UsageIsOverFreeTierDataCap) => {
                    LbErrKind::UsageIsOverFreeTierDataCap
                }
                ApiError::Endpoint(CancelSubscriptionError::ExistingRequestPending) => {
                    LbErrKind::ExistingRequestPending
                }
                ApiError::Endpoint(CancelSubscriptionError::CannotCancelForAppStore) => {
                    LbErrKind::CannotCancelSubscriptionForAppStore
                }
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        let account = self.get_account()?;

        let response = self
            .client
            .get_subscription_info_v2(account, GetSubscriptionInfoRequestV2 {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;
        Ok(response.subscription_info.map(Into::into))
    }
}
