use hmac::{Hmac, Mac};
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fmt::{self, Display};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

const API_BASE: &str = "https://api.stripe.com/v1";
const API_VERSION: &str = "2023-10-16";
const WEBHOOK_TOLERANCE_SECS: i64 = 300;

#[derive(Debug)]
pub struct ParseIdError {
    pub expected_prefixes: &'static [&'static str],
}

macro_rules! id_type {
    ($name:ident, [$($prefix:literal),+]) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if [$($prefix),+].iter().any(|prefix| s.starts_with(prefix)) {
                    Ok(Self(s.to_string()))
                } else {
                    Err(ParseIdError { expected_prefixes: &[$($prefix),+] })
                }
            }
        }
    };
}

macro_rules! strict_id_deserialize {
    ($name:ident) => {
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(|e| D::Error::custom(format!("{e:?}")))
            }
        }
    };
}

id_type!(CustomerId, ["cus_"]);
id_type!(PaymentMethodId, ["pm_", "card_", "src_", "ba_"]);
id_type!(SubscriptionId, ["sub_"]);
id_type!(InvoiceId, ["in_"]);
strict_id_deserialize!(CustomerId);
strict_id_deserialize!(PaymentMethodId);
strict_id_deserialize!(SubscriptionId);

impl<'de> Deserialize<'de> for InvoiceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Option::<String>::deserialize(deserializer)? {
            None => Ok(InvoiceId(String::new())),
            Some(s) if s.is_empty() => Ok(InvoiceId(s)),
            Some(s) => s.parse().map_err(|e| D::Error::custom(format!("{e:?}"))),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Expandable<T: Object> {
    Id(T::Id),
    Object(Box<T>),
}

pub trait Object {
    type Id: DeserializeOwned;
}

impl<'de, T: Object + DeserializeOwned> Deserialize<'de> for Expandable<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_object() {
            serde_json::from_value(value)
                .map(|object| Expandable::Object(Box::new(object)))
                .map_err(D::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Expandable::Id)
                .map_err(D::Error::custom)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Customer {
    pub id: CustomerId,
}

impl Object for Customer {
    type Id = CustomerId;
}

#[derive(Clone, Debug, Deserialize)]
pub struct Card {
    pub last4: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PaymentMethod {
    pub id: PaymentMethodId,
    pub card: Option<Card>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupIntentStatus {
    Succeeded,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetupIntent {
    pub id: String,
    pub status: SetupIntentStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIntentStatus {
    RequiresPaymentMethod,
    RequiresAction,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PaymentIntent {
    pub status: PaymentIntentStatus,
}

impl Object for PaymentIntent {
    type Id = String;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Incomplete,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub status: SubscriptionStatus,
    pub current_period_end: i64,
    #[serde(default)]
    pub latest_invoice: Option<Expandable<Invoice>>,
}

impl Object for Subscription {
    type Id = SubscriptionId;
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceBillingReason {
    SubscriptionCycle,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Invoice {
    pub id: InvoiceId,
    #[serde(default)]
    pub customer: Option<Expandable<Customer>>,
    #[serde(default)]
    pub billing_reason: Option<InvoiceBillingReason>,
    #[serde(default)]
    pub subscription: Option<Expandable<Subscription>>,
    #[serde(default)]
    pub payment_intent: Option<Expandable<PaymentIntent>>,
}

impl Object for Invoice {
    type Id = InvoiceId;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    BalanceInsufficient,
    CardDeclined,
    ExpiredCard,
    InvalidCardType,
    InvalidCvc,
    IncorrectCvc,
    InvalidExpiryMonth,
    InvalidExpiryYear,
    InvalidNumber,
    IncorrectNumber,
    ProcessingError,
    Other(String),
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Ok(match code.as_str() {
            "balance_insufficient" => ErrorCode::BalanceInsufficient,
            "card_declined" => ErrorCode::CardDeclined,
            "expired_card" => ErrorCode::ExpiredCard,
            "invalid_card_type" => ErrorCode::InvalidCardType,
            "invalid_cvc" => ErrorCode::InvalidCvc,
            "incorrect_cvc" => ErrorCode::IncorrectCvc,
            "invalid_expiry_month" => ErrorCode::InvalidExpiryMonth,
            "invalid_expiry_year" => ErrorCode::InvalidExpiryYear,
            "invalid_number" => ErrorCode::InvalidNumber,
            "incorrect_number" => ErrorCode::IncorrectNumber,
            "processing_error" => ErrorCode::ProcessingError,
            _ => ErrorCode::Other(code),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct RequestError {
    #[serde(skip)]
    pub http_status: u16,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
    pub code: Option<ErrorCode>,
    pub decline_code: Option<String>,
}

#[derive(Debug)]
pub enum StripeError {
    Stripe(RequestError),
    Http(reqwest::Error),
    Parse(serde_json::Error),
}

impl From<reqwest::Error> for StripeError {
    fn from(error: reqwest::Error) -> Self {
        StripeError::Http(error)
    }
}

impl From<serde_json::Error> for StripeError {
    fn from(error: serde_json::Error) -> Self {
        StripeError::Parse(error)
    }
}

type Form = Vec<(String, String)>;

fn field(name: &str, value: impl Into<String>) -> (String, String) {
    (name.to_string(), value.into())
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    secret: String,
}

impl Client {
    pub fn new(secret: impl Into<String>) -> Self {
        let mut builder = reqwest::Client::builder()
            .tls_built_in_root_certs(false)
            .pool_max_idle_per_host(0);
        let mut added = 0;
        for cert in rustls_native_certs::load_native_certs().certs {
            if let Ok(cert) = reqwest::Certificate::from_der(&cert) {
                builder = builder.add_root_certificate(cert);
                added += 1;
            }
        }
        assert!(added > 0, "no CA certificates found");
        Self { http: builder.build().unwrap(), secret: secret.into() }
    }

    async fn send<T: DeserializeOwned>(
        &self, method: reqwest::Method, path: &str, form: Form,
    ) -> Result<T, StripeError> {
        let mut request = self
            .http
            .request(method.clone(), format!("{API_BASE}{path}"))
            .bearer_auth(&self.secret)
            .header("Stripe-Version", API_VERSION);

        if !form.is_empty() {
            request = if method == reqwest::Method::POST {
                request.form(&form)
            } else {
                request.query(&form)
            };
        }

        let response = request.send().await?;
        let status = response.status();
        let body = response.bytes().await?;

        if status.is_success() {
            Ok(serde_json::from_slice(&body)?)
        } else {
            #[derive(Deserialize)]
            struct Envelope {
                error: RequestError,
            }
            let mut error = serde_json::from_slice::<Envelope>(&body)?.error;
            error.http_status = status.as_u16();
            Err(StripeError::Stripe(error))
        }
    }

    pub async fn create_customer(
        &self, name: &str, payment_method: &PaymentMethodId,
    ) -> Result<Customer, StripeError> {
        let form = vec![field("name", name), field("payment_method", payment_method.as_str())];
        self.send(reqwest::Method::POST, "/customers", form).await
    }

    pub async fn create_card_payment_method(
        &self, number: &str, exp_month: i32, exp_year: i32, cvc: &str,
    ) -> Result<PaymentMethod, StripeError> {
        let form = vec![
            field("type", "card"),
            field("card[cvc]", cvc),
            field("card[exp_month]", exp_month.to_string()),
            field("card[exp_year]", exp_year.to_string()),
            field("card[number]", number),
        ];
        self.send(reqwest::Method::POST, "/payment_methods", form)
            .await
    }

    pub async fn create_confirmed_setup_intent(
        &self, customer: &CustomerId, payment_method: &PaymentMethodId,
    ) -> Result<SetupIntent, StripeError> {
        let form = vec![
            field("customer", customer.as_str()),
            field("payment_method", payment_method.as_str()),
            field("confirm", "true"),
            field("automatic_payment_methods[allow_redirects]", "never"),
            field("automatic_payment_methods[enabled]", "true"),
        ];
        self.send(reqwest::Method::POST, "/setup_intents", form)
            .await
    }

    pub async fn create_subscription(
        &self, customer: &CustomerId, payment_method: &str, price: &str,
    ) -> Result<Subscription, StripeError> {
        let form = vec![
            field("customer", customer.as_str()),
            field("default_payment_method", payment_method),
            field("items[0][price]", price),
            field("expand[0]", "latest_invoice"),
            field("expand[1]", "latest_invoice.payment_intent"),
        ];
        self.send(reqwest::Method::POST, "/subscriptions", form)
            .await
    }

    pub async fn detach_payment_method(
        &self, payment_method: &PaymentMethodId,
    ) -> Result<PaymentMethod, StripeError> {
        let path = format!("/payment_methods/{payment_method}/detach");
        self.send(reqwest::Method::POST, &path, Vec::new()).await
    }

    pub async fn cancel_subscription(
        &self, subscription: &SubscriptionId,
    ) -> Result<Subscription, StripeError> {
        let path = format!("/subscriptions/{subscription}");
        self.send(reqwest::Method::DELETE, &path, Vec::new()).await
    }

    pub async fn get_subscription(
        &self, subscription: &SubscriptionId,
    ) -> Result<Subscription, StripeError> {
        let path = format!("/subscriptions/{subscription}");
        self.send(reqwest::Method::GET, &path, Vec::new()).await
    }

    pub async fn get_invoice(
        &self, invoice: &InvoiceId, expand: &[&str],
    ) -> Result<Invoice, StripeError> {
        let path = format!("/invoices/{invoice}");
        let form = expand
            .iter()
            .enumerate()
            .map(|(i, name)| field(&format!("expand[{i}]"), *name))
            .collect();
        self.send(reqwest::Method::GET, &path, form).await
    }
}

#[derive(Debug)]
pub enum WebhookError {
    BadKey,
    BadHeader(std::num::ParseIntError),
    BadSignature,
    BadTimestamp(i64),
    BadParse(serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventType {
    InvoicePaymentFailed,
    InvoicePaid,
    Other(String),
}

#[derive(Clone, Debug)]
pub enum EventObject {
    Invoice(Box<Invoice>),
    Other(serde_json::Value),
}

#[derive(Clone, Debug)]
pub struct EventData {
    pub object: EventObject,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub type_: EventType,
    pub data: EventData,
}

const KNOWN_OBJECT_KINDS: &[&str] = &[
    "account",
    "capability",
    "application",
    "application_fee",
    "fee_refund",
    "balance",
    "bank_account",
    "billing_portal.configuration",
    "card",
    "charge",
    "checkout.session",
    "coupon",
    "customer",
    "discount",
    "dispute",
    "file",
    "invoice",
    "invoiceitem",
    "issuing.authorization",
    "issuing.card",
    "issuing.cardholder",
    "issuing.dispute",
    "issuing.transaction",
    "mandate",
    "payment_intent",
    "payment_link",
    "payment_method",
    "payout",
    "person",
    "plan",
    "price",
    "product",
    "promotion_code",
    "quote",
    "refund",
    "review",
    "setup_intent",
    "subscription",
    "subscription_schedule",
    "tax_id",
    "tax_rate",
    "test_helpers.test_clock",
    "topup",
    "transfer",
];

pub struct Webhook;

impl Webhook {
    pub fn construct_event(
        payload: &str, signature_header: &str, secret: &str,
    ) -> Result<Event, WebhookError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
        Self::construct_event_at(payload, signature_header, secret, now)
    }

    fn construct_event_at(
        payload: &str, signature_header: &str, secret: &str, now: i64,
    ) -> Result<Event, WebhookError> {
        let fields: std::collections::HashMap<&str, &str> = signature_header
            .split(',')
            .filter_map(|field| {
                let mut key_and_value = field.split('=');
                Some((key_and_value.next()?, key_and_value.next()?))
            })
            .collect();
        let timestamp = fields.get("t").ok_or(WebhookError::BadSignature)?;
        let signature = fields.get("v1").ok_or(WebhookError::BadSignature)?;
        let timestamp = timestamp.parse::<i64>().map_err(WebhookError::BadHeader)?;

        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| WebhookError::BadKey)?;
        mac.update(format!("{timestamp}.{payload}").as_bytes());
        let signature = hex::decode(signature).map_err(|_| WebhookError::BadSignature)?;
        mac.verify_slice(&signature)
            .map_err(|_| WebhookError::BadSignature)?;

        if (now - timestamp).abs() > WEBHOOK_TOLERANCE_SECS {
            return Err(WebhookError::BadTimestamp(timestamp));
        }

        Self::parse_event(payload)
    }

    fn parse_event(payload: &str) -> Result<Event, WebhookError> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(rename = "type")]
            type_: String,
            data: RawData,
        }
        #[derive(Deserialize)]
        struct RawData {
            object: serde_json::Value,
        }

        let raw: Raw = serde_json::from_str(payload).map_err(WebhookError::BadParse)?;
        let kind = raw.data.object.get("object").and_then(|kind| kind.as_str());
        if !kind.is_some_and(|kind| KNOWN_OBJECT_KINDS.contains(&kind)) {
            return Err(WebhookError::BadParse(<serde_json::Error as serde::de::Error>::custom(
                format!("unknown event object {kind:?}"),
            )));
        }
        let (type_, object) = match raw.type_.as_str() {
            "invoice.payment_failed" => (
                EventType::InvoicePaymentFailed,
                EventObject::Invoice(Box::new(
                    serde_json::from_value(raw.data.object).map_err(WebhookError::BadParse)?,
                )),
            ),
            "invoice.paid" => (
                EventType::InvoicePaid,
                EventObject::Invoice(Box::new(
                    serde_json::from_value(raw.data.object).map_err(WebhookError::BadParse)?,
                )),
            ),
            other => (EventType::Other(other.to_string()), EventObject::Other(raw.data.object)),
        };
        Ok(Event { type_, data: EventData { object } })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test";
    const PAYLOAD: &str = r#"{"type":"invoice.paid","data":{"object":{"object":"invoice","id":"in_1","customer":"cus_1","billing_reason":"subscription_cycle","subscription":"sub_1"}}}"#;

    fn header(timestamp: i64, payload: &str, secret: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{timestamp}.{payload}").as_bytes());
        format!("t={timestamp},v1={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn valid_invoice_paid_event() {
        let header = header(1000, PAYLOAD, SECRET);
        let event = Webhook::construct_event_at(PAYLOAD, &header, SECRET, 1010).unwrap();
        assert_eq!(event.type_, EventType::InvoicePaid);
        let EventObject::Invoice(invoice) = event.data.object else { panic!() };
        assert_eq!(invoice.billing_reason, Some(InvoiceBillingReason::SubscriptionCycle));
        assert!(matches!(invoice.customer, Some(Expandable::Id(_))));
        assert!(matches!(invoice.subscription, Some(Expandable::Id(_))));
    }

    #[test]
    fn verification_matches_async_stripe() {
        let at = |header: &str, now: i64| Webhook::construct_event_at(PAYLOAD, header, SECRET, now);
        let good = header(1000, PAYLOAD, SECRET);
        let bad_v1 = format!("v1={}", "00".repeat(32));

        assert!(at(&good, 1300).is_ok());
        assert!(at(&good, 700).is_ok());
        assert!(matches!(at(&good, 1301), Err(WebhookError::BadTimestamp(1000))));
        assert!(matches!(at(&good, 699), Err(WebhookError::BadTimestamp(1000))));
        assert!(matches!(
            at(&header(1000, PAYLOAD, "whsec_other"), 1000),
            Err(WebhookError::BadSignature)
        ));
        assert!(matches!(at(&format!("{good},{bad_v1}"), 1000), Err(WebhookError::BadSignature)));
        assert!(at(&format!("{bad_v1},{good}"), 1000).is_ok());
        assert!(at(&format!("junk,{good},v0=ab"), 1000).is_ok());
        assert!(matches!(at("garbage", 1000), Err(WebhookError::BadSignature)));
        assert!(matches!(at("t=1000", 1000), Err(WebhookError::BadSignature)));
        assert!(matches!(
            at(&good.replace("t=1000", "t=abc"), 1000),
            Err(WebhookError::BadHeader(_))
        ));
        assert!(matches!(
            at(&good.replace(",v1=", ", v1="), 1000),
            Err(WebhookError::BadSignature)
        ));
        assert!(matches!(at("t=1000,v1=zz", 1000), Err(WebhookError::BadSignature)));
        assert!(matches!(
            Webhook::construct_event_at("{}", &header(1000, "{}", SECRET), SECRET, 1000),
            Err(WebhookError::BadParse(_))
        ));
    }

    #[test]
    fn unknown_object_kinds_fail_to_parse_like_async_stripe() {
        let parse = |payload: &str| Webhook::parse_event(payload);
        assert!(matches!(
            parse(r#"{"type":"something.new","data":{"object":{"object":"brand_new_thing"}}}"#),
            Err(WebhookError::BadParse(_))
        ));
        assert!(matches!(
            parse(r#"{"type":"customer.created","data":{"object":{"id":"cus_1"}}}"#),
            Err(WebhookError::BadParse(_))
        ));
        let event = parse(r#"{"type":"customer.created","data":{"object":{"object":"customer"}}}"#)
            .unwrap();
        assert_eq!(event.type_, EventType::Other("customer.created".to_string()));
    }

    #[test]
    fn expandable_accepts_id_or_object() {
        let id: Expandable<Customer> = serde_json::from_str(r#""cus_1""#).unwrap();
        assert!(matches!(id, Expandable::Id(_)));
        let object: Expandable<Customer> = serde_json::from_str(r#"{"id":"cus_1"}"#).unwrap();
        assert!(matches!(object, Expandable::Object(_)));
    }

    #[test]
    fn ids_accept_the_prefixes_async_stripe_accepted() {
        for id in ["pm_1", "card_1", "src_1", "ba_1"] {
            assert!(id.parse::<PaymentMethodId>().is_ok(), "{id}");
        }
        assert!("tok_1".parse::<PaymentMethodId>().is_err());
        assert!("cus_1".parse::<CustomerId>().is_ok());
        assert!("pm_1".parse::<CustomerId>().is_err());
        assert!("sub_1".parse::<SubscriptionId>().is_ok());
        assert!(serde_json::from_str::<CustomerId>(r#""pm_1""#).is_err());
        assert!(
            serde_json::from_str::<InvoiceId>("null")
                .unwrap()
                .as_str()
                .is_empty()
        );
        assert!(
            serde_json::from_str::<InvoiceId>(r#""""#)
                .unwrap()
                .as_str()
                .is_empty()
        );
        assert!(serde_json::from_str::<InvoiceId>(r#""in_1""#).is_ok());
    }

    #[test]
    fn unknown_error_code_is_other() {
        let error: RequestError =
            serde_json::from_str(r#"{"type":"card_error","code":"something_new"}"#).unwrap();
        assert_eq!(error.code, Some(ErrorCode::Other("something_new".to_string())));
        let error: RequestError = serde_json::from_str(r#"{"code":"incorrect_cvc"}"#).unwrap();
        assert_eq!(error.code, Some(ErrorCode::IncorrectCvc));
    }
}
