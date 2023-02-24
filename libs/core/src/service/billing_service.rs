use lockbook_shared::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, GetSubscriptionInfoRequest,
    StripeAccountTier, SubscriptionInfo, UpgradeAccountAppStoreError,
    UpgradeAccountAppStoreRequest, UpgradeAccountGooglePlayError, UpgradeAccountGooglePlayRequest,
    UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};

use crate::model::errors::lb_err_unexpected;
use crate::service::api_service::ApiError;
use crate::{CoreState, LbErrorKind, LbResult, Requester};

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, UpgradeAccountStripeRequest { account_tier })
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountStripeError::OldCardDoesNotExist => {
                        LbErrorKind::OldCardDoesNotExist
                    }
                    UpgradeAccountStripeError::AlreadyPremium => LbErrorKind::AlreadyPremium,
                    UpgradeAccountStripeError::InvalidCardNumber => LbErrorKind::CardInvalidNumber,
                    UpgradeAccountStripeError::InvalidCardExpYear => {
                        LbErrorKind::CardInvalidExpYear
                    }
                    UpgradeAccountStripeError::InvalidCardExpMonth => {
                        LbErrorKind::CardInvalidExpMonth
                    }
                    UpgradeAccountStripeError::InvalidCardCvc => LbErrorKind::CardInvalidCvc,
                    UpgradeAccountStripeError::CardDecline => LbErrorKind::CardDecline,
                    UpgradeAccountStripeError::InsufficientFunds => {
                        LbErrorKind::CardInsufficientFunds
                    }
                    UpgradeAccountStripeError::TryAgain => LbErrorKind::TryAgain,
                    UpgradeAccountStripeError::CardNotSupported => LbErrorKind::CardNotSupported,
                    UpgradeAccountStripeError::ExpiredCard => LbErrorKind::CardExpired,
                    UpgradeAccountStripeError::ExistingRequestPending => {
                        LbErrorKind::ExistingRequestPending
                    }
                    UpgradeAccountStripeError::UserNotFound => LbErrorKind::AccountNonexistent,
                },
                ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
                _ => lb_err_unexpected(err),
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
                    UpgradeAccountGooglePlayError::AlreadyPremium => LbErrorKind::AlreadyPremium,
                    UpgradeAccountGooglePlayError::InvalidPurchaseToken => {
                        LbErrorKind::InvalidPurchaseToken
                    }
                    UpgradeAccountGooglePlayError::ExistingRequestPending => {
                        LbErrorKind::ExistingRequestPending
                    }
                    UpgradeAccountGooglePlayError::UserNotFound => lb_err_unexpected(err),
                },
                ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
                _ => lb_err_unexpected(err),
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
                    UpgradeAccountAppStoreError::AlreadyPremium => LbErrorKind::AlreadyPremium,
                    UpgradeAccountAppStoreError::InvalidAuthDetails => {
                        LbErrorKind::InvalidAuthDetails
                    }
                    UpgradeAccountAppStoreError::ExistingRequestPending => {
                        LbErrorKind::ExistingRequestPending
                    }
                    UpgradeAccountAppStoreError::AppStoreAccountAlreadyLinked => {
                        LbErrorKind::AppStoreAccountAlreadyLinked
                    }
                    UpgradeAccountAppStoreError::UserNotFound => lb_err_unexpected(err),
                },
                ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
                _ => lb_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn cancel_subscription(&self) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, CancelSubscriptionRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(CancelSubscriptionError::NotPremium) => LbErrorKind::NotPremium,
                ApiError::Endpoint(CancelSubscriptionError::AlreadyCanceled) => {
                    LbErrorKind::AlreadyCanceled
                }
                ApiError::Endpoint(CancelSubscriptionError::UsageIsOverFreeTierDataCap) => {
                    LbErrorKind::UsageIsOverFreeTierDataCap
                }
                ApiError::Endpoint(CancelSubscriptionError::ExistingRequestPending) => {
                    LbErrorKind::ExistingRequestPending
                }
                ApiError::Endpoint(CancelSubscriptionError::CannotCancelForAppStore) => {
                    LbErrorKind::CannotCancelSubscriptionForAppStore
                }
                ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
                _ => lb_err_unexpected(err),
            })?;

        Ok(())
    }

    pub(crate) fn get_subscription_info(&self) -> LbResult<Option<SubscriptionInfo>> {
        let account = self.get_account()?;

        Ok(self
            .client
            .request(account, GetSubscriptionInfoRequest {})
            .map_err(|err| match err {
                ApiError::SendFailed(_) => LbErrorKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrorKind::ClientUpdateRequired,
                _ => lb_err_unexpected(err),
            })?
            .subscription_info)
    }
}
