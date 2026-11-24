use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tracing::*;

use super::stripe_error::SimplifiedStripeError;

const STRIPE_API_BASE: &str = "https://api.stripe.com/v1";

/// Minimal subscription fields Lockbook actually uses. Intentionally excludes
/// buyer PII (email, name, billing address, card fingerprint) and closed
/// Stripe enums so API additions never break deserialization or leak into logs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripeSubscription {
    pub id: String,
    pub status: String,
    /// Unix seconds.
    pub current_period_end: u64,
    /// Unix seconds.
    pub created: u64,
    pub cancel_at_period_end: bool,
    pub cancel_at: Option<u64>,
    pub price_id: String,
    pub payment_method_id: String,
    pub card_last4: String,
}

impl StripeSubscription {
    /// `past_due` keeps Premium while Stripe retries the payment.
    pub fn is_live(&self) -> bool {
        matches!(self.status.as_str(), "active" | "trialing" | "past_due")
    }
}

#[async_trait]
pub trait StripeClient: Send + Sync + Clone + 'static {
    /// Creates an anonymous Stripe customer and returns its `cus_...` ID.
    async fn create_customer(&self) -> Result<String, SimplifiedStripeError>;

    /// A hosted page where `customer_id` subscribes to `price_id`. Returns the session URL.
    async fn create_checkout_session(
        &self, customer_id: &str, price_id: &str, success_url: &str, cancel_url: &str,
    ) -> Result<String, SimplifiedStripeError>;

    /// A hosted page where `customer_id` manages their subscription. Returns the portal URL.
    async fn create_billing_portal_session(
        &self, customer_id: &str, return_url: &str,
    ) -> Result<String, SimplifiedStripeError>;

    async fn cancel_subscription_at_period_end(
        &self, subscription_id: &str,
    ) -> Result<StripeSubscription, SimplifiedStripeError>;

    /// Ends a subscription immediately (used when a Lockbook account is deleted).
    async fn cancel_subscription_immediately(
        &self, subscription_id: &str,
    ) -> Result<(), SimplifiedStripeError>;

    async fn get_subscription(
        &self, subscription_id: &str,
    ) -> Result<StripeSubscription, SimplifiedStripeError>;

    /// Every subscription of `customer_id`, with its default payment method expanded.
    async fn list_subscriptions(
        &self, customer_id: &str,
    ) -> Result<Vec<StripeSubscription>, SimplifiedStripeError>;

    /// Expires `customer_id`'s checkout pages that are still open, so only the newest can be paid.
    async fn expire_open_checkout_sessions(
        &self, customer_id: &str,
    ) -> Result<(), SimplifiedStripeError>;
}

#[derive(Clone)]
pub struct StripeHttpClient {
    http: reqwest::Client,
    secret_key: Arc<String>,
}

impl StripeHttpClient {
    pub fn new(secret_key: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), secret_key: Arc::new(secret_key.into()) }
    }

    async fn parse_response(resp: reqwest::Response) -> Result<Value, SimplifiedStripeError> {
        let status = resp.status();
        let body = resp.text().await?;
        let json: Value = serde_json::from_str(&body).map_err(|e| {
            SimplifiedStripeError(format!("Stripe HTTP {status} invalid JSON: {e}"))
        })?;
        if !status.is_success() {
            let err_type = json
                .pointer("/error/type")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let err_code = json
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("");
            let err_msg = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Stripe request failed");
            return Err(SimplifiedStripeError(format!(
                "Stripe HTTP {status} ({err_type}/{err_code}): {err_msg}"
            )));
        }
        Ok(json)
    }

    async fn get_json(
        &self, path: &str, query: &[(&str, &str)],
    ) -> Result<Value, SimplifiedStripeError> {
        let resp = self
            .http
            .get(format!("{STRIPE_API_BASE}{path}"))
            .bearer_auth(&*self.secret_key)
            .query(query)
            .send()
            .await?;
        Self::parse_response(resp).await
    }

    async fn post_form(
        &self, path: &str, form: &[(&str, &str)],
    ) -> Result<Value, SimplifiedStripeError> {
        let resp = self
            .http
            .post(format!("{STRIPE_API_BASE}{path}"))
            .bearer_auth(&*self.secret_key)
            .form(form)
            .send()
            .await?;
        Self::parse_response(resp).await
    }
}

fn parse_subscription(v: &Value) -> Result<StripeSubscription, SimplifiedStripeError> {
    let id = v
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| SimplifiedStripeError("Stripe subscription missing id".into()))?
        .to_string();
    let status = v
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Pre-2025-03-31.basil puts current_period_end on the subscription; basil+ puts it on items.data[0].
    let current_period_end = v
        .get("current_period_end")
        .and_then(Value::as_i64)
        .or_else(|| {
            v.pointer("/items/data/0/current_period_end")
                .and_then(Value::as_i64)
        })
        .and_then(|ts| u64::try_from(ts).ok())
        .unwrap_or(0);

    let created = v
        .get("created")
        .and_then(Value::as_i64)
        .and_then(|ts| u64::try_from(ts).ok())
        .unwrap_or(0);

    let cancel_at_period_end = v
        .get("cancel_at_period_end")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let cancel_at = v
        .get("cancel_at")
        .and_then(Value::as_i64)
        .and_then(|ts| u64::try_from(ts).ok());

    let price_id = v
        .pointer("/items/data/0/price/id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let (payment_method_id, card_last4) = match v.get("default_payment_method") {
        Some(Value::String(id)) => (id.clone(), String::new()),
        Some(Value::Object(pm)) => {
            let id = pm
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let last4 = pm
                .get("card")
                .and_then(|c| c.get("last4"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            (id, last4)
        }
        _ => (String::new(), String::new()),
    };

    Ok(StripeSubscription {
        id,
        status,
        current_period_end,
        created,
        cancel_at_period_end,
        cancel_at,
        price_id,
        payment_method_id,
        card_last4,
    })
}

#[async_trait]
impl StripeClient for StripeHttpClient {
    async fn create_customer(&self) -> Result<String, SimplifiedStripeError> {
        let json = self.post_form("/customers", &[]).await?;
        let id = json
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| SimplifiedStripeError("Stripe customer missing id".into()))?
            .to_string();
        info!(customer_id = id.as_str(), "Created stripe customer");
        Ok(id)
    }

    async fn create_checkout_session(
        &self, customer_id: &str, price_id: &str, success_url: &str, cancel_url: &str,
    ) -> Result<String, SimplifiedStripeError> {
        info!(customer_id, price_id, "Creating stripe checkout session");

        let json = self
            .post_form(
                "/checkout/sessions",
                &[
                    ("mode", "subscription"),
                    ("customer", customer_id),
                    ("line_items[0][price]", price_id),
                    ("line_items[0][quantity]", "1"),
                    ("success_url", success_url),
                    ("cancel_url", cancel_url),
                    (
                        "custom_text[submit][message]",
                        "Billing is handled by Stripe. Lockbook never receives your card number or email.",
                    ),
                ],
            )
            .await?;

        let session_id = json.get("id").and_then(Value::as_str).unwrap_or("");
        let url = json
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                SimplifiedStripeError(format!("Stripe checkout session has no url: {session_id}"))
            })?
            .to_string();
        debug!(session_id, "Created stripe checkout session");
        Ok(url)
    }

    async fn create_billing_portal_session(
        &self, customer_id: &str, return_url: &str,
    ) -> Result<String, SimplifiedStripeError> {
        info!(customer_id, "Creating stripe billing portal session");

        let json = self
            .post_form(
                "/billing_portal/sessions",
                &[("customer", customer_id), ("return_url", return_url)],
            )
            .await?;

        let session_id = json.get("id").and_then(Value::as_str).unwrap_or("");
        let url = json
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                SimplifiedStripeError(format!(
                    "Stripe billing portal session has no url: {session_id}"
                ))
            })?
            .to_string();
        debug!(session_id, "Created stripe billing portal session");
        Ok(url)
    }

    async fn cancel_subscription_at_period_end(
        &self, subscription_id: &str,
    ) -> Result<StripeSubscription, SimplifiedStripeError> {
        info!(subscription_id, "Canceling stripe subscription at period end");

        let json = self
            .post_form(
                &format!("/subscriptions/{subscription_id}"),
                &[("cancel_at_period_end", "true")],
            )
            .await?;
        let sub = parse_subscription(&json)?;
        debug!(
            subscription_id = sub.id.as_str(),
            status = sub.status.as_str(),
            "Scheduled stripe subscription to cancel"
        );
        Ok(sub)
    }

    async fn cancel_subscription_immediately(
        &self, subscription_id: &str,
    ) -> Result<(), SimplifiedStripeError> {
        info!(subscription_id, "Canceling stripe subscription immediately");
        let resp = self
            .http
            .delete(format!("{STRIPE_API_BASE}/subscriptions/{subscription_id}"))
            .bearer_auth(&*self.secret_key)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            debug!(subscription_id, "Stripe subscription already gone");
            return Ok(());
        }
        let json = Self::parse_response(resp).await?;
        let status = json.get("status").and_then(Value::as_str).unwrap_or("");
        debug!(subscription_id, status, "Canceled stripe subscription immediately");
        Ok(())
    }

    async fn get_subscription(
        &self, subscription_id: &str,
    ) -> Result<StripeSubscription, SimplifiedStripeError> {
        let json = self
            .get_json(
                &format!("/subscriptions/{subscription_id}"),
                &[("expand[]", "default_payment_method")],
            )
            .await?;
        let sub = parse_subscription(&json)?;
        debug!(
            subscription_id = sub.id.as_str(),
            status = sub.status.as_str(),
            "Retrieved stripe subscription"
        );
        Ok(sub)
    }

    async fn list_subscriptions(
        &self, customer_id: &str,
    ) -> Result<Vec<StripeSubscription>, SimplifiedStripeError> {
        let json = self
            .get_json(
                "/subscriptions",
                &[
                    ("customer", customer_id),
                    ("status", "all"),
                    ("limit", "100"),
                    ("expand[]", "data.default_payment_method"),
                ],
            )
            .await?;
        let items = json.get("data").and_then(Value::as_array).ok_or_else(|| {
            SimplifiedStripeError("Stripe subscriptions list missing data".into())
        })?;
        let mut subs = Vec::with_capacity(items.len());
        for item in items {
            subs.push(parse_subscription(item)?);
        }
        debug!(customer_id, count = subs.len(), "Listed stripe subscriptions");
        Ok(subs)
    }

    async fn expire_open_checkout_sessions(
        &self, customer_id: &str,
    ) -> Result<(), SimplifiedStripeError> {
        let json = self
            .get_json(
                "/checkout/sessions",
                &[("customer", customer_id), ("status", "open"), ("limit", "100")],
            )
            .await?;
        let Some(items) = json.get("data").and_then(Value::as_array) else {
            return Ok(());
        };
        for item in items {
            let Some(session_id) = item.get("id").and_then(Value::as_str) else {
                continue;
            };
            info!(session = session_id, "Expiring an older stripe checkout session");
            let resp = self
                .http
                .post(format!("{STRIPE_API_BASE}/checkout/sessions/{session_id}/expire"))
                .bearer_auth(&*self.secret_key)
                .send()
                .await?;
            // A session that expired or completed between list and expire returns 400 — don't abort checkout.
            if resp.status() == reqwest::StatusCode::BAD_REQUEST {
                debug!(session = session_id, "Older checkout session already closed");
                continue;
            }
            Self::parse_response(resp).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_subscription_handles_pre_and_post_basil_and_payment_methods() {
        // Pre-2025-03-31.basil: top-level current_period_end + expanded card payment method.
        let pre_basil = parse_subscription(&json!({
            "id": "sub_1",
            "status": "active",
            "current_period_end": 1_800_000_000,
            "created": 1_700_000_000,
            "cancel_at_period_end": false,
            "default_payment_method": {
                "id": "pm_card",
                "type": "card",
                "card": { "last4": "4242" }
            },
            "items": { "data": [{ "price": { "id": "price_monthly" } }] }
        }))
        .unwrap();
        assert!(pre_basil.is_live());
        assert_eq!(pre_basil.current_period_end, 1_800_000_000);
        assert_eq!(pre_basil.payment_method_id, "pm_card");
        assert_eq!(pre_basil.card_last4, "4242");
        assert_eq!(pre_basil.price_id, "price_monthly");

        // Post-basil: current_period_end lives on items.data[0], Link/future method (no card).
        let post_basil = parse_subscription(&json!({
            "id": "sub_2",
            "status": "past_due",
            "created": 1_750_000_000,
            "cancel_at_period_end": false,
            "cancel_at": 1_850_000_000,
            "default_payment_method": { "id": "pm_link", "type": "amazon_pay" },
            "items": {
                "data": [{
                    "current_period_end": 1_850_000_000,
                    "price": { "id": "price_monthly" }
                }]
            },
            "future_unknown_field": { "nested": true }
        }))
        .unwrap();
        assert!(post_basil.is_live());
        assert_eq!(post_basil.current_period_end, 1_850_000_000);
        assert_eq!(post_basil.cancel_at, Some(1_850_000_000));
        assert_eq!(post_basil.payment_method_id, "pm_link");
        assert_eq!(post_basil.card_last4, "");

        // Unexpanded string payment method ID + non-live status.
        let ended = parse_subscription(&json!({
            "id": "sub_3",
            "status": "canceled",
            "current_period_end": 1_700_000_000,
            "created": 1_690_000_000,
            "default_payment_method": "pm_str",
            "items": { "data": [] }
        }))
        .unwrap();
        assert!(!ended.is_live());
        assert_eq!(ended.payment_method_id, "pm_str");
        assert_eq!(ended.card_last4, "");
    }
}
