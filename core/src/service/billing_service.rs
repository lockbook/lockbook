use crate::model::errors::core_err_unexpected;
use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{CoreError, Tx};
use lockbook_models::api::{PremiumAccountType, GetCreditCardError, GetCreditCardRequest, CancelAndroidSubscriptionRequest, CancelAndroidSubscriptionError, ConfirmAndroidSubscriptionRequest, ConfirmAndroidSubscriptionError, SwitchAccountTierStripeRequest, SwitchAccountTierStripeError, StripeAccountTier};

pub type CreditCardLast4Digits = String;

impl Tx<'_> {
    pub fn switch_account_tier_stripe(&self, new_account_tier: StripeAccountTier) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, SwitchAccountTierStripeRequest { account_tier: new_account_tier })
            .map_err(|err| match err {
                ApiError::Endpoint(err) => match err {
                    SwitchAccountTierStripeError::OldCardDoesNotExist => CoreError::OldCardDoesNotExist,
                    SwitchAccountTierStripeError::NewTierIsOldTier => CoreError::NewTierIsOldTier,
                    SwitchAccountTierStripeError::InvalidCardNumber => CoreError::InvalidCardNumber,
                    SwitchAccountTierStripeError::InvalidCardExpYear => CoreError::InvalidCardExpYear,
                    SwitchAccountTierStripeError::InvalidCardExpMonth => CoreError::InvalidCardExpMonth,
                    SwitchAccountTierStripeError::InvalidCardCvc => CoreError::InvalidCardCvc,
                    SwitchAccountTierStripeError::CardDecline => CoreError::CardDecline,
                    SwitchAccountTierStripeError::InsufficientFunds => {
                        CoreError::CardHasInsufficientFunds
                    }
                    SwitchAccountTierStripeError::TryAgain => CoreError::TryAgain,
                    SwitchAccountTierStripeError::CardNotSupported => CoreError::CardNotSupported,
                    SwitchAccountTierStripeError::ExpiredCard => CoreError::ExpiredCard,
                    SwitchAccountTierStripeError::CurrentUsageIsMoreThanNewTier => {
                        CoreError::CurrentUsageIsMoreThanNewTier
                    }
                    SwitchAccountTierStripeError::ConcurrentRequestsAreTooSoon => {
                        CoreError::ConcurrentRequestsAreTooSoon
                    }
                    SwitchAccountTierStripeError::UserNotFound => CoreError::AccountNonexistent,
                },
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub fn get_credit_card(&self) -> Result<CreditCardLast4Digits, CoreError> {
        let account = self.get_account()?;

        Ok(api_service::request(&account, GetCreditCardRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(GetCreditCardError::NotAStripeCustomer) => {
                    CoreError::NotAStripeCustomer
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .credit_card_last_4_digits)
    }

    pub fn confirm_android_subscription(&self, purchase_token: &str, new_account_type: PremiumAccountType) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, ConfirmAndroidSubscriptionRequest { purchase_token: purchase_token.to_string(), new_account_type })
            .map_err(|err| match err {
                ApiError::Endpoint(ConfirmAndroidSubscriptionError::AlreadyPremium) => CoreError::AlreadyPremium,
                ApiError::Endpoint(ConfirmAndroidSubscriptionError::InvalidPurchaseToken) => CoreError::InvalidPurchaseToken,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }

    pub fn cancel_android_subscription(&self) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, CancelAndroidSubscriptionRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(CancelAndroidSubscriptionError::NotPremium) => CoreError::NotPremium,
                ApiError::Endpoint(CancelAndroidSubscriptionError::NotAGooglePlayCustomer) => CoreError::NotAGooglePlayCustomer,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }
}
