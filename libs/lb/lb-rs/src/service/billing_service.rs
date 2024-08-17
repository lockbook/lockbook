use crate::shared::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, GetSubscriptionInfoRequest,
    StripeAccountTier, SubscriptionInfo, UpgradeAccountAppStoreError,
    UpgradeAccountAppStoreRequest, UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};
use crate::shared::document_repo::DocumentService;

use crate::model::errors::core_err_unexpected;
use crate::service::api_service::ApiError;
use crate::{CoreError, CoreState, LbResult, Requester};

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, UpgradeAccountStripeRequest { account_tier })
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountStripeError::OldCardDoesNotExist => {
                        CoreError::OldCardDoesNotExist
                    }
                    UpgradeAccountStripeError::AlreadyPremium => CoreError::AlreadyPremium,
                    UpgradeAccountStripeError::InvalidCardNumber => CoreError::CardInvalidNumber,
                    UpgradeAccountStripeError::InvalidCardExpYear => CoreError::CardInvalidExpYear,
                    UpgradeAccountStripeError::InvalidCardExpMonth => {
                        CoreError::CardInvalidExpMonth
                    }
                    UpgradeAccountStripeError::InvalidCardCvc => CoreError::CardInvalidCvc,
                    UpgradeAccountStripeError::CardDecline => CoreError::CardDecline,
                    UpgradeAccountStripeError::InsufficientFunds => {
                        CoreError::CardInsufficientFunds
                    }
                    UpgradeAccountStripeError::TryAgain => CoreError::TryAgain,
                    UpgradeAccountStripeError::CardNotSupported => CoreError::CardNotSupported,
                    UpgradeAccountStripeError::ExpiredCard => CoreError::CardExpired,
                    UpgradeAccountStripeError::ExistingRequestPending => {
                        CoreError::ExistingRequestPending
                    }
                    UpgradeAccountStripeError::UserNotFound => CoreError::AccountNonexistent,
                },
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn upgrade_account_google_play(
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
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountGooglePlayError::AlreadyPremium => CoreError::AlreadyPremium,
                    UpgradeAccountGooglePlayError::InvalidPurchaseToken => {
                        CoreError::InvalidPurchaseToken
                    }
                    UpgradeAccountGooglePlayError::ExistingRequestPending => {
                        CoreError::ExistingRequestPending
                    }
                    UpgradeAccountGooglePlayError::UserNotFound => core_err_unexpected(err),
                },
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn upgrade_account_app_store(
        &self, original_transaction_id: String, app_account_token: String,
    ) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(
                account,
                UpgradeAccountAppStoreRequest { original_transaction_id, app_account_token },
            )
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountAppStoreError::AlreadyPremium => CoreError::AlreadyPremium,
                    UpgradeAccountAppStoreError::InvalidAuthDetails => {
                        CoreError::InvalidAuthDetails
                    }
                    UpgradeAccountAppStoreError::ExistingRequestPending => {
                        CoreError::ExistingRequestPending
                    }
                    UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked => {
                        CoreError::AppStoreAccountAlreadyLinked
                    }
                    UpgradeAccountAppStoreError::UserNotFound => core_err_unexpected(err),
                },
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn cancel_subscription(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, CancelSubscriptionRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(CancelSubscriptionError::NotPremium) => CoreError::NotPremium,
                ApiError::Endpoint(CancelSubscriptionError::AlreadyCanceled) => {
                    CoreError::AlreadyCanceled
                }
                ApiError::Endpoint(CancelSubscriptionError::UsageIsOverFreeTierDataCap) => {
                    CoreError::UsageIsOverFreeTierDataCap
                }
                ApiError::Endpoint(CancelSubscriptionError::ExistingRequestPending) => {
                    CoreError::ExistingRequestPending
                }
                ApiError::Endpoint(CancelSubscriptionError::CannotCancelForAppStore) => {
                    CoreError::CannotCancelSubscriptionForAppStore
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        let account = self.get_account()?;

        Ok(self
            .client
            .request(account, GetSubscriptionInfoRequest {})
            .map_err(|err| match err {
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .subscription_info)
    }
}
