use crate::model::errors::core_err_unexpected;
use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{CoreError, RequestContext};
use lockbook_shared::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, GetSubscriptionInfoRequest,
    StripeAccountTier, SubscriptionInfo, UpgradeAccountGooglePlayError,
    UpgradeAccountGooglePlayRequest, UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};

impl RequestContext<'_, '_> {
    pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, UpgradeAccountStripeRequest { account_tier }).map_err(
            |err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountStripeError::OldCardDoesNotExist => {
                        CoreError::OldCardDoesNotExist
                    }
                    UpgradeAccountStripeError::AlreadyPremium => CoreError::AlreadyPremium,
                    UpgradeAccountStripeError::InvalidCardNumber => CoreError::InvalidCardNumber,
                    UpgradeAccountStripeError::InvalidCardExpYear => CoreError::InvalidCardExpYear,
                    UpgradeAccountStripeError::InvalidCardExpMonth => {
                        CoreError::InvalidCardExpMonth
                    }
                    UpgradeAccountStripeError::InvalidCardCvc => CoreError::InvalidCardCvc,
                    UpgradeAccountStripeError::CardDecline => CoreError::CardDecline,
                    UpgradeAccountStripeError::InsufficientFunds => {
                        CoreError::CardHasInsufficientFunds
                    }
                    UpgradeAccountStripeError::TryAgain => CoreError::TryAgain,
                    UpgradeAccountStripeError::CardNotSupported => CoreError::CardNotSupported,
                    UpgradeAccountStripeError::ExpiredCard => CoreError::ExpiredCard,
                    UpgradeAccountStripeError::ExistingRequestPending => {
                        CoreError::ExistingRequestPending
                    }
                    UpgradeAccountStripeError::UserNotFound => CoreError::AccountNonexistent,
                },
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            },
        )?;

        Ok(())
    }

    pub fn upgrade_account_google_play(
        &self, purchase_token: &str, account_id: &str,
    ) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(
            &account,
            UpgradeAccountGooglePlayRequest {
                purchase_token: purchase_token.to_string(),
                account_id: account_id.to_string(),
            },
        )
        .map_err(|err| match err {
            ApiError::Endpoint(UpgradeAccountGooglePlayError::AlreadyPremium) => {
                CoreError::AlreadyPremium
            }
            ApiError::Endpoint(UpgradeAccountGooglePlayError::InvalidPurchaseToken) => {
                CoreError::InvalidPurchaseToken
            }
            ApiError::Endpoint(UpgradeAccountGooglePlayError::ExistingRequestPending) => {
                CoreError::ExistingRequestPending
            }
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(err),
        })?;

        Ok(())
    }

    pub fn cancel_subscription(&self) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, CancelSubscriptionRequest {}).map_err(|err| match err {
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
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(err),
        })?;

        Ok(())
    }

    pub fn get_subscription_info(&self) -> Result<Option<SubscriptionInfo>, CoreError> {
        let account = self.get_account()?;

        Ok(api_service::request(&account, GetSubscriptionInfoRequest {})
            .map_err(|err| match err {
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .subscription_info)
    }
}
