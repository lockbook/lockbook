use crate::Lb;
use crate::io::network::ApiError;
use crate::model::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, GetSubscriptionInfoRequest,
    StripeAccountTier, SubscriptionInfo, UpgradeAccountAppStoreError,
    UpgradeAccountAppStoreRequest, UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};

// todo: when core is responsible for syncing, these should probably trigger syncs and status updates
impl Lb {
    #[instrument(level = "debug", skip(self, account_tier), err(Debug))]
    pub async fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, UpgradeAccountStripeRequest { account_tier })
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountStripeError::OldCardDoesNotExist => {
                        LbErrKind::OldCardDoesNotExist
                    }
                    UpgradeAccountStripeError::AlreadyPremium => LbErrKind::AlreadyPremium,
                    UpgradeAccountStripeError::InvalidCardNumber => LbErrKind::CardInvalidNumber,
                    UpgradeAccountStripeError::InvalidCardExpYear => LbErrKind::CardInvalidExpYear,
                    UpgradeAccountStripeError::InvalidCardExpMonth => {
                        LbErrKind::CardInvalidExpMonth
                    }
                    UpgradeAccountStripeError::InvalidCardCvc => LbErrKind::CardInvalidCvc,
                    UpgradeAccountStripeError::CardDecline => LbErrKind::CardDecline,
                    UpgradeAccountStripeError::InsufficientFunds => {
                        LbErrKind::CardInsufficientFunds
                    }
                    UpgradeAccountStripeError::TryAgain => LbErrKind::TryAgain,
                    UpgradeAccountStripeError::CardNotSupported => LbErrKind::CardNotSupported,
                    UpgradeAccountStripeError::ExpiredCard => LbErrKind::CardExpired,
                    UpgradeAccountStripeError::ExistingRequestPending => {
                        LbErrKind::ExistingRequestPending
                    }
                    UpgradeAccountStripeError::UserNotFound => LbErrKind::AccountNonexistent,
                },
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(
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
            .request(
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
            .request(account, CancelSubscriptionRequest {})
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

        Ok(self
            .client
            .request(account, GetSubscriptionInfoRequest {})
            .await
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .subscription_info)
    }
}
