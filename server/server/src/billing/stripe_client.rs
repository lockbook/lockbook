use crate::billing::stripe_client::SimplifiedStripeError::{
    CardDeclined, InvalidCreditCard, Other,
};

use crate::{ServerState, StripeDeclineCodeCatcher, StripeKnownDeclineCode};
use lockbook_models::api::{CardDeclineReason, CardRejectReason};
use log::{error, info};
use std::fmt::Debug;
use stripe::{
    CardDetailsParams, ErrorCode, Expandable, ParseIdError, PaymentIntentStatus, SetupIntentStatus,
    StripeError, SubscriptionStatus,
};

#[derive(Debug)]
pub enum SimplifiedStripeError {
    CardDeclined(CardDeclineReason),
    InvalidCreditCard(CardRejectReason),
    Other(String),
}

impl From<StripeError> for SimplifiedStripeError {
    fn from(e: StripeError) -> Self {
        match e {
            StripeError::Stripe(stripe_error) => {
                simplify_stripe_error(stripe_error.code, stripe_error.decline_code)
            }
            _ => SimplifiedStripeError::Other(format!(
                "Unexpected stripe error was encountered: {:?}",
                e
            )),
        }
    }
}

impl From<stripe::ParseIdError> for SimplifiedStripeError {
    fn from(e: ParseIdError) -> Self {
        Other(format!("Stripe parsing error: {:?}", e))
    }
}

fn simplify_stripe_error(
    error_code: Option<stripe::ErrorCode>, maybe_decline_code: Option<String>,
) -> SimplifiedStripeError {
    match error_code {
        None => {
            Other(format!("stripe error with no details: error_code: {:?}, decline_code: {:?}", err_code, maybe_decline_code))
        },
        Some(error_code) => match error_code {
            ErrorCode::BalanceInsufficient => CardDeclined(CardDeclineReason::BalanceOrCreditExceeded),
            ErrorCode::CardDeclined => match maybe_decline_code {
                None => CardDeclined(CardDeclineReason::Generic),
                Some(decline_code) => {
                    match serde_json::from_str::<StripeDeclineCodeCatcher>(&format!("\"{}\"", decline_code)).map_err(|e| SimplifiedStripeError::Other(format!("An error was encountered while serializing decline code: {:?}", e))) {
                        Ok(StripeDeclineCodeCatcher::Unknown(unknown_decline_code)) => {
                            error!("Unknown decline code from stripe: {}", unknown_decline_code);
                            CardDeclined(CardDeclineReason::Generic)
                        }
                        Ok(StripeDeclineCodeCatcher::Known(decline_code)) => match decline_code {
                            // Try again
                            StripeKnownDeclineCode::ApproveWithId
                            | StripeKnownDeclineCode::IssuerNotAvailable
                            | StripeKnownDeclineCode::ProcessingError
                            | StripeKnownDeclineCode::ReenterTransaction
                            | StripeKnownDeclineCode::TryAgainLater => {
                                CardDeclined(CardDeclineReason::TryAgain)
                            }

                            // Not supported
                            StripeKnownDeclineCode::CardNotSupported
                            | StripeKnownDeclineCode::CurrencyNotSupported => {
                                CardDeclined(CardDeclineReason::NotSupported)
                            }

                            // Balance or credit exceeded
                            StripeKnownDeclineCode::CardVelocityExceeded
                            | StripeKnownDeclineCode::InsufficientFunds
                            | StripeKnownDeclineCode::WithdrawalCountLimitExceeded => {
                                CardDeclined(CardDeclineReason::BalanceOrCreditExceeded)
                            }

                            // Expired card
                            StripeKnownDeclineCode::ExpiredCard => {
                                CardDeclined(CardDeclineReason::ExpiredCard)
                            }

                            // Generic
                            StripeKnownDeclineCode::CallIssuer
                            | StripeKnownDeclineCode::DoNotTryAgain
                            | StripeKnownDeclineCode::DoNotHonor
                            | StripeKnownDeclineCode::NewAccountInformationAvailable
                            | StripeKnownDeclineCode::RestrictedCard
                            | StripeKnownDeclineCode::RevocationOfAllAuthorizations
                            | StripeKnownDeclineCode::RevocationOfAuthorization
                            | StripeKnownDeclineCode::SecurityViolation
                            | StripeKnownDeclineCode::ServiceNotAllowed
                            | StripeKnownDeclineCode::StopPaymentOrder
                            | StripeKnownDeclineCode::TransactionNotAllowed
                            | StripeKnownDeclineCode::Fraudulent
                            | StripeKnownDeclineCode::GenericDecline
                            | StripeKnownDeclineCode::LostCard
                            | StripeKnownDeclineCode::MerchantBlacklist
                            | StripeKnownDeclineCode::NoActionTaken
                            | StripeKnownDeclineCode::NotPermitted
                            | StripeKnownDeclineCode::PickupCard
                            | StripeKnownDeclineCode::StolenCard => {
                                CardDeclined(CardDeclineReason::Generic)
                            }

                            // Incorrect number
                            StripeKnownDeclineCode::IncorrectNumber
                            | StripeKnownDeclineCode::InvalidNumber => {
                                InvalidCreditCard(CardRejectReason::Number)
                            }

                            // Incorrect cvc
                            StripeKnownDeclineCode::IncorrectCvc
                            | StripeKnownDeclineCode::InvalidCvc => {
                                InvalidCreditCard(CardRejectReason::CVC)
                            }

                            // Incorrect expiry month
                            StripeKnownDeclineCode::InvalidExpiryMonth => {
                                InvalidCreditCard(CardRejectReason::ExpMonth)
                            }

                            // Incorrect expiry year
                            StripeKnownDeclineCode::InvalidExpiryYear => {
                                InvalidCreditCard(CardRejectReason::ExpYear)
                            }
                        }
                        Err(e) => e,
                    }
                }
            }
            ErrorCode::ExpiredCard => CardDeclined(CardDeclineReason::ExpiredCard),
            ErrorCode::InvalidCardType => CardDeclined(CardDeclineReason::NotSupported),
            ErrorCode::InvalidCvc | ErrorCode::IncorrectCvc => InvalidCreditCard(CardRejectReason::CVC),
            ErrorCode::InvalidExpiryMonth => InvalidCreditCard(CardRejectReason::ExpMonth),
            ErrorCode::InvalidExpiryYear => InvalidCreditCard(CardRejectReason::ExpYear),
            ErrorCode::InvalidNumber | ErrorCode::IncorrectNumber => InvalidCreditCard(CardRejectReason::Number),
            ErrorCode::ProcessingError => CardDeclined(CardDeclineReason::TryAgain),
            _ => Other(format!("Unexpected error code received: {:?}", error_code))
        }
    }
}

pub async fn create_customer(
    stripe_client: &stripe::Client, customer_name: &str, payment_method_id: stripe::PaymentMethodId,
) -> Result<stripe::Customer, SimplifiedStripeError> {
    info!("Creating stripe customer. payment_method_id: {}", payment_method_id.as_str());

    let mut customer_params = stripe::CreateCustomer::new();
    customer_params.payment_method = Some(payment_method_id);
    customer_params.name = Some(customer_name);

    stripe::Customer::create(stripe_client, customer_params)
        .await
        .map_err(SimplifiedStripeError::from)
}

pub async fn create_payment_method(
    stripe_client: &stripe::Client, card_number: &str, exp_month: i32, exp_year: i32, cvc: &str,
) -> Result<stripe::PaymentMethod, SimplifiedStripeError> {
    let mut payment_method_params = stripe::CreatePaymentMethod::new();
    payment_method_params.type_ = Some(stripe::PaymentMethodTypeFilter::Card);
    payment_method_params.card =
        Some(stripe::CreatePaymentMethodCardUnion::CardDetailsParams(CardDetailsParams {
            cvc: Some(cvc.to_string()),
            exp_month,
            exp_year,
            number: card_number.to_string(),
        }));

    stripe::PaymentMethod::create(stripe_client, payment_method_params)
        .await
        .map_err(SimplifiedStripeError::from)
}

pub async fn create_setup_intent(
    stripe_client: &stripe::Client, customer_id: stripe::CustomerId,
    payment_method_id: stripe::PaymentMethodId,
) -> Result<stripe::SetupIntent, SimplifiedStripeError> {
    info!(
        "Creating stripe setup intent. customer_id: {}, payment_method_id {}",
        customer_id.as_str(),
        payment_method_id.as_str()
    );

    let mut setup_intent_params = stripe::CreateSetupIntent::new();
    setup_intent_params.customer = Some(customer_id);
    setup_intent_params.payment_method = Some(payment_method_id);
    setup_intent_params.confirm = Some(true);

    let intent_resp = stripe::SetupIntent::create(stripe_client, setup_intent_params).await?;

    match intent_resp.status {
        SetupIntentStatus::Succeeded => Ok(intent_resp),
        _ => Err(Other(format!("Unexpected intent response status: {:?}", intent_resp.status))),
    }
}

pub async fn create_subscription(
    server_state: &ServerState, customer_id: stripe::CustomerId, payment_method_id: &str,
) -> Result<stripe::Subscription, SimplifiedStripeError> {
    info!(
        "Creating stripe subscription. customer_id: {}, payment_method_id: {}",
        customer_id.as_str(),
        payment_method_id
    );

    let mut subscription_params = stripe::CreateSubscription::new(customer_id);
    let mut subscription_item_params = stripe::CreateSubscriptionItems::new();
    subscription_item_params.price = Some(server_state.config.stripe.premium_price_id.clone());

    subscription_params.default_payment_method = Some(payment_method_id);
    subscription_params.items = Some(vec![subscription_item_params]);
    subscription_params.expand = &["latest_invoice", "latest_invoice.payment_intent"];

    let subscription_resp =
        stripe::Subscription::create(&server_state.stripe_client, subscription_params).await?;

    match subscription_resp.status {
        SubscriptionStatus::Active => Ok(subscription_resp),
        SubscriptionStatus::Incomplete => match subscription_resp.latest_invoice.as_ref().ok_or_else(|| Other(format!("There is no latest invoice for a subscription: {:?}", subscription_resp)))? {
            Expandable::Id(id) => Err(Other(format!("The latest invoice was expanded, yet returned an id: {:?}", id))),
            Expandable::Object(invoice) => match invoice.payment_intent.as_ref().ok_or_else(|| Other(format!("no payment intent for latest subscription invoice: {:?}", subscription_resp)))? {
                Expandable::Id(id) => Err(Other(format!("payment intent expanded yet returned an id: {:?}", id))),
                Expandable::Object(payment_intent) => match payment_intent.status {
                    PaymentIntentStatus::RequiresPaymentMethod => Err(CardDeclined(CardDeclineReason::Generic)),
                    PaymentIntentStatus::RequiresAction => Err(Other(format!("Payment intent requires additional action to be completed. This is unimplemented. subscription_resp: {:?}", subscription_resp))),
                    _ => Err(Other(format!("Unexpected payment intent failure status: {:?}", subscription_resp))),
                }
            }
        }
        _ => Err(Other(format!("Unexpected subscription response: {:?}", subscription_resp)))
    }
}

pub async fn detach_payment_method_from_customer(
    stripe_client: &stripe::Client, payment_method_id: &stripe::PaymentMethodId,
) -> Result<(), SimplifiedStripeError> {
    info!("Detaching stripe payment method. payment_method_id: {}", payment_method_id.as_str());

    stripe::PaymentMethod::detach(stripe_client, payment_method_id).await?;
    Ok(())
}

pub async fn cancel_subscription(
    stripe_client: &stripe::Client, subscription_id: &stripe::SubscriptionId,
) -> Result<(), SimplifiedStripeError> {
    info!("Cancelling stripe subscription. subscription_id: {}", subscription_id.as_str());

    stripe::Subscription::cancel(
        stripe_client,
        subscription_id,
        stripe::CancelSubscription::default(),
    )
    .await?;

    Ok(())
}

const EXPAND_INVOICE_DETAILS: &[&str] = &["subscription"];

pub async fn retrieve_invoice(
    stripe_client: &stripe::Client, invoice_id: &stripe::InvoiceId,
) -> Result<stripe::Invoice, SimplifiedStripeError> {
    info!("Getting stripe invoice. invoice_id: {}", invoice_id.as_str());

    stripe::Invoice::retrieve(stripe_client, invoice_id, EXPAND_INVOICE_DETAILS)
        .await
        .map_err(SimplifiedStripeError::from)
}
