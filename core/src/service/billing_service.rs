use crate::model::errors::core_err_unexpected;
use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{CoreError, Tx};
use lockbook_models::api::{
    CancelSubscriptionError, CancelSubscriptionRequest, ConfirmAndroidSubscriptionError,
    ConfirmAndroidSubscriptionRequest, GetCreditCardError, GetCreditCardRequest, PaymentMethod,
    PremiumAccountType, StripeAccountTier, UpgradeAccountStripeError, UpgradeAccountStripeRequest,
};

pub type CreditCardLast4Digits = String;

impl Tx<'_> {
    pub fn upgrade_account_stripe(&self, account_tier: StripeAccountTier) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, UpgradeAccountStripeRequest { account_tier }).map_err(
            |err| match err {
                ApiError::Endpoint(err) => match err {
                    UpgradeAccountStripeError::OldCardDoesNotExist => {
                        CoreError::OldCardDoesNotExist
                    }
                    UpgradeAccountStripeError::NewTierIsOldTier => CoreError::NewTierIsOldTier,
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
                    UpgradeAccountStripeError::ConcurrentRequestsAreTooSoon => {
                        CoreError::ConcurrentRequestsAreTooSoon
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

    pub fn get_credit_card(&self) -> Result<CreditCardLast4Digits, CoreError> {
        let account = self.get_account()?;

        Ok(api_service::request(&account, GetCreditCardRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(GetCreditCardError::NoCardAdded) => CoreError::NoCardAdded,
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .credit_card_last_4_digits)
    }

    pub fn confirm_android_subscription(&self, purchase_token: &str) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(
            &account,
            ConfirmAndroidSubscriptionRequest { purchase_token: purchase_token.to_string() },
        )
        .map_err(|err| match err {
            ApiError::Endpoint(ConfirmAndroidSubscriptionError::AlreadyPremium) => {
                CoreError::AlreadyPremium
            }
            ApiError::Endpoint(ConfirmAndroidSubscriptionError::InvalidPurchaseToken) => {
                CoreError::InvalidPurchaseToken
            }
            ApiError::Endpoint(ConfirmAndroidSubscriptionError::ConcurrentRequestsAreTooSoon) => {
                CoreError::ConcurrentRequestsAreTooSoon
            }
            _ => core_err_unexpected(err),
        })?;

        Ok(())
    }

    pub fn cancel_subscription(&self) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, CancelSubscriptionRequest {}).map_err(|err| match err {
            ApiError::Endpoint(CancelSubscriptionError::NotPremium) => CoreError::NotPremium,
            ApiError::Endpoint(CancelSubscriptionError::UsageIsOverFreeTierDataCap) => {
                CoreError::UsageIsOverFreeTierDataCap
            }
            ApiError::Endpoint(CancelSubscriptionError::ConcurrentRequestsAreTooSoon) => {
                CoreError::ConcurrentRequestsAreTooSoon
            }
            _ => core_err_unexpected(err),
        })?;

        Ok(())
    }
}
