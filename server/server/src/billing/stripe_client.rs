use crate::billing::stripe_client::SimplifiedStripeError::{CardDeclined, InvalidCreditCard, Other};

use crate::{ServerState, StripeKnownErrorDeclineCode, StripeMaybeContainer};
use lockbook_models::api::{CardDeclineReason, CreditCardRejectReason};
use log::error;
use std::fmt::Debug;
use stripe::{CardDetailsParams, ErrorCode, Expandable, ParseIdError, PaymentIntentStatus, PaymentMethod, PaymentMethodId, SetupIntentStatus, StripeError, SubscriptionStatus};

#[derive(Debug)]
pub enum SimplifiedStripeError {
    CardDeclined(CardDeclineReason),
    InvalidCreditCard(CreditCardRejectReason),
    Other(String),
}

impl From<StripeError> for SimplifiedStripeError {
    fn from(e: StripeError) -> Self {
        match e {
            StripeError::Stripe(stripe_error) => simplify_stripe_error(stripe_error.code, stripe_error.decline_code),
            _ => SimplifiedStripeError::Other(format!("Unexpected stripe error was encountered: {:?}", e))
        }
    }
}

impl From<stripe::ParseIdError> for SimplifiedStripeError {
    fn from(e: ParseIdError) -> Self {
        Other(format!("Stripe parsing error: {:?}", e))
    }
}

fn simplify_stripe_error(error_code: Option<stripe::ErrorCode>, maybe_decline_code: Option<String>) -> SimplifiedStripeError {
    match error_code {
        None => {
            Other(format!("Although a stripe error was encountered, there is no details about it. error_code: {:?}, decline_code: {:?}", error_code, maybe_decline_code))
        },
        Some(error_code) => match error_code {
            ErrorCode::BalanceInsufficient => CardDeclined(CardDeclineReason::BalanceOrCreditExceeded),
            ErrorCode::CardDeclined => match maybe_decline_code {
                None => CardDeclined(CardDeclineReason::Generic),
                Some(decline_code) => match serde_json::from_slice::<StripeMaybeContainer<StripeKnownErrorDeclineCode, String>>(decline_code.as_bytes()).map_err(|e| SimplifiedStripeError::Other(format!("An error was encountered while serializing decline code: {:?}", e))) {
                    Ok(StripeMaybeContainer::Unexpected(unknown_decline_code)) => {
                        error!("Unknown decline code given from stripe: {}", unknown_decline_code);
                        CardDeclined(CardDeclineReason::Generic)
                    }
                    Ok(StripeMaybeContainer::Expected(decline_code)) => match decline_code {
                        // Try again
                        StripeKnownErrorDeclineCode::ApproveWithId
                        | StripeKnownErrorDeclineCode::IssuerNotAvailable
                        | StripeKnownErrorDeclineCode::ProcessingError
                        | StripeKnownErrorDeclineCode::ReenterTransaction
                        | StripeKnownErrorDeclineCode::TryAgainLater => {
                            CardDeclined(CardDeclineReason::TryAgain)
                        }

                        //Unknown
                        StripeKnownErrorDeclineCode::CallIssuer
                        | StripeKnownErrorDeclineCode::DoNotTryAgain
                        | StripeKnownErrorDeclineCode::DoNotHonor
                        | StripeKnownErrorDeclineCode::NewAccountInformationAvailable
                        | StripeKnownErrorDeclineCode::RestrictedCard
                        | StripeKnownErrorDeclineCode::RevocationOfAllAuthorizations
                        | StripeKnownErrorDeclineCode::RevocationOfAuthorization
                        | StripeKnownErrorDeclineCode::SecurityViolation
                        | StripeKnownErrorDeclineCode::ServiceNotAllowed
                        | StripeKnownErrorDeclineCode::StopPaymentOrder
                        | StripeKnownErrorDeclineCode::TransactionNotAllowed => {
                            CardDeclined(CardDeclineReason::Unknown)
                        }

                        // Not supported
                        StripeKnownErrorDeclineCode::CardNotSupported
                        | StripeKnownErrorDeclineCode::CurrencyNotSupported => {
                            CardDeclined(CardDeclineReason::NotSupported)
                        }

                        // Balance or credit exceeded
                        StripeKnownErrorDeclineCode::CardVelocityExceeded
                        | StripeKnownErrorDeclineCode::InsufficientFunds
                        | StripeKnownErrorDeclineCode::WithdrawalCountLimitExceeded => {
                            CardDeclined(CardDeclineReason::BalanceOrCreditExceeded)
                        }

                        // Expired card
                        StripeKnownErrorDeclineCode::ExpiredCard => {
                            CardDeclined(CardDeclineReason::ExpiredCard)
                        }

                        // Generic
                        StripeKnownErrorDeclineCode::Fraudulent
                        | StripeKnownErrorDeclineCode::GenericDecline
                        | StripeKnownErrorDeclineCode::LostCard
                        | StripeKnownErrorDeclineCode::MerchantBlacklist
                        | StripeKnownErrorDeclineCode::NoActionTaken
                        | StripeKnownErrorDeclineCode::NotPermitted
                        | StripeKnownErrorDeclineCode::PickupCard
                        | StripeKnownErrorDeclineCode::StolenCard => {
                            CardDeclined(CardDeclineReason::Generic)
                        }

                        // Incorrect number
                        StripeKnownErrorDeclineCode::IncorrectNumber
                        | StripeKnownErrorDeclineCode::InvalidNumber => {
                            CardDeclined(CardDeclineReason::IncorrectNumber)
                        }

                        // Incorrect cvc
                        StripeKnownErrorDeclineCode::IncorrectCvc
                        | StripeKnownErrorDeclineCode::InvalidCvc => {
                            CardDeclined(CardDeclineReason::IncorrectCVC)
                        }

                        // Incorrect expiry month
                        StripeKnownErrorDeclineCode::InvalidExpiryMonth => {
                            CardDeclined(CardDeclineReason::IncorrectExpiryMonth)
                        }

                        // Incorrect expiry year
                        StripeKnownErrorDeclineCode::InvalidExpiryYear => {
                            CardDeclined(CardDeclineReason::IncorrectExpiryYear)
                        }
                    }
                    Err(e) => e,
                }
            }
            ErrorCode::ExpiredCard => CardDeclined(CardDeclineReason::ExpiredCard),
            ErrorCode::InvalidCardType => CardDeclined(CardDeclineReason::NotSupported),
            ErrorCode::InvalidCvc | ErrorCode::IncorrectCvc => InvalidCreditCard(CreditCardRejectReason::CVC),
            ErrorCode::InvalidExpiryMonth => InvalidCreditCard(CreditCardRejectReason::ExpMonth),
            ErrorCode::InvalidExpiryYear => InvalidCreditCard(CreditCardRejectReason::ExpYear),
            ErrorCode::InvalidNumber | ErrorCode::IncorrectNumber => InvalidCreditCard(CreditCardRejectReason::Number),
            ErrorCode::ProcessingError => CardDeclined(CardDeclineReason::TryAgain),
            _ => Other(format!("Unexpected error code received: {:?}", error_code))
        }
    }
}

pub async fn create_customer(
    stripe_client: &stripe::Client,
    payment_method_id: PaymentMethodId,
) -> Result<stripe::Customer, SimplifiedStripeError> {
    let mut customer_params = stripe::CreateCustomer::new();
    customer_params.payment_method = Some(payment_method_id);

    stripe::Customer::create(&stripe_client, customer_params).await.map_err(SimplifiedStripeError::from)
}

pub async fn delete_customer(
    stripe_client: &stripe::Client,
    customer_id: &stripe::CustomerId,
) -> Result<(), SimplifiedStripeError> {
    stripe::Customer::delete(stripe_client, customer_id).await?;

    Ok(())
}

pub async fn create_payment_method(
    stripe_client: &stripe::Client,
    card_number: &str,
    exp_month: i32,
    exp_year: i32,
    cvc: &str,
) -> Result<stripe::PaymentMethod, SimplifiedStripeError> {
    let mut payment_method_params = stripe::CreatePaymentMethod::new();
    payment_method_params.type_ = Some(stripe::PaymentMethodTypeFilter::Card);
    payment_method_params.card0 = Some(CardDetailsParams {
        cvc: Some(cvc.to_string()),
        exp_month,
        exp_year,
        number: card_number.to_string()
    });

    stripe::PaymentMethod::create(stripe_client, payment_method_params).await.map_err(SimplifiedStripeError::from)
}

pub async fn create_setup_intent(
    stripe_client: &stripe::Client,
    customer_id: stripe::CustomerId,
    payment_method_id: stripe::PaymentMethodId,
) -> Result<stripe::SetupIntent, SimplifiedStripeError> {
    let mut setup_intent_params = stripe::CreateSetupIntent::new();
    setup_intent_params.customer = Some(customer_id);
    setup_intent_params.payment_method = Some(payment_method_id);
    setup_intent_params.confirm = Some(true);

    let intent_resp = stripe::SetupIntent::create(stripe_client, setup_intent_params).await?;

    match intent_resp.status {
        SetupIntentStatus::Succeeded => Ok(intent_resp),
        _ => {
            Err(Other(format!("Unexpected intent response status: {:?}", intent_resp.status)))
        }
    }
}

pub async fn create_subscription(
    server_state: &ServerState,
    customer_id: stripe::CustomerId,
    payment_method_id: &str,
) -> Result<stripe::Subscription, SimplifiedStripeError> {

    let mut subscription_params = stripe::CreateSubscription::new(customer_id);
    let mut subscription_item_params = stripe::CreateSubscriptionItems::new();
    subscription_item_params.price = Some(Box::new(server_state.config.stripe.premium_price_id.clone()));

    subscription_params.default_payment_method = Some(payment_method_id);
    subscription_params.items = Some(Box::new(vec![subscription_item_params]));
    subscription_params.expand = &["latest_invoice", "latest_invoice.payment_intent"];

    let subscription_resp = stripe::Subscription::create(&server_state.stripe_client, subscription_params).await?;

    match subscription_resp.status {
        SubscriptionStatus::Active => Ok(subscription_resp),
        SubscriptionStatus::Incomplete => match subscription_resp.latest_invoice.as_deref().ok_or_else(|| Other(format!("There is no latest invoice for a recently created subscription: {:?}", subscription_resp)))? {
            Expandable::Id(id) => Err(Other(format!("The latest invoice was expanded, yet returned an id: {:?}", id))),
            Expandable::Object(invoice) => match invoice.payment_intent.as_deref().ok_or_else(|| Other(format!("There is no payment intent for the latest invoice of a subscription: {:?}", subscription_resp)))? {
                Expandable::Id(id) => Err(Other(format!("The payment intent was expanded, yet returned an id: {:?}", id))),
                Expandable::Object(payment_intent) => match payment_intent.status {
                    PaymentIntentStatus::RequiresPaymentMethod => Err(CardDeclined(CardDeclineReason::Generic)),
                    PaymentIntentStatus::RequiresAction => Err(Other(format!("Payment intent requires action to be completed. This is unimplemented. subscription_resp : {:?}", subscription_resp))),
                    _ => Err(Other(format!("Unexpected payment intent failure status: {:?}", subscription_resp))),
                }
            }
        }
        _ => Err(Other(format!("Unexpected subscription resp outcome: {:?}", subscription_resp)))
    }
}

pub async fn detach_payment_method_from_customer(
    stripe_client: &stripe::Client,
    payment_method_id: &stripe::PaymentMethodId,
) -> Result<(), SimplifiedStripeError> {
    stripe::PaymentMethod::detach(stripe_client, payment_method_id).await?;

    Ok(())
}

pub async fn delete_subscription(
    stripe_client: &stripe::Client,
    subscription_id: &stripe::SubscriptionId,
) -> Result<(), SimplifiedStripeError> {
    stripe::Subscription::delete(stripe_client, subscription_id).await?;

    Ok(())
}

const EXPAND_INVOICE_DETAILS: &[&str] = &["subscription"];

pub async fn retrieve_invoice(
    stripe_client: &stripe::Client,
    invoice_id: &stripe::InvoiceId,
) -> Result<stripe::Invoice, SimplifiedStripeError> {
    stripe::Invoice::retrieve(stripe_client, invoice_id, EXPAND_INVOICE_DETAILS).await.map_err(SimplifiedStripeError::from)
}

// pub async fn verify_stripe_webhook_request() -> Result<(), StripeWebhookError> {
//     Ok(())
// }
