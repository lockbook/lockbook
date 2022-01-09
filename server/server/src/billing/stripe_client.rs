use crate::billing::stripe::{
    BasicStripeResponse, SetupPaymentIntentStatus, StripeError, StripeErrorCode,
    StripeKnownErrorCode, StripeKnownErrorDeclineCode, StripePaymentMethodResponse, StripeResult,
    StripeSetupIntentResponse, StripeSubscriptionResponse, SubscriptionStatus,
};
use crate::ServerError::{ClientError, InternalError};
use crate::{ServerError, ServerState};
use lockbook_models::api::{CardDeclinedType, SwitchAccountTierError};
use log::error;
use reqwest::Method;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;

static STRIPE_ENDPOINT: &str = "https://api.stripe.com/v1";
static PAYMENT_METHODS_ENDPOINT: &str = "/payment_methods";
static DETACH_ENDPOINT: &str = "/detach";
static ATTACH_ENDPOINT: &str = "/attach";
static CUSTOMER_ENDPOINT: &str = "/customers";
static SUBSCRIPTIONS_ENDPOINT: &str = "/subscriptions";
static SETUP_INTENTS_ENDPOINT: &str = "/setup_intents";

pub async fn create_customer(
    server_state: &ServerState,
) -> Result<String, ServerError<SwitchAccountTierError>> {
    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT),
        Method::POST,
        None,
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp.id),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst creating an account: {:?}",
            e
        ))),
    }
}

pub async fn delete_customer(
    server_state: &ServerState,
    customer_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}/{}", STRIPE_ENDPOINT, CUSTOMER_ENDPOINT, customer_id),
        Method::DELETE,
        None,
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst deleting an account: {:?}",
            e
        ))),
    }
}

pub async fn create_payment_method(
    server_state: &ServerState,
    card_number: &str,
    card_exp_year: &str,
    card_exp_month: &str,
    card_cvc: &str,
) -> Result<StripePaymentMethodResponse, ServerError<SwitchAccountTierError>> {
    let mut payment_method_form = HashMap::new();
    payment_method_form.insert("type", "card");
    payment_method_form.insert("card[number]", card_number);
    payment_method_form.insert("card[exp_month]", card_exp_year);
    payment_method_form.insert("card[exp_year]", card_exp_month);
    payment_method_form.insert("card[cvc]", card_cvc);

    match send_stripe_request::<StripePaymentMethodResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT),
        Method::POST,
        Some(payment_method_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => Ok(resp),
        StripeResult::Err(e) => Err(match_stripe_error(e.error).await),
    }
}

pub async fn create_setup_intent(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let mut create_setup_intent_form = HashMap::new();
    create_setup_intent_form.insert("customer", customer_id);
    create_setup_intent_form.insert("payment_method", payment_method_id);
    create_setup_intent_form.insert("confirm", "true");

    match send_stripe_request::<StripeSetupIntentResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, SETUP_INTENTS_ENDPOINT),
        Method::POST,
        Some(create_setup_intent_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => match resp.status {
            SetupPaymentIntentStatus::Succeeded => Ok(()),
            SetupPaymentIntentStatus::RequiresAction => Err(InternalError(format!(
                "Verification required for a card (potentially a european user): {:?}",
                resp
            ))),
            SetupPaymentIntentStatus::RequiresPaymentMethod => match resp.last_setup_error {
                None => Err(InternalError(format!(
                    "Cannot view stripe's setup intent error despite having a related status: {:?}",
                    resp
                ))),
                Some(e) => match match_stripe_error(e).await {
                    ClientError(e) => {
                        delete_customer(server_state, customer_id).await?;
                        Err(ClientError(e))
                    }
                    e => Err(e),
                },
            },
        },
        StripeResult::Err(e) => match match_stripe_error(e.error).await {
            ClientError(e) => {
                delete_customer(server_state, customer_id).await?;
                Err(ClientError(e))
            }
            e => Err(e),
        },
    }
}

pub async fn attach_payment_method_to_customer(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    let mut attach_payment_method_form = HashMap::new();
    attach_payment_method_form.insert("customer", customer_id);

    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!(
            "{}{}/{}{}",
            STRIPE_ENDPOINT, PAYMENT_METHODS_ENDPOINT, payment_method_id, ATTACH_ENDPOINT
        ),
        Method::POST,
        Some(attach_payment_method_form),
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(match_stripe_error(e.error).await),
    }
}

pub async fn create_subscription(
    server_state: &ServerState,
    customer_id: &str,
    payment_method_id: &str,
) -> Result<String, ServerError<SwitchAccountTierError>> {
    let mut create_subscription_form = HashMap::new();
    create_subscription_form.insert("customer", customer_id);
    create_subscription_form.insert(
        "items[0][price]",
        server_state.config.stripe.premium_price_id.as_str(),
    );
    create_subscription_form.insert("default_payment_method", payment_method_id);
    create_subscription_form.insert("expand[]", "latest_invoice");
    create_subscription_form.insert("expand[]", "latest_invoice.payment_intent");

    match send_stripe_request::<StripeSubscriptionResponse, SwitchAccountTierError>(
        server_state,
        format!("{}{}", STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT),
        Method::POST,
        Some(create_subscription_form),
    )
    .await?
    {
        StripeResult::Ok(resp) => match resp.status {
            SubscriptionStatus::Active => Ok(resp.id),
            SubscriptionStatus::Incomplete => match resp.latest_invoice.payment_intent.status {
                SetupPaymentIntentStatus::Succeeded => Err(InternalError(format!("Unexpected stripe payment intent status: {:?}", resp))),
                SetupPaymentIntentStatus::RequiresPaymentMethod => match resp.latest_invoice.payment_intent.last_payment_error {
                    None => Err(InternalError(format!("Cannot view stripe's payment intent error despite having a related status: {:?}", resp))),
                    Some(e) => Err(match_stripe_error(e).await)
                }
                SetupPaymentIntentStatus::RequiresAction => Err(InternalError(format!("Cannot verify payment method whilst creating subscription: {:?}", resp))),
            }
            _ => Err(InternalError(format!("Unexpected subscription status (considering payment method has already been checked): {:?}", resp)))
        },
        StripeResult::Err(e) => Err(match_stripe_error(e.error).await)
    }
}

pub async fn detach_payment_method_from_customer(
    server_state: &ServerState,
    payment_method_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!(
            "{}/{}{}",
            STRIPE_ENDPOINT, payment_method_id, DETACH_ENDPOINT
        ),
        Method::POST,
        None,
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst detaching a payment method from a customer: {:?}",
            e
        ))),
    }
}

pub async fn delete_subscription(
    server_state: &ServerState,
    subscription_id: &str,
) -> Result<(), ServerError<SwitchAccountTierError>> {
    match send_stripe_request::<BasicStripeResponse, SwitchAccountTierError>(
        server_state,
        format!(
            "{}{}/{}",
            STRIPE_ENDPOINT, SUBSCRIPTIONS_ENDPOINT, subscription_id
        ),
        Method::DELETE,
        None,
    )
    .await?
    {
        StripeResult::Ok(_) => Ok(()),
        StripeResult::Err(e) => Err(InternalError(format!(
            "Stripe returned an error whilst deleting a subscription: {:?}",
            e
        ))),
    }
}

async fn send_stripe_request<U: DeserializeOwned, E: Debug>(
    server_state: &ServerState,
    url: String,
    method: Method,
    maybe_form: Option<HashMap<&str, &str>>,
) -> Result<StripeResult<U>, ServerError<E>> {
    let request = server_state
        .stripe_client
        .request(method, &url)
        .basic_auth::<&str, &str>(&server_state.config.stripe.stripe_secret, None);

    if let Some(form) = maybe_form {
        request.form(&form)
    } else {
        request
    }
    .send()
    .await
    .map_err(|e| {
        InternalError(format!(
            "Cannot make stripe request at '{}' due to reqwest error: {:?}",
            url, e
        ))
    })?
    .json::<StripeResult<U>>()
    .await
    .map_err(|e| InternalError(format!("Cannot parse stripe request at '{}': {:?}", url, e)))
}

async fn match_stripe_error(error: StripeError) -> ServerError<SwitchAccountTierError> {
    match error.code {
        StripeErrorCode::Known(ref error_code) => match error_code {
            StripeKnownErrorCode::CardDeclineRateLimitExceeded => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::TooManyTries),
            ),
            StripeKnownErrorCode::CardDeclined => match error.decline_code {
                None => {
                    error!("Although stripe error code being `card_declined`, there seems to be no decline code: {:?}", error);

                    ClientError(SwitchAccountTierError::CardDeclined(
                        CardDeclinedType::Generic,
                    ))
                }
                Some(ref decline_code) => match decline_code {
                    StripeErrorCode::Known(ref known_decline_code) => match known_decline_code {
                        // Try again
                        StripeKnownErrorDeclineCode::ApproveWithId
                        | StripeKnownErrorDeclineCode::IssuerNotAvailable
                        | StripeKnownErrorDeclineCode::ProcessingError
                        | StripeKnownErrorDeclineCode::ReenterTransaction
                        | StripeKnownErrorDeclineCode::TryAgainLater => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::TryAgain),
                        ),

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
                        | StripeKnownErrorDeclineCode::TransactionNotAllowed => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::Unknown),
                        ),

                        // Not supported
                        StripeKnownErrorDeclineCode::CardNotSupported
                        | StripeKnownErrorDeclineCode::CurrencyNotSupported => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::NotSupported),
                        ),

                        // Balance or credit exceeded
                        StripeKnownErrorDeclineCode::CardVelocityExceeded
                        | StripeKnownErrorDeclineCode::InsufficientFunds
                        | StripeKnownErrorDeclineCode::WithdrawalCountLimitExceeded => {
                            ClientError(SwitchAccountTierError::CardDeclined(
                                CardDeclinedType::BalanceOrCreditExceeded,
                            ))
                        }

                        // Expired card
                        StripeKnownErrorDeclineCode::ExpiredCard => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::ExpiredCard),
                        ),

                        // Generic
                        StripeKnownErrorDeclineCode::Fraudulent
                        | StripeKnownErrorDeclineCode::GenericDecline
                        | StripeKnownErrorDeclineCode::LostCard
                        | StripeKnownErrorDeclineCode::MerchantBlacklist
                        | StripeKnownErrorDeclineCode::NoActionTaken
                        | StripeKnownErrorDeclineCode::NotPermitted
                        | StripeKnownErrorDeclineCode::PickupCard
                        | StripeKnownErrorDeclineCode::StolenCard => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::Generic),
                        ),

                        // Incorrect number
                        StripeKnownErrorDeclineCode::IncorrectNumber
                        | StripeKnownErrorDeclineCode::InvalidNumber => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectNumber),
                        ),

                        // Incorrect cvc
                        StripeKnownErrorDeclineCode::IncorrectCvc
                        | StripeKnownErrorDeclineCode::InvalidCvc => ClientError(
                            SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectCVC),
                        ),

                        // Incorrect expiry month
                        StripeKnownErrorDeclineCode::InvalidExpiryMonth => {
                            ClientError(SwitchAccountTierError::CardDeclined(
                                CardDeclinedType::IncorrectExpiryMonth,
                            ))
                        }

                        // Incorrect expiry year
                        StripeKnownErrorDeclineCode::InvalidExpiryYear => {
                            ClientError(SwitchAccountTierError::CardDeclined(
                                CardDeclinedType::IncorrectExpiryYear,
                            ))
                        }
                    },
                    StripeErrorCode::Unknown(_) => InternalError(format!(
                        "Unexpected stripe decline error code encountered: {:?}",
                        error
                    )),
                },
            },
            StripeKnownErrorCode::ExpiredCard => ClientError(SwitchAccountTierError::CardDeclined(
                CardDeclinedType::ExpiredCard,
            )),
            StripeKnownErrorCode::IncorrectCvc | StripeKnownErrorCode::InvalidCvc => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectCVC),
            ),
            StripeKnownErrorCode::IncorrectNumber | StripeKnownErrorCode::InvalidNumber => {
                ClientError(SwitchAccountTierError::CardDeclined(
                    CardDeclinedType::IncorrectNumber,
                ))
            }
            StripeKnownErrorCode::InsufficientFunds => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::BalanceOrCreditExceeded),
            ),
            StripeKnownErrorCode::InvalidExpiryMonth => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectExpiryMonth),
            ),
            StripeKnownErrorCode::InvalidExpiryYear => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectExpiryYear),
            ),
            StripeKnownErrorCode::ProcessingError => ClientError(
                SwitchAccountTierError::CardDeclined(CardDeclinedType::TryAgain),
            ),
            StripeKnownErrorCode::SetupIntentAuthenticationFailure
            | StripeKnownErrorCode::PaymentIntentAuthenticationFailure => {
                InternalError(format!("Stripe authentication error: {:?}", error))
            }
        },
        StripeErrorCode::Unknown(_) => InternalError(format!(
            "Unknown stripe error code encountered: {:?}",
            error
        )),
    }
}
